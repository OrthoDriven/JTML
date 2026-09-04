## Review — seam: UI config decoupling

Note: no `structured_output` tool is available in this runtime; returning the complete structured artifact here per runtime fallback instructions.

### Verified call-site inventory (dilation sourcing, after spot-check)

All former `getActiveCostFunctionClass()->getIntParameters()` + name-scan sites in the widgets UI have been migrated to `jta_cost_function::getDilation(trunk_manager_.objective_spec)` — **but `objective_spec` is never written anywhere in production**, so all of these read the default `DirectDilationSpec{dilation=6}` (objective_spec.h:9). Exhaustive grep: the only assignment to `objective_spec` is the CFM copy-assign (CostFunctionManager.cpp:72). Sites:

| # | Site | Location | Replacement |
|---|------|----------|-------------|
| 1 | Segment helper (`segmentHelperFunction`) | mainscreen.cpp:1782 | shared spec accessor (after P0-1 fix) |
| 2 | Load-image (`on_load_image_button_clicked`) | mainscreen.cpp:2508 | same |
| 3 | Edge handler 1 (`on_aperture_spin_box_valueChanged`) | mainscreen.cpp:3881 | same |
| 4 | Edge handler 2 (`on_low_threshold_slider_valueChanged`) | mainscreen.cpp:3960 | same |
| 5 | Edge handler 3 (`on_high_threshold_slider_valueChanged`) | mainscreen.cpp:4039 | same |
| 6 | Edge handler 4 (`on_apply_all_edge_button_clicked`) | mainscreen.cpp:4093 | same |
| 7 | `UpdateDilationFrames` | mainscreen.cpp:4848 | same; drop `value_or(3)` |
| 8 | SettingsBridge QML props (`trunk/branch/leafDilation`, `hasDilation`) | SettingsBridge.cpp:416–441 → impl 603–640 | CF-level `getIntParameterValue("Dilation", v)` (CostFunction.cpp:82) — kills both raw scans |
| 9 | `setDilation` | SettingsBridge.cpp:618–629 | keep `setIntParameterValue` + add spec sync (P0-1 write path) |
| 10 | MlBridge | MlBridge.cpp:514–517 | **KEEP** — already one typed call, `0` fallback matches widgets' `value_or(0)` |
| 11 | `DeriveStageParams` funnel | optimizer_manager.cpp:1280–1284 → DeriveStageCostParams (optimizer_stage_script.cpp:145–184) | **KEEP** (transitional; becomes `spec.dilation` once spec is source of truth, then PEEL) |
| 12 | Engine A/B spec prints | optimizer_manager.cpp:1013–1024, 1060–1071, 1125–1136 | debug-only; delete with the seam |
| 13 | settings_control parameter listing | settings_control.cpp:562, 616 | **KEEP** — generic parameter UI, not dilation special-cased |

### Findings

**P0-1 — ObjectiveSpec is dead state; 7 production UI reads are pinned to constant 6, and the two front-ends now diverge.**
Location: `objective_spec` (CostFunctionManager.h:132) is written nowhere (only CostFunctionManager.cpp:72 copy). All mutation paths — settings_control.cpp:912 (`setIntParameterValue`), SettingsBridge.cpp:627, session load mainscreen.cpp:4700–4712 — update only the CF parameter. Evidence of divergence: QML reads the live CF param (SettingsBridge.cpp:606–616, MlBridge.cpp:517), widgets read the dead spec → segment/load-image/UpdateDilationFrames always dilate by 6 regardless of the user's Dilation setting. The engine's own A/B prints expose this live: `ObjectiveSpec dilation: 6` vs `Legacy dilation: <user value>` (optimizer_manager.cpp:1019–1029). This changes segmentation output, i.e., pipeline behavior. Smallest fix: one write-path sync — CFM `setIntParameterValue` (and the load path) re-derives `objective_spec` from the CF's params; or revert the 7 mainscreen reads to the CF param until spec is wired. Verify with a targeted probe: set trunk Dilation=9, resegment, compare dilation of produced images (legacy expectation: 9).

**P0-2 — SettingsBridge::applyCostFunctionEntries parameter writes are silent no-ops.**
Location: SettingsBridge.cpp:536–560. `manager->getAvailableCostFunctions()` returns by value (CostFunctionManager.cpp:209–212); the writes at 551–559 mutate the discarded copy; only `ACTIVE_CF` (513–523) survives `load()` (SettingsBridge.cpp:76–86). So the QML app never restores saved Dilation/parameter values. Fix (mirrors the widgets load, mainscreen.cpp:4700–4712): `manager->getCostFunctionClass(*cost_function_type)->set...ParameterValue(...)` — getCostFunctionClass returns a pointer into the live map (CostFunctionManager.cpp:225–228). Same no-op class as the recorded baseline.json:724 finding.

**P1-1 — `updateCostFunctionParameterValues` (3 overloads) is a no-op AND dead in production. DELETE.**
Location: CostFunctionManager.cpp:162–204 (each loops over a by-value `getIntParameters()` copy and mutates the copy; returns true as if it worked). Production callers: **zero** (exhaustive grep). Only callers are `test/oracle/*.cpp` (oracle_test.cpp:239, throughput_serial_pipeline.cpp:161, layered_correctness_test.cpp:180, bit_identity_test.cpp:214) — not built, not run, explicitly not a gate (AGENTS.md). The only production session-restore path is `getCostFunctionClass()->set...ParameterValue`. Delete all three overloads and the header decls (CostFunctionManager.h:63–72).

**P1-2 — 4x default-config duplication → ONE factory.**
Locations: mainscreen.cpp:4772–4781, settings_control.cpp:989–998, SettingsBridge.cpp:36–48 (ctor), SettingsBridge.cpp:110–119 (reset). All four are: `CFM(Stage::Trunk/Branch/Leaf)` + `DirectDilation` Dilation=4 (branch) / 1 (leaf). Factory:
```cpp
// services/stage_config_factory.{h,cpp}   (jtml_services already privately links
// jtml_compute — see cost_function_registry.cpp:15; no coordinator involved)
struct DefaultStageConfigs {
    std::unique_ptr<jta_cost_function::CostFunctionManager> trunk, branch, leaf;
};
DefaultStageConfigs MakeDefaultStageConfigs();
// trunk: registered default (D=6, CostFunctionManager.cpp:287), branch: D=4, leaf: D=1;
// also writes each stage's ObjectiveSpec (6/4/1) — the P0-1 write path exists from birth.
```
Companion: add `BRANCH_DILATION = 4` to domain/settings_constants.h (only TRUNK_DILATION=6 and Z_SEARCH_DILATION=1 exist today, settings_constants.h:26/37; the 4 is a local constexpr in SettingsBridge.cpp:22). SettingsBridge ctor and reset become `*this-config = MakeDefaultStageConfigs()`.

**P2-1 — `getAvailableCostFunctions` should return `const std::map<CostFunctionType, CostFunction>&`.**
All callers verified read-only: settings_control.cpp:54 (name enumeration), cost_function_registry.cpp:41 (registry mapping), SettingsBridge.cpp:649 (`costFunctionNames`); SettingsBridge.cpp:536 is deleted by the P0-2 fix. No mutation through the returned map exists anywhere except the P0-2 bug itself. Prerequisite: constify the pure getters `CostFunction::getIntParameters/getDoubleParameters/getBoolParameters/getCostFunctionName` (CostFunction.h:100–107, non-const today) and `Parameter::getParameterName/getParameterValue/getParameterType` (Parameter.h:37–49 etc.) — mechanical, no behavior change. This structurally kills the copy-write bug class.

**P2-2 — `UpdateDilationFrames` `value_or(3)` fallback is dead and contradicts the legacy contract; the "DIRECT_MAHFOUZ→3" special case no longer exists.**
`getDilation(spec)` returns 6 for DirectDilationSpec (objective_spec.cpp:12–14), never nullopt, and `CostFunctionType` has exactly one member (`COST_FUNCTION_TYPE_LIST(X) X(DirectDilation)`, CostFunction.h:21) — no MAHFOUZ. The legacy comment promises "saves Dilation as 0 if no trunk Dilation param" (mainscreen.cpp:4841–4843) but the code falls back to 3 (mainscreen.cpp:4848). DELETE the fallback once spec is the source of truth. Related comment rot: `DeriveStageCostParams`' header still claims "the DIRECT_MAHFOUZ → 3 special case (applied after the clamp, like the manager)" (optimizer_stage_script.cpp:152–158) but the body contains no such case (165–184) — fix the comment; the ≤0-clamp and last-match-wins transcription pins remain valid (KEEP until the declarative graph subsumes DeriveStageCostParams).

### Classification summary
- **KEEP**: MlBridge.cpp:514–517 typed read; `DeriveStageParams` single funnel (transitional, bit-identity); settings_control generic parameter listing (settings_control.cpp:562, 616).
- **PEEL**: 7 mainscreen spec reads → one shared accessor over the *written* spec (P0-1); SettingsBridge dilation trio → `getIntParameterValue("Dilation", v)`; 4x default duplication → `MakeDefaultStageConfigs()`; CFM load paths → `getCostFunctionClass` mutation only.
- **DELETE**: `updateCostFunctionParameterValues` x3; the by-value map pattern in applyCostFunctionEntries; `value_or(3)` fallback + stale MAHFOUZ comment (optimizer_stage_script.cpp:152–158); engine debug spec prints (optimizer_manager.cpp:1013–1029 et al.) once the seam lands. (P2, separate: settings_control.cpp:54 constructs a whole temp CFM just to enumerate one name — use a static list.)

### Merge verdict: **BLOCK** (for the current tree as a step toward the target shape)
Not against a diff but against the seam's own invariant: P0-1 makes the widgets dilation reads silently ignore user settings (behavior regression, two front-ends disagree), and P0-2 makes the QML session-restore drop all parameter values. Both are small, local fixes; everything else is P1/P2 peel work. No issues found beyond those listed; the inventory, grouping (segmentation/edge-path vs. QML props vs. UpdateDilationFrames vs. DeriveStageParams), and factory design above are otherwise consistent with the explicit-inputs-peeling playbook and require no new wrapping layer between callers and CFM.