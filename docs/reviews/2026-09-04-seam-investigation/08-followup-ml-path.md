All evidence is gathered. Here is the full report.

---

# FOLLOW-UP B: The production ML path and remaining ambient config reads

## Review

### Verdict on prior claims (spot-checked, corrected where needed)

| Prior claim | Status | Evidence |
|---|---|---|
| `objective_spec` is NEVER written (only copy-assign) | **CONFIRMED** | Exhaustive grep over `src/` + `include/`: the only assignment is `CostFunctionManager.cpp:72` (`objective_spec = other.objective_spec;`). The member is declared at `include/compute/CostFunctionManager.h:128` (`JTML_DLL ObjectiveSpec objective_spec;`) and is default-constructed → `DirectDilationSpec{dilation=6}` (`include/compute/objective_spec.h:9-10`). Every spec read therefore reads constant **6**. |
| 7 mainscreen dilation reads read constant 6 | **CONFIRMED, with a mechanism correction** | The 7 sites are **not** `getActiveCostFunctionClass()` scans (prior framing) — they are `getDilation(trunk_manager_.objective_spec)` reads: `mainscreen.cpp:1782` (ML segment, per-frame), `2508` (load image), `3881`, `3960`, `4039` (edge slots), `4093` (apply-all-edges), `4848` (`UpdateDilationFrames`, note `.value_or(3)` here vs `.value_or(0)` elsewhere). All 7 return the dead constant 6. |
| `MlBridge.cpp:515` CFM-parameter Dilation scan | **CONFIRMED EXISTS, but experimental only** | `app/experimental/MlBridge.cpp:514-517` (`trunkManager()->getActiveCostFunctionClass()->getIntParameterValue("Dilation", dilation_val)`). Per the owner rule, out of scope. |
| SettingsBridge.cpp:536-560 apply-by-value no-op | **CONFIRMED, but experimental only** | `SettingsBridge.cpp:536` `auto available_cost_functions = manager->getAvailableCostFunctions();` copies the map by value; the mutations at 552-558 hit the copy. Irrelevant per rule — and note the **production** registry-apply (`mainscreen.cpp:4683`, `getCostFunctionClass(...)` returns a pointer; mutations at 4686/4690/4696 hit the live manager) is NOT affected by that bug pattern. |
| Test tree not built | **CONFIRMED** | Root `CMakeLists.txt:127-130`: only `add_subdirectory(src)`, `add_subdirectory(packaging)`, `add_subdirectory(rust)`. `JTML_BUILD_TESTS` option exists (line 42) but no `add_subdirectory(test)` anywhere. |

### Q1 — The production ML path (`src/services/ml_orchestrator.{h,cpp}`)

**What it reads/writes.** `MlOrchestrator` is fully explicit-inputs; it reads **nothing ambient**:

- `SegmentFrame` (`src/services/ml_orchestrator.cpp:18-66`): takes `aperture / low / high / dilation / full_postprocessing` as **parameters**; runs the injected `SegmentOp`; writes the frame tail (`GetInvertedImage()` copy at :54, `SetEdgeImage` :56, `SetDilatedImage` :57, `SetDistanceMap` + `setCurvatureHeatmaps` :59-60). No CFM, no optimizer settings, no LocationStorage.
- `EstimateFrame` (`ml_orchestrator.cpp:67-89`): runs the injected `EstimateOp`, calls the injected `SavePoseFn` (:79-81), and returns the seed `{frame, model, pose}` (:84-88). LocationStorage is reached **only** through the injected save seam.
- The GPU/torch seams (`SegmentationController::SegmentFrame`, `EstimateImplantPose`, `implant_estimator.cpp`) include only `compute/machine_learning_tools.h` / `gpu_model.cuh` — no CFM/coordinator references (services include sweep, `src/services/CMakeLists.txt` + all 16 TUs: the **only** CFM touch in services is `cost_function_registry.cpp:20-82`, an explicit-argument registry dump; `edge_processor.cpp` is parameter-driven; no services TU references coordinator).

**Production path ML estimate → seed → controller (widgets).**

1. `MainScreen` builds the injected ops: segment op at `mainscreen.cpp:1773-1779` (closes over `segmentation_controller_` + torch module), estimate op at `1994-1998`, save-pose lambda at `2006-2008` → `model_locations_.SavePose(frame, model, pose)`.
2. `ml_orchestrator_.EstimateFrame(...)` at `mainscreen.cpp:2015-2020` (and again at `2247` for the tibial slot). The returned `outcome.seed` is **ignored** in widgets — only `status` is consumed (`mainscreen.cpp:2021-2024`); the comment at `2001-2004` documents that "the storage write IS the seed".
3. `LaunchOptimizer` (`mainscreen.cpp:4243+`): `req.storage = &model_locations_` (`4283`) and `req.launch.pose_matrix = model_locations_` **by value** (`4291`).
4. `optimizer_run_controller_.start(req)`: widgets never sets an explicit seed (no `setSeedPose` call anywhere in mainscreen — verified; only `clearSeedPose` in the ctor lambda at `mainscreen.cpp:146`). So `takeSeedForRun` returns `applied=false` (`src/coordinator/optimizer_run_controller_core.cpp:101-103`), the controller re-syncs `launch.pose_matrix = *req.storage` (`optimizer_run_controller.cpp:117-125`).
5. `OptimizerManagerRunDriver::Initialize` passes `launch.pose_matrix` into a **fresh** `OptimizerManager` (`src/coordinator/optimizer_run_driver.cpp:66-83`, manager constructed at :31-33).
6. `OptimizerManager::Initialize` uses `SetStartingPoint(pose_matrix.GetPose(start_frame_index_, primary_model_index_))` (`src/coordinator/optimizer_manager.cpp:232-234`).

**The hop that carries the ML estimate is the LocationStorage by-value copy into `launch.pose_matrix`.** There is no explicit seed in the widgets app.

**Does the ML path do the `getActiveCostFunctionClass()->getIntParameters() + "Dilation"` scan?** **No — and that is itself the finding.** The widgets ML segment path sources dilation from the **dead spec** (`mainscreen.cpp:1782`), i.e. the ML segment tail (`frame.SetDilatedImage(dilation)`, `ml_orchestrator.cpp:57`) is permanently dilated by **6** regardless of user settings. There is no production twin of the `MlBridge.cpp:515` scan; instead widgets and the experimental bridge *diverge* (widgets = frozen spec 6; experimental = live CFM parameter). The only production `"Dilation"` reads are:

- `optimizer_manager.cpp:981-982, 1028-1029, 1093-1094` — three per-stage **diagnostic** reads (`int legacy_dilation = -1; getActiveCostFunctionClass()->getIntParameterValue("Dilation", ...)` feeding `std::cout` only, :988/:994 etc.). These are the closest production twins of the MlBridge read.
- `DeriveStageCostParams` name-scan (`src/coordinator/optimizer_stage_script.cpp:145-167`, `"Dilation"||"DILATION"||"dilation"` last-match-wins at :164-168) — the one real production scan, invoked **6×** via `DeriveStageParams`: pre-run at `optimizer_manager.cpp:279/284/288` (into `trunk_/branch_/leaf_dilation_val_`) and per-stage in the loop at `992/1043/1107` (feeding `ResetStageDilation`, `996/1046/1110`, which dilates the comparison images, `optimizer_manager.cpp:1243-1269`).
- 6 kernel-side reads at cost execution (`compute/DIRECT_DILATION.cpp:34-35`, `DIRECT_DILATION_T1.cpp:35-36`, `DIRECT_DILATION_SAME_Z.cpp:43-44`, `DIRECT_DILATION_POLE_CONSTRAINT.cpp:42-43`, `DD_NEW_POLE_CONSTRAINT.cpp:41-42`, `sym_trap_function.cpp:44-45`) — legitimate runtime cost reads, not ambient config.

### Q2 — Ambient-read sweep of `src/services/`

**Clean.** Exhaustive greps for `getActiveCostFunctionClass`, `objective_spec`, `CostFunctionManager`, `optimizer_settings`, `getCostFunctionClass`, `set*ParameterValue` over `src/services/`:

- `cost_function_registry.cpp` — the only CFM consumer; takes the three managers as **explicit arguments** (:24-28); iterates `getAvailableCostFunctions()` and copies parameter vectors for the registry dump (:44-64). Not ambient; no `"Dilation"` name filter.
- `settings_service.cpp` — persistence only; the `CostFunctionManager` mention is a comment (:52).
- `edge_processor.cpp` — `ResolveDilation(raw, name)` (:15-27) is explicit-inputs (clamp ≥0 + `DIRECT_MAHFOUZ`→3 override); dilation comes in as a parameter.
- `ml_orchestrator.cpp`, `session_controller.cpp`, `study_load_controller.cpp`, `location_storage.cpp`, `save_last_pose.cpp`, `segmentation_controller.cpp`, `implant_estimator.cpp` — no CFM / optimizer-settings / coordinator references.

The prior inventory's focus on view/coordinator was correct: **services adds zero ambient reads.**

### Q3 — `session_controller.cpp:56` area

Lines 56 and 58 (`src/services/session_controller.cpp`):

```cpp
float* prin_dist_ = &principal_calibration_file.principal_distance_;   // :56
Calibration* cal_pointer_ = &result.calibration;                        // :58
```

Both are **dead locals** (grep confirms no other use in the file) pointing into locals of the monoplane branch. They are vestigial legacy scaffolding, **not** reads of dead CFM members — `session_controller.cpp` contains no CFM reference at all. The same pattern does not repeat in the biplane branch. P2 dead code, safe to delete; nothing else in the file touches CFM state.

### Q4 — True production counts and fix coverage

**Spec reads (all currently dead → constant 6):**

| # | Site | Purpose |
|---|---|---|
| 7 | `mainscreen.cpp:1782, 2508, 3881, 3960, 4039, 4093, 4848` | view post-processing dilation (edge/dilation images + **ML segment tail**) |
| 3 | `optimizer_manager.cpp:971-978, 1018-1025, 1083-1090` | diagnostic `std::get<DirectDilationSpec>(stage_manager->objective_spec)` couts |

**Total: 10 dead spec reads in production** (7 view + 3 coordinator diagnostics). Plus one dormant consumer: `CostFunctionManager::InitializeActiveCostFunction` has the `DirectDilationObjective(std::get<DirectDilationSpec>(objective_spec), ...)` construction **commented out** (`CostFunctionManager.cpp:254-262`; header `include/objectives/direct_dilation.hpp:32-45` stores `spec_`).

**Live CFM-parameter `"Dilation"` reads in production (non-kernel):** 6 scan invocations (1 scan implementation) + 3 direct diagnostic reads (listed in Q1). **Dilation parameter writes in production:** `settings_control.cpp:912` (user int-parameter edit on the settings dialog's own `sc_*` copies), registry load `mainscreen.cpp:4690` (pointer into live managers), first-run defaults `mainscreen.cpp:4778-4781` (branch 4 / leaf 1), reset defaults `settings_control.cpp:996-998`; and the `SettingsControl`→`MainScreen` round-trip is by-value `SaveSettings` → copy-assign at `mainscreen.cpp:4813-4816` (which does propagate `objective_spec`, `CostFunctionManager.cpp:72` — but only ever copies a never-written spec).

**Does the `MakeDefaultStageConfigs` factory + spec-sync fix still cover all of them?** With amendments:

1. **`MakeDefaultStageConfigs` does not exist in code yet** — it appears only in the synthesis doc (`docs/reviews/2026-09-04-seam-investigation/00-synthesis.md:84,137`; `04-ui-config-decoupling.md:44-48`). When implemented, it must own the three per-stage default dilation values that are today inline in **three** places: CFM registration default 6 (`CostFunctionManager.cpp:284`), branch 4 / leaf 1 (`mainscreen.cpp:4778-4781`, `settings_control.cpp:996-998`). Note the existing `domain/settings_constants.h` constants (`TRUNK_DILATION=6` :26, `Z_SEARCH_DILATION=1` :37) are referenced **only** by experimental code (`SettingsBridge.cpp:417/423`) — production ignores them.
2. **The spec-sync must hook the production write paths, not just SettingsBridge.** The synthesis framed "Registry→spec sync at write path" against the experimental bridge; in production the write paths are `settings_control.cpp:912` (user edits — and this is the *only* sink for user dilation changes) and the registry load `mainscreen.cpp:4690`. Without hooking these, spec-sync fixes nothing in the widgets app.
3. **The 3 coordinator diagnostic spec reads + 3 diagnostic CFM reads** (`optimizer_manager.cpp:971-994, 1018-1040, 1083-1104`) are parity-shim scaffolding (`std::cout` only) and should be deleted with the shim, not "fixed" — the synthesis's "DeriveStageCostParams = parity shim until probes pass, then DELETE" already covers the shim; make explicit that the six diagnostic lines go with it.
4. **Coverage of the 7 view reads:** one shared spec accessor over the *written* spec covers all 7 (they are literally identical expressions). After sync they also become *live*, which fixes the real user-visible bug below.

### Findings

- **Finding P1 — View-side and widgets-ML dilation is frozen at 6 while the optimizer uses the live setting.** Location: `mainscreen.cpp:1782, 2508, 3881, 3960, 4039, 4093, 4848`; root cause: `objective_spec` never written (only `CostFunctionManager.cpp:72` copies a never-written spec). Evidence: user edits Dilation via `settings_control.cpp:912` → the run path picks it up (`optimizer_stage_script.cpp:164-168` → `ResetStageDilation` → kernels) but all view dilation images and the ML segment tail stay at 6. Smallest fix: spec-sync at the two write paths (`settings_control.cpp:912`, `mainscreen.cpp:4690`) + the shared accessor over the written spec for the 7 reads.
- **Finding P2 — Dormant ObjectiveInstance path would consume the dead spec.** `CostFunctionManager.cpp:254-262` (commented `DirectDilationObjective(std::get<DirectDilationSpec>(objective_spec), ...)`) — re-enabling the peeled objective before spec-sync lands would make the **optimizer cost itself** use constant dilation 6. The peel plan must sequence spec-sync before (or with) un-commenting this path.
- **Finding P2 — Mahfouz fallback unreachable in the view path.** `getDilation` returns `nullopt` only for `MahfouzVariantSpec` (`objective_spec.cpp:8-17`), but the spec is never switched to that alternative, so the `.value_or(0)` / `.value_or(3)` fallbacks at the 7 mainscreen sites are dead branches; the Mahfouz→3 override survives only through `EdgeProcessor::ResolveDilation`'s name check (`edge_processor.cpp:22-24`) on the edge paths, and **not** on the ML segment path (`mainscreen.cpp:1782` passes the raw value into `MlOrchestrator::SegmentFrame`).
- **Finding P2 — Widgets ML seed has no stale-frame guard, unlike the QML path.** The U5 `takeSeedForRun` guards (`optimizer_run_controller_core.cpp:105-113`) protect the explicit-seed path only; widgets has no explicit seed, so the run always starts from `pose_matrix.GetPose(start_frame, primary)` (`optimizer_manager.cpp:234`). An ML estimate on frame X followed by a launch on frame Y starts from whatever Y holds in storage, silently. Legacy parity, report-only.
- **Finding P2 — Dead locals in `session_controller.cpp:56/58`** (see Q3). Smallest fix: delete both lines.
- **Experimental findings confirmed but out of scope per the owner rule:** `SettingsBridge.cpp:536` by-value map no-op; `SettingsBridge.cpp:609/634` `"Dilation"` scans; `MlBridge.cpp:514-517` scan. None affect production.

### Corrections to the prior reports (summary)

1. The "7 mainscreen dilation reads" are **spec reads** (`getDilation(objective_spec)`), not `getActiveCostFunctionClass` scans — same constant-6 outcome, different mechanism, and it changes where the fix lands (spec-sync at CFM-parameter write paths, not a parameter-scan replacement).
2. The production twin of the `MlBridge.cpp:515` scan does **not** exist on the widgets ML path — widgets ML uses the dead spec instead (divergence, P1 above). The nearest production twins are the three diagnostic reads in `optimizer_manager.cpp` and the `DeriveStageCostParams` scan.
3. The production analog of the SettingsBridge by-value registry bug does **not** exist (`mainscreen.cpp:4683` mutates via pointer); the bug is experimental-only.
4. `MakeDefaultStageConfigs` is still proposal-only; its default set must include branch 4 / leaf 1, which currently live inline in two production files plus the CFM registration default.
5. services adds zero ambient reads; `session_controller.cpp:56/58` are dead locals, not CFM reads.

**Merge verdict:** n/a (report-only pass; no diff). Overall assessment: prior seam reports stand with the four corrections above; the P1 spec-sync/divergence item should gate the ObjectiveInstance un-commenting.

**Suggested supervisor-run commands:** none required; no build/test/git commands needed for this pass (all claims verified by reading source).