## Review — FOLLOW-UP D: Re-classify the experimental-adjacent tests

**Scope note:** read-only pass; no edits, no jj, no build. All claims verified by reading files; the owner rule ("everything in `src/app/experimental/` is purely experimental") was taken as given and cross-checked against what the tests actually compile.

---

### 1. The six `experimental_*_test.cpp` files — what they actually pin

First, the structural fact that frames every verdict: **every one of the six includes experimental headers directly** (verified: the only `#include "AppBridge.h"` / `ExperimentalScene.h` / etc. hits in all of `test/` are these six files plus `oracle/qml_parity_check.cpp` and `oracle/qml_render_smoke.cpp`). And the entire test tree is not built: root `CMakeLists.txt` has `add_subdirectory(src)`, `add_subdirectory(packaging)`, `add_subdirectory(rust)` only (root CMakeLists.txt:127–130) — no `add_subdirectory(test)`. Nothing below is running today.

| Test file | What it pins | Verdict |
|---|---|---|
| `experimental_optimizer_gate_test.cpp` | **Bridge policy only.** `OptimizerBridge::EvaluateGate` (static, experimental), the `SingleModelOnly` v1 pre-check (a QML policy, deliberately NOT in the controller — lines 17–20, 118–124), `run()` rejection message texts, `stop()` no-op, `buildGateInput` assembly. The shared core (gate, state machine, progress, seed) was already ported out: the header says so (lines 7–14) and `optimizer_run_controller_core_test.cpp` pins it. Includes `AppBridge/ExperimentalScene/ExperimentalSession/OptimizerBridge/StudyBridge` (lines 33–37) — cannot compile without the experiment. | **DELETE-WITH-EXPERIMENT.** Nothing here is production: the production controller core is pinned in `optimizer_run_controller_core_test.cpp`, and the single-model rejection is a bridge-only QML policy. |
| `experimental_selection_test.cpp` | StudyBridge load flow + error mapping + the `DelegateSelection` helper contract. The production surfaces it *reaches through* (SessionController, LocationStorage, Model/stl_reader, ModelListBuilder, calibration parsing, Frame/Model list models) each have their own direct tests: `session_controller_test.cpp` (calibration parse errors :99, image parse + counts :188/:257, model parse :315), `study_load_controller_test.cpp` (:137/:204/:238/:297/:366), `test_model_list_builder.cpp` (:428 target), `test_location_storage.cpp` (:291), `test_calibration.cpp` (:302). `DelegateSelection` itself lives in `src/app/experimental/DelegateSelection.*` — production widgets uses QItemSelectionModel instead, pinned by `lifecycle/list_models_test.cpp`. | **DELETE-WITH-EXPERIMENT.** All production behavior it touches is pinned elsewhere at the service seam. |
| `experimental_ml_bridge_test.cpp` | MlBridge path surface, env fallback (`JTML_SEG_PT`/`JTML_FEM_ESTIMATE_PT`), degradation state machine, kind-preferred segment picker, OptimizerBridge seed apply/one-shot/stale-frame guards, hub D3/D4 relays. All bridge/hub wiring. The **production ML logic** (`MlOrchestrator` in services — plain C++/OpenCV, injected `SegmentOp/EstimateOp/SavePoseFn` seams) is pinned torch-free by `ml_orchestrator_test.cpp` (:765 target; header lines 5–12: happy path + failure scenarios, .pt-load throw/empty legs). | **DELETE-WITH-EXPERIMENT.** The orchestrator-level pins survive; the bridge degradation/relay surface dies with the bridge. |
| `experimental_pose_bridge_test.cpp` | PoseBridge table model, copy-prev/next delegation to `pose_copy`, save/load guards + typed messages, round-trip, golden `.jtak` read, D3/D4 relays. Production analogs all pinned: `test_pose_file_io.cpp` (kinematics round-trip :61, NOT_OPTIMIZED :79, malformed :113, golden fixture :129, failure paths :159/:191), `pose_copy_test.cpp` (:609 target; the bridge test itself cites it — line 152: "pinned in pose_copy_test.cpp"), `test_location_storage.cpp`. | **DELETE-WITH-EXPERIMENT.** The one unique fixture — reading `test/golden/fem_oracle_captured.jtak` through the bridge (:396–401) — pins nothing production pose_file_io tests don't: the `JTA_EULER_KINEMATICS` format, NOT_OPTIMIZED handling, malformed-row skipping, and a real golden fixture are all pinned in `test_pose_file_io.cpp` (which uses `fem_golden.jts`, `test/CMakeLists.txt:110`). |
| `experimental_settings_test.cpp` | See §4 — this is the only one with a salvage question, and it is **worse than the prior reports claimed**: parts of it *cannot pass*. | **DELETE-WITH-EXPERIMENT** (salvage analysis below). |
| `experimental_file_dialog_test.cpp` | Pure `FileDialogBridge` MRU/directory-memory (whole file, 1 test case). FileDialogBridge is experimental-only; production widgets uses `QFileDialog` directly and has no analog. | **DELETE-WITH-EXPERIMENT.** Zero production surface. |

### 2. `test/qml/` — fakes only, exercise zero production code

- `qml/main.cpp` is 10 lines: `QUICK_TEST_MAIN(qml_view)`; its own header (lines 1–8): *"The C++ bridges are NOT linked: the fakes (test/qml/Fake*.qml, same-directory types) replace them."*
- The 5 `tst_*.qml` files instantiate the **experimental QML components** — `tests.qrc:31+` aliases `../../src/app/experimental/Theme.qml`, `StudyPanel.qml`, `MlStrip.qml`, `PosesTable.qml`, etc. into `qrc:/components`. `tst_StudyFlows.qml:12` imports only `qrc:/components` and drives `FakeAppBridge/FakeStudyBridge/FakeOptimizerBridge/FakeMlBridge/FakePoseBridge` (lines 17–21) plus inline fake picker/viewport (lines 25–37).
- The 6 `Fake*.qml` are `QtObject` stubs (verified `FakeAppBridge.qml` — properties + one signal, no logic).
- No production C++ is compiled or linked into `jtml_test_qml_view` (target: `qml/main.cpp` + `tests.qrc` only, `test/CMakeLists.txt:1596+` block).

**Verdict: DELETE-WITH-EXPERIMENT, entirely.** These are view-layer pins of the disposable QML app; they test fake bridges against experimental components.

### 3. CMake / lifecycle dependency inventory

Ten test targets reference `src/app/experimental` (complete list — `grep 'app/experimental' test/CMakeLists.txt` hits only these blocks):

1. `jtml_test_experimental_selection` (:792–848, headless)
2. `jtml_test_experimental_settings` (:861–890, headless)
3. `jtml_test_experimental_file_dialog` (:897–913, headless)
4. `jtml_test_experimental_optimizer_gate` (:1007–1060, headless)
5. `jtml_test_experimental_ml_bridge` (:1079–1136, headless)
6. `jtml_test_experimental_pose_bridge` (:1157–1198, headless)
7. `jtml_test_qml_parity_check` (:1380–1420 block, oracle label) — compiles AppBridge/MlBridge/PoseBridge/OptimizerBridge/SettingsBridge/StudyBridge/ExperimentalScene directly
8. `jtml_test_qml_render_smoke` (:1518–1560 block, oracle;render) — `QmlVtkRenderer` + `ExperimentalScene` + `renderer.qrc`
9. `jtml.qml_lint` (:1585 — `QML_DIR=${CMAKE_SOURCE_DIR}/src/app/experimental`)
10. `jtml_test_qml_view` (qml/tests.qrc aliases experimental QML, tail block)

If `src/app/experimental/` were deleted, **all ten targets fail to configure/build**; the six `headless`-labeled ones would break the default suite immediately. Everything else in `test/CMakeLists.txt` (37 other targets) references only `src/{domain,services,coordinator,compute,view}` + test sources — verified by the exhaustive grep above: the only `app/experimental` occurrences are in those ten blocks.

`test/lifecycle/` has exactly two files, both clean: `session_state_controller_test.cpp` includes only `coordinator/session_state_controller.h` (line 24); `list_models_test.cpp` includes only `view/frame_list_model.h` + `view/model_list_model.h` (lines 19–20). Neither touches the experiment.

**Stale-reference finding (P2):** the comment block at `test/CMakeLists.txt:465–488` describes a QtTest "Shared optimizer-run controller lifecycle tests" target with a FAKE `OptimizerRunDriver`, and `experimental_optimizer_gate_test.cpp:12–13` says shared cases "moved to … test/lifecycle/optimizer_run_controller_test.cpp" — **that file does not exist** (`test/lifecycle/` has only the two files above; no such target in CMake). Consequence: the `OptimizerRunController` *shell* (signal relays, by-value payloads, QTBUG-2842 re-emit behavior described in the comment) has **no direct test** — only the Qt-free core does. Worth knowing before deleting the bridge tests, since the shell was only ever exercised through the bridge test's link (it links `jtml_coordinator`), not pinned.

### 4. Does "KEEP as characterization" flip to DELETE? Yes — and the salvage is smaller than the prior reports implied.

The KEEP recommendation's premise was that these tests run and pin behavior until the subject dies. Both premises fail:

- They don't run at all (no `add_subdirectory(test)`), and the experiment is disposable by owner rule.
- **The settings suite is latent-failing as written.** Verified end-to-end: `CostFunctionManager::getAvailableCostFunctions()` returns **by value** (`src/compute/CostFunctionManager.cpp:199–202`); `SettingsBridge::applyCostFunctionEntries` iterates that copy and mutates `cost_function_it->second` (`SettingsBridge.cpp:536–560`) — a no-op on the manager, since `CostFunction` stores parameters in by-value member vectors (`CostFunction.cpp:30+`). `load()` calls this path (`SettingsBridge.cpp:84`). But `experimental_settings_test.cpp` case (b) asserts a *fresh* bridge after `load()` has the applied values (e.g. `loaded.trunkDilation() == 9`, which reads the manager's active-CF param — `SettingsBridge.cpp:416–418`), and case (c) asserts `75.123456789012345` bit-exact through `load()`. With the no-op bug, the fresh manager holds constructor defaults (Dilation=3 per the golden table) → **cases (b) and (c) would fail if built and run**. (The `ACTIVE_CF` leg works — it calls `manager->setActiveCostFunction` directly, `SettingsBridge.cpp:508–513` — so only the parameter entries are dead.) The prior finding is confirmed and sharpened: the no-op is not merely "unpinned" — the tests that appear to pin it pin a fiction. The production widgets apply path is *not* affected: `mainscreen.cpp:4682–4694` uses `getCostFunctionClass(...)` which returns `CostFunction*` into the manager's map (`CostFunctionManager.cpp:215–218`) — correct pointer mutation.

**What is worth salvaging before deletion: almost nothing, because production coverage already exists:**

- **Golden 51-entry registry parity fixture** (the `TRUNK@/BRANCH@/LEAF@…` table with the branch Dilation=4 / leaf Dilation=1 first-run overrides): already duplicated **verbatim** in the production test `test/unit/cost_function_registry_test.cpp:77–133`, with the same `requireEntryMatches` bit-exact comparator and the full widgets-save-path round-trip through `SaveCostFunctionSettings → LoadSettings` (its case (d), :232–282). Its header explicitly documents the copy (:22–27). No salvage needed — at most, when deleting, keep the capture-procedure paragraph in `experimental_settings_test.cpp:45–58` somewhere in docs if the provenance of the golden table matters.
- **Fractional-double bit-exact round-trip** (the Parameter<double> truncation-bug invariant): pinned at the production service level by the hegel PBT `settings_service_properties.cpp` (header :4–8; draws fractionals/negatives/±0.0 and asserts `got.toDouble() == expected` plus signbit, :44–48, :192–199). The experimental test's extra leg (through `setDoubleParameterValue` on a manager + `load()` back) is the leg that can't work anyway.
- **SettingsBridge save-path behavior**: nothing to keep. `SettingsBridge` is experimental-only; its `save()` is just `SaveCostFunctionSettings(BuildCostFunctionRegistryEntries(...))` (SettingsBridge.cpp:57–72) — both halves production-pinned (registry test case (d), `test_settings_service.cpp`, `settings_service_properties.cpp`). The widgets first-run/reset semantics it mirrors are pinned via the `FirstRunManagers` fixture in `cost_function_registry_test.cpp:169–181` (fresh managers + Dilation 4/1 overrides, citing mainscreen.cpp:4836).
- **`applyCostFunctionEntries` no-op**: do NOT salvage as a test — it pins experiment-only code. If anything, note it in `docs/solutions/` as the known divergence between the bridge's apply loop (by-value copy) and the widgets' correct pointer-based loop, so nobody transcribes the bridge version during the peel.

### 5. KEEP-AS-PIN set independence — confirmed

Verified by exhaustive `#include` grep across `test/` (only the 6 experimental tests + 2 oracle files include any experimental header) and by the exhaustive CMake grep (experimental paths appear only in the 10 target blocks listed in §3):

- `test_stage_script.cpp` / `test_stage_script_properties.cpp` — includes only `coordinator/optimizer_stage_script.h` (header, lines 5–13); target :362/:395 sources are domain/coordinator only. Clean.
- `test_metric_semantics.cpp` / `_properties.cpp` — targets :319/:349, no experimental references in their blocks. Clean.
- `optimizer_run_controller_core_test.cpp` — includes only `coordinator/optimizer_run_controller_core.h` (line 13); target :489–504 compiles `optimizer_run_controller_core.cpp` + `optimize_intent_controller.cpp` + `data_structures_6D.cpp`, links only Catch2. Clean.
- `session_state_controller_test.cpp` (lifecycle :517–537 and unit `unit/session_state_controller_test.cpp` :541–556) — coordinator/domain only. Clean.
- `save_last_pose_test.cpp` — pure services (`services/save_last_pose.h`); the QML bridge appears only in a comment (line 9). Clean.
- `ml_orchestrator_test.cpp`, `session_controller_test.cpp`, `study_load_controller_test.cpp`, `pose_copy_test.cpp`, `test_pose_file_io.cpp`, `settings_service*.cpp`, `cost_function_registry_test.cpp`, `lifecycle/list_models_test.cpp` — all clean.
- The hegel PBT targets (:34, :49) and all domain/compute tests — clean.

**No issue found in the KEEP-AS-PIN set: it depends on nothing in the experimental tree.**

---

## Summary

- **Correct:** the port out of the experiment was done properly — shared core (gate/state machine/progress/seed) → `optimizer_run_controller_core_test.cpp`; shared registry mapping + golden fixture → `cost_function_registry_test.cpp`; ML logic → `ml_orchestrator_test.cpp`; pose/kinematics format → `test_pose_file_io.cpp`; settings service → `test_settings_service.cpp` + PBT. The KEEP-AS-PIN set is fully independent of `src/app/experimental/`.
- **Finding (P2, evidence):** `experimental_settings_test.cpp` cases (b)/(c) assert `load()`-applied parameter values that `applyCostFunctionEntries` (by-value map, `SettingsBridge.cpp:536`) cannot deliver — the suite is latent-failing and does not pin the save-path it claims to. Location: `test/unit/experimental_settings_test.cpp:326–460` vs `src/app/experimental/SettingsBridge.cpp:498–567`. Smallest fix: none needed — the test dies with the experiment; record the bridge-vs-widgets apply-loop divergence in docs if desired.
- **Finding (P2, stale reference):** `test/CMakeLists.txt:465–488` and `experimental_optimizer_gate_test.cpp:12–13` reference a `test/lifecycle/optimizer_run_controller_test.cpp` (controller-shell QtTest with fake driver) that does not exist — the `OptimizerRunController` shell has no direct test; its only observers today are the bridge tests slated for deletion. Smallest fix: correct the comments, or write the shell test against the production controller before deleting the bridge tests, per the peel plan.
- **Verdict on the 6 tests + test/qml + 2 oracle targets + qml_lint:** DELETE-WITH-EXPERIMENT. Salvage before deletion: nothing functional (all duplicated in production tests); optionally preserve the golden-fixture capture-procedure note (`experimental_settings_test.cpp:45–58`) into `docs/` or the `cost_function_registry_test.cpp` header it's already summarized in.

**Merge verdict: OK** (review-only; no code change requested or made).