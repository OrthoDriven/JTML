All evidence gathered. Here is the follow-up report.

## Follow-up A Review — Production call surface + build reality of `src/app/experimental/`

Method note: read-only pass; every claim below verified by reading the cited file at the cited lines in the current working tree (`.build` artifacts read as evidence of the configured cache — no commands run, no edits).

---

### 1. Build wiring: the experiment IS built by the default pixi flow

**Verdict: `jtml_experimental` is compiled and linked in every `pixi run build` (release) today.** The "it doesn't matter" rule is semantically true but the sources are still on the compiler's plate.

Evidence chain:

- Root `CMakeLists.txt:127-130` — `add_subdirectory(src)`, `packaging`, `rust`. **No `add_subdirectory(test)` anywhere in the tree** (exhaustive grep over all `CMakeLists.txt` for `add_subdirectory`: only `src/*`, `rust/*`, `Study2Grid`, `experimental`). The `JTML_BUILD_TESTS` option at root `CMakeLists.txt:51-52` is a dead knob.
- `src/CMakeLists.txt:4-9` — adds `domain`, `services`, `compute`, `coordinator`, `view`, `app`.
- `src/app/CMakeLists.txt:17-19` — `option(JTML_BUILD_EXPERIMENTAL "Build experimental QML frontend" ON)` — **default ON**; gated `add_subdirectory(experimental)` at line 27.
- `CMakePresets.json` — the **`release` preset (lines 37-46) sets only `CMAKE_BUILD_TYPE`** and inherits `pixi-base`; it does **not** set `JTML_BUILD_EXPERIMENTAL`. Only the `profile` preset (lines 63-65) sets it false (along with TESTS and STUDY2GRID).
- `pixi.toml` — `[tasks.configure]` = `cmake --preset release`; `[tasks.build]` = `cmake --build --preset release`. So the default dev/prod loop → option default ON applies.
- **Hard proof from the actual build tree:** `.build/CMakeCache.txt:611` → `JTML_BUILD_EXPERIMENTAL:BOOL=ON` (and `:617` → `JTML_BUILD_TESTS:BOOL=OFF`); `.build/bin/jtml_experimental` **exists as a built binary** alongside `joint-track-machine-learning` and `Study2Grid-Cmake`.
- `src/app/experimental/CMakeLists.txt:49-77` — `add_executable(jtml_experimental …)` with all 7 bridges + renderer; links `jtml_coordinator` + `jtml_compute` (lines 79-93). It is a leaf **executable**: nothing in production links it.
- `packaging/CMakeLists.txt:42` — `install(TARGETS jtml_compute jtml_domain jtml_services jtml_coordinator jtml_view …)` plus the main app (lines 54-55). **No experimental install.**
- Production references to experimental, exhaustive: **comments only**. `src/view/CMakeLists.txt:13` (QML decision record), `src/services/render_pipeline_builder.h:9`, `src/view/mainscreen.cpp:4831`, `src/coordinator/optimizer_run_controller.cpp:6,61`, `optimizer_run_controller_core.cpp:104`. `src/app/main.cpp` (composition root) includes only `view/mainscreen.h` + `view/settings_impl.h` and instantiates `MainScreen` — zero experimental contact.
- Gating knob: the only thing that turns the experiment off in the default flow is the **profile** preset / `pixi run configure-profiling`. The release preset does not.

**Correction/confirmation for the reports:** synthesis §2a's build-wiring claim ("`jtml_experimental` is compiled in every production build today") is **confirmed**, with stronger evidence than it cited (cache line + binary on disk). The proposed step 0.5 (set `JTML_BUILD_EXPERIMENTAL=false` in the release preset or delete the subtree) remains the correct hygiene move.

---

### 2. Corrected CFM call surface (production = view, coordinator, services, compute, domain only)

#### New structural fact discovered during verification (changes the shape of the surface)

**The cost-function registry has been stripped to a single function.** `include/compute/CostFunction.h:21` — `#define COST_FUNCTION_TYPE_LIST(X) X(DirectDilation)`; the enum has exactly **one** enumerator. `CostFunctionManager::listCostFunctions()` (`CostFunctionManager.cpp:295-298`) registers exactly one `CostFunction("DIRECT_DILATION")` with int param `"Dilation"` default 6. The dispatch switches `callActiveCostFunction` / `InitializeActiveCostFunction` / `DestructActiveCostFunction` (`CostFunctionManager.cpp:237-290`) each have a single `case DirectDilation`. The other cost-function TUs (`sym_trap_function.cpp`, `DD_NEW_POLE_CONSTRAINT.cpp`, `DIRECT_DILATION_T1/SAME_Z/POLE_CONSTRAINT.cpp`, `DIRECT_MAHFOUZ.cpp`) are still compiled (`src/compute/CMakeLists.txt:53-60`) and still declared as CFM members (`CostFunctionManager.h` private section), but are **unreachable** through the active-CF switches. Any report statement implying multiple selectable cost functions in the current tree is stale.

#### `objective_spec` — corrected "who reads / who writes"

**Writes:** exactly one — copy-assignment `CostFunctionManager.cpp:72`. Exhaustive grep `objective_spec\s*=[^=]` over `src/` and `test/`: one hit. No setter exists; the member is public (`CostFunctionManager.h:132`, `JTML_DLL ObjectiveSpec objective_spec;`). Therefore **every CFM instance forever holds `DirectDilationSpec{dilation = 6}`** (`include/compute/objective_spec.h:11-13`). Prior claim confirmed.

**Production readers (corrected list — the prior "7 mainscreen sites" list was incomplete):**

| Site | Lines | What it does |
|---|---|---|
| `src/view/mainscreen.cpp` | **1782** | `getDilation(trunk_manager_.objective_spec).value_or(0)` → feeds `ml_orchestrator_.SegmentFrame` for camera A (1785) and B (1801) |
| `src/view/mainscreen.cpp` | **2508** | `on_load_image_button_clicked` → `ImageLoadParams` dilation |
| `src/view/mainscreen.cpp` | **3881, 3960, 4039, 4093** | four edge handlers (`EdgeProcessingParams` dilation; note each also reads the *live* type via `trunk_manager_.getActiveCostFunction()` at 3889/3968/4047/4101 — type live, dilation frozen) |
| `src/view/mainscreen.cpp` | **4848** | `UpdateDilationFrames`, `value_or(3)` |
| `src/coordinator/optimizer_manager.cpp` | **971/974** (trunk), **1018/1021** (branch), **1083/1086** (leaf) | `std::holds_alternative`/`std::get<DirectDilationSpec>` on `stage_manager->objective_spec`, then `std::cout << spec.dilation` — the engine debug spec prints, immediately followed by the live read `getActiveCostFunctionClass()->getIntParameterValue("Dilation", legacy_dilation)` (981-982, 1028-1029, 1093-1094) |
| `src/compute/CostFunctionManager.cpp` | **259** | commented-out `ObjectiveInstance` swap reads `std::get<DirectDilationSpec>(objective_spec)` — the prepared swap path (unreachable today) |

**Experimental readers of `objective_spec`: zero.** None of SettingsBridge/OptimizerBridge/MlBridge/AppBridge/PoseBridge/StudyBridge touches the member (grep across `src/` confirms). So "who reads objective_spec" = **7 mainscreen sites + 3 optimizer_manager debug pairs + 1 commented CFM line**. The synthesis §1 headline listed only the 7; the optimizer_manager reads were mentioned later as "debug spec prints" but not counted as spec readers — now they are, with lines verified (synthesis cited 1013-1030/1060-1077/1126-1140; actual current lines are 971-982/1018-1029/1083-1094 — line drift, same code).

#### `MlBridge.cpp:515` — where it lives and whether a production analog exists

- **Location:** `src/app/experimental/MlBridge.cpp:515-517` — `settings_bridge_->trunkManager()->getActiveCostFunctionClass()->getIntParameterValue("Dilation", dilation_val)`. Confirmed experimental-only (target source list, `src/app/experimental/CMakeLists.txt:57-58`).
- **Production analog EXISTS and is the same consumer:** `mainscreen.cpp:1782-1793` also feeds `jta::MlOrchestrator::SegmentFrame` (production, `src/services/ml_orchestrator.cpp:18`, wired at `services/CMakeLists.txt:35`; MlOrchestrator itself has **no CFM access** — dilation is a plain parameter, `ml_orchestrator.cpp:23,59`).
- **Important nuance the prior reports got right but under-stated:** the two reads are **not equivalent**. MlBridge reads the **live CF parameter** (user-settable); mainscreen reads the **never-written spec** (constant 6). So even in a one-front-end world, the widgets Ml path is pinned to 6 while the engine (`DeriveStageParams`, below) and the experiment honor the user value. The "two front-ends diverge" retraction (§2a) is correct as far as *front-ends* go, but the *divergence* between mainscreen's display/segment dilation and the engine's derived dilation is a single-front-end bug.

#### Corrected per-symbol production inventory

- **`getDilation`** (free fn, `objective_spec.cpp:5`): callers = the 7 mainscreen sites above. No other production caller. (`value_or(0)` at six sites; `value_or(3)` at 4848.)
- **`getAvailableCostFunctions`**: production = `settings_control.cpp:54` (constructs a whole **temporary** `CostFunctionManager()` just to enumerate — now a list of exactly one name) and `services/cost_function_registry.cpp:41` (live 3-manager registry build, `BuildCostFunctionRegistryEntries`; also `getIntParameters` at 63). Experimental = `SettingsBridge.cpp:536, 649`.
- **`setIntParameterValue`**: production = `mainscreen.cpp:4690` (session-load key registry), **4779/4781** (branch=4/leaf=1 defaults block, `getCostFunctionClass(CostFunctionType::DirectDilation)`), `settings_control.cpp:912` (int spinbox), **996/998** (reset defaults 4/1); impl `compute/CostFunction.cpp:43`. Note: the 4/1 defaults live in the CF **parameter**, and they reach the engine only via `DeriveStageParams` → `jta::DeriveStageCostParams` live-scan (`optimizer_manager.cpp:1229-1241` shim → `optimizer_stage_script.cpp:145-160`, which scans the copied int params, "Dilation/DILATION/dilation" last-match, clamp ≥0). The mainscreen displays and the segmentation path never see 4/1 — they see spec-6.
- **`setActiveCostFunction`**: production = `mainscreen.cpp:4641/4643/4645` (session-load ACTIVE_CF keys), `settings_control.cpp:551` (dialog selection); impl + ctor defaults `compute/CostFunctionManager.cpp:26, 54, 147`.
- **`updateCostFunctionParameterValues`**: **zero production callers.** Declared `include/compute/CostFunctionManager.h:63-71` (3 overloads), impl `.cpp:153/167/182`. Exhaustive repo grep: callers only in `test/oracle/` (oracle_test.cpp:239, bit_identity_test.cpp:214, layered_correctness_test.cpp:180, multistage_oracle_test.cpp:326 comment, throughput_serial_pipeline.cpp:161, z_profile_test.cpp comments). Confirms synthesis §2: DELETE stands; the by-value-map no-op class survives in production only as… nothing (the `applyCostFunctionEntries` manifestation was experimental-only, §2a demotion confirmed — `SettingsBridge.cpp:536-560` mutates the by-value map from `getAvailableCostFunctions()`, verified read).
- **`getCostFunctionClass`**: production = `mainscreen.cpp:4683, 4778, 4780`; `settings_control.cpp:995, 997`; impl `.cpp:215`. Experimental = `SettingsBridge.cpp:42, 46, 113, 117`.
- **`getActiveCostFunctionClass`**: production = `settings_control.cpp:552, 602, 886 (setDouble), 912 (setInt), 938/964 (setBool)`; `optimizer_manager.cpp:981, 1028, 1093, 1239-1240` (DeriveStageParams shim); impl `.cpp:210`; in-CFM self-reads inside cost TUs (`DIRECT_DILATION.cpp:34` — the only **reachable** one today; the others at `DIRECT_DILATION_T1.cpp:35`, `DIRECT_DILATION_SAME_Z.cpp:43,50`, `DIRECT_DILATION_POLE_CONSTRAINT.cpp:42,109`, `DD_NEW_POLE_CONSTRAINT.cpp:41,130-136`, `sym_trap_function.cpp:44,174-176` are compiled but unreachable given the 1-value enum). Experimental = `SettingsBridge.cpp:609, 627, 634`, `MlBridge.cpp:516`.
- **`getActiveCostFunction`** (type accessor, adjacent to the surface): production = `cost_function_registry.cpp:41`, mainscreen edge_params `3889/3968/4047/4101`, `optimizer_manager.cpp:1238`.
- **`getIntParameterValue`** (explicit): production = `optimizer_manager.cpp:982, 1029, 1094`; compute self-reads as above. **`getIntParameters`**: `settings_control.cpp:562, 616`; `cost_function_registry.cpp:63`; `optimizer_manager.cpp:1239`.
- **Domain**: zero hits for the entire symbol set (`src/domain/` does not touch CFM). `src/services/`: only `cost_function_registry.cpp` (read-only enumeration) — no `ml_orchestrator` CFM access.

**Net corrected statement of the P0:** user-set trunk Dilation reaches (a) the engine, via the live CF param through `DeriveStageCostParams`, and (b) the settings dialog and registry, but **never** reaches segmentation (1782), image loading (2508), edge processing (3881-4101), or dilation-frame display (4848), which all read the frozen spec-6. Single front-end; the divergence is between mainscreen and the engine, not between two front-ends.

---

### 3. "One production front-end" — confirmed end to end

- `src/view/mainscreen.cpp` mentions `SettingsBridge` exactly once, at **4831, inside a comment** ("which the QML SettingsBridge calls too"). No `#include` of any experimental header exists in `src/view/` or `src/coordinator/` (exhaustive grep for all seven bridge names + `app/experimental` + `jtml_experimental`: comments only, listed in §1).
- `src/coordinator/` mentions `OptimizerBridge` only in comments at `optimizer_run_controller.cpp:6,61` and `optimizer_run_controller_core.cpp:104` (provenance notes, e.g. "OptimizerBridge.cpp:276-291 preserved" — describing what logic was ported from where).
- Composition root `src/app/main.cpp`: `MainScreen w; w.show();` — the only production GUI entry. The `--settings-impl qml` flag (`main.cpp:23-45`) selects `SettingsImpl::RustQml` → `QmlSettingsDialog` at `mainscreen.cpp:2334-2338` (`src/view/qml/qml_settings_dialog.cpp`, in `jtml_view`'s source list, `src/view/CMakeLists.txt:29`) — a **production view-layer seam**, unrelated to `src/app/experimental/`; do not let that flag's existence muddy the "one front-end" claim.
- Production executables: `joint-track-machine-learning` (widgets) and `Study2Grid` (`src/app/Study2Grid`, no bridge references). `jtml_experimental` is the third binary, built (see §1) but consumed by nothing.

---

### Findings summary

- **Correct:** the owner rule holds — `src/app/experimental/` has zero production semantic reach (no production include, link, install, or call site; comments only). The 7 mainscreen `objective_spec` sites, the copy-assign-only write, the `updateCostFunctionParameterValues` dead/no-op class, and the SettingsBridge by-value-map no-op are all re-verified exactly as the reports state (with line-number drift on the optimizer_manager debug prints, now corrected above).
- **Finding (P2, report-only, build hygiene):** `jtml_experimental` compiles in the default `pixi run build` — release preset doesn't set `JTML_BUILD_EXPERIMENTAL` and the option defaults ON (`src/app/CMakeLists.txt:17-19`; cache `.build/CMakeCache.txt:611`; binary present in `.build/bin/`). Smallest fix: add `"JTML_BUILD_EXPERIMENTAL": false` to the release preset's cacheVariables (the profile preset already does, `CMakePresets.json:64`), or delete the subtree + option. Zero-risk; retires the experimental call-site surface from every future build.
- **Finding (P2, report-only, inventory correction):** prior "who reads objective_spec" lists omitted the production coordinator readers — `optimizer_manager.cpp:971/974, 1018/1021, 1083/1086` (debug spec prints) plus the commented swap read at `CostFunctionManager.cpp:259`. Relevant because the prepared `ObjectiveInstance` swap (the peel's target) would consume the frozen spec-6 and silently diverge from the live 4/1 branch/leaf parameter values if flipped before the registry→spec sync.
- **Finding (P2, report-only, stale-structure note for the reports):** the CFM registry/enum is now single-function (`CostFunction.h:21`; `listCostFunctions` `CostFunctionManager.cpp:295-298`; single-case switches 237-290). Any synthesis text implying multi-function selection or reachable non-Dilation CF paths in the current tree should be updated; the orphan cost-function TUs are compiled but unreachable.
- **Merge verdict:** OK (review-only pass; no code changed). No P0/P1 introduced by this follow-up — the P0s already on record (spec freeze; dead update family) are confirmed with corrected line numbers and the completed reader inventory above.