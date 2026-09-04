## Review — Seam 6: Test trust audit

Tool note: no `structured_output` tool exists in this subagent's toolset (read/grep/find/ls/contact_supervisor only); returning the complete artifact here per coordination rules.

### P0 (blocks trust in "the suite")

**P0 — test/CMakeLists.txt is entirely unreachable; zero tests are registered by any build.**
- Evidence: root `CMakeLists.txt:42` declares `option(JTML_BUILD_TESTS ... OFF)`, but there is **no `add_subdirectory(test)` anywhere in the root CMakeLists** (grep: only `add_subdirectory(src)` :127, `packaging` :129, `rust` :130). `CMakePresets.json:63` sets `JTML_BUILD_TESTS: false` only for the `profile` preset; the `release` preset (:38-46) doesn't set it. The option has **no consumer** — it's a dead flag.
- So `test/CMakeLists.txt` does declare ~60 `add_test()`s, but none of them is ever configured, built, or run. The brief's "registers 0 tests" is correct; root cause is the missing `add_subdirectory(test)`, not merely the OFF default. Nothing under test/ is green — there is no suite.

### File inventory correction
Actual counts: `test/unit` 57 files (incl. helper `frame_headless.cpp`), `test/lifecycle` 2, `test/oracle` 19 (.cpp/.cu, incl. `throughput_serial_pipeline.h` helper), `test/qml` 5 `tst_*.qml` + `main.cpp` + 6 fakes. Brief's "75" is approximately right.

### Per-file classification

**Bucket (a): legacy doomed / will not compile — all UNREGISTERED orphans referencing deleted headers. DELETE.**

Verified deleted headers (find across `include/compute/`): `graph_recipe.h`, `graph_key_assembler.h`, `evaluation_executor.h`, `evaluation_context.h`, `bank_state.h`, `cost_capacity_service.cuh`, `graph_preflight.h`, `graph_admission_policy.h`, `graph_recipe_direct_dilation.h` are all gone. Only `batch_outcome.h` and `capture_coordinator.h` survive.

unit/ (10 files):
| File | Evidence | Verdict |
|---|---|---|
| test_bank_binding_api.cpp | static_asserts on `RenderEngine::SetActiveBank(BankState*)` (:21-25); BankState deleted | DELETE |
| test_bank_state.cpp | `BankState`/`bank_state_math` (header :7-8) deleted | DELETE |
| evaluation_context_test.cpp | `EvaluationContext`/`Pool` (:12-13) deleted | DELETE |
| evaluation_context_lease_test.cpp | same (:8-9) | DELETE |
| graph_key_assembler_test.cpp | `compute/graph_key_assembler.h` :9 deleted | DELETE |
| graph_recipe_preflight_test.cpp | subject `graph_preflight.h` deleted | DELETE |
| hook_feeder_test.cpp | `evaluation_executor.h` :12 deleted (only `batch_outcome.h` survives) | DELETE |
| test_cost_capacity_service.cpp | `cost_capacity_service.cuh` :15 deleted | DELETE |
| test_direct_optimizer_batch.cpp | `evaluation_executor.h` :19 + `graph_admission_policy.h` :21 deleted | DELETE (see P2 dead-seam note) |
| capture_coordinator_test.cpp | orphan test of `CaptureCoordinator` — subject **alive** (`src/compute/CMakeLists.txt:37`) but **zero production consumers** (grep src: self-references only) | DELETE test + module (P2) |

oracle/ (12 files): `bit_identity_test.cpp` (:41 `evaluation_executor.h`, :45 `graph_recipe.h`), `cost_capacity_oracle_test.cu` (:25), `evaluation_executor_graph_test.cu`, `evaluation_executor_oracle_test.cu`, `graph_capture_probe_test.cu`, `graph_recipe_direct_dilation_test.cu`, `graph_throughput_oracle_test.cu`, `layered_correctness_test.cpp` (:38-43), `multistage_oracle_test.cpp` (:91), `throughput_serial_pipeline.cpp/.h` (:8,:12), `u4_production_integration_test.cu` (:35 `graph_preflight.h`) — all include deleted headers. `graph_free_overlap_poc_test.cpp` compiles in principle but is the measured-host-bound graph PoC. **DELETE all.** Also prune the graph-era golden artifacts when deleted: `test/golden/graph_performance_baseline.json`, `graph_layer_verdict.json`, `graph_pre_registration.json`, `bit_identity_baseline.txt`, `oracle_multistage*.json`.

**Bucket (b): still-true pure-logic pins (subject alive) — KEEP-AS-PIN.** All registered in `test/CMakeLists.txt`:
- **StageScript/DeriveStageCostParams transcription pins** — `test_stage_script.cpp` (pins the exact `[{Trunk,20000,1},{Branch,5000,2},{Leaf,5000,1}]` sequence, the Sym_Trap leaf-only repeat=0 shape, last-match-wins/clamps/DIRECT_MAHFOZ->3 scan) and `test_stage_script_properties.cpp` (hegel: budget-sum/order/determinism). Subject live: `src/coordinator/optimizer_stage_script.cpp`. These are the migration anchor for seam 3.
- **test_metric_semantics.cpp / _properties.cpp** — kernel-as-spec CPU references + the real-CFM `getStage()` pin (:850-855); CFM alive, KEEP-AS-PIN until CFM dies.
- Pure domain/services: `test_direct_optimizer.cpp`(+`_properties`), `test_data_structures.cpp`, `test_direct_data_storage_properties.cpp`, `test_cost_function.cpp`(+`_properties`), `test_sym_trap_functions.cpp`(+props), `test_ambiguous_pose_processing.cpp`(+props), `test_session_state.cpp`(+props), `test_pose_file_io.cpp`(+props), `test_model_list_builder.cpp`(+props), `test_location_storage.cpp`(+props), `test_calibration.cpp`(+props), `test_optimize_intent_controller.cpp`, `test_hegel_smoke.cpp`, `test_harness_smoke.cpp`, `save_last_pose_test.cpp`, `cost_function_registry_test.cpp` (51-entry golden table), `pose_copy_test.cpp`(+props), `settings_service_test.cpp`(+props), `edge_processor_test.cpp`(+props), `session_controller_test.cpp`, `study_load_controller_test.cpp`, `ml_orchestrator_test.cpp`.
- **Coordinator core pins:** `optimizer_run_controller_core_test.cpp` (Qt-free gate/state-machine/epoch/seed — subject live), `session_state_controller_test.cpp` (unit + lifecycle QtTest), `list_models_test.cpp`.

**Bucket (c): relay/bridge characterization (subject alive, doomed by the peel) — KEEP as characterization until the subject dies, then delete with it.**
- `experimental_optimizer_gate_test.cpp`, `experimental_selection_test.cpp`, `experimental_ml_bridge_test.cpp`, `experimental_pose_bridge_test.cpp`, `experimental_settings_test.cpp`, `experimental_file_dialog_test.cpp` — pin the 5-layer relay pipeline (brief finding 3). **Gap:** none of them pins `SettingsBridge::applyCostFunctionEntries`'s by-value-copy no-op (verified finding 1) — the save-path round-trip (`experimental_settings_test.cpp:458ff`) works, so the load-side bug is unpinned. Fold into the fresh SettingsBridge pin.
- Oracle instruments (label `oracle`, never a gate — per brief): `oracle_test.cpp`, `z_profile_test.cpp`, `segmentation_oracle_test.cpp`, `render_smoke.cpp`, `probe_vtk.cpp`, `qml_render_smoke.cpp`, `qml_parity_check.cpp` — KEEP as instruments.
- QML view tests (`qml/main.cpp`, 5 `tst_*.qml`, 6 Fake*.qml) — fake bridges decouple them from C++; in the target shape QML is a client. KEEP (they survive the coordinator peel).

### Minimal FRESH pins the new seams need
1. **Seam 1 — objective parity probe:** A/B same pose grid, legacy `jta::BuildGpuCostAdapter`-bound CFM vs `ObjectiveInstance::evaluate(pose)`, bit-identical score + emit order; also the biplane-B gap (currently stubbed) once unified. **Location:** `test/oracle/objective_parity_test.cpp`, new target `jtml.objective_parity`, label `oracle;gpu` (GPU instrument, like `jtml.z_profile`), **Catch2**.
2. **Seam 3 — declarative script build:** when `BuildStageScript`'s settings-scan input becomes a declarative registration definition: Catch2 deterministic pins for the exact 20k/25k/30k script + Sym_Trap leaf-only + error paths (extend `test_stage_script.cpp` in place), hegel PBT for budget-accumulation/order/determinism invariants (extend `test_stage_script_properties.cpp`). Both already live and are the template.
3. **Seam 2 — bundle construction:** runtime/binder instantiates exactly the resources the upcoming stages need, once, passed by reference; ctor/dtor pairing must reproduce the error-gating asymmetry (trunk destruct unconditional; leaf gated on `!error_occurred_`). **Peel the gate decision to pure logic first**, then Catch2 + hegel PBT on the pure decision; QtTest only if the binder is a QObject seam.
4. **Seam 5 — coordinator request shape:** `jtml.optimizer_run_controller_core` already covers the Qt-free core — KEEP it. Fresh pin needed only for the view→`OptimizerRunRequest` field mapping as a Catch2 direct-compile (replaces the 4-by-value-copy relay pins when the copies die). The QSignalSpy-on-main-thread rule (QTBUG-2842) still applies to any lifecycle QtTest.
5. **Cross-cutting bit-identity pin:** replacement for the deleted `bit_identity_test.cpp` — headless capture of per-pose score sequences (legacy stage loop vs replacement), diffed byte-exact. `test/oracle/`, label `oracle;gpu`.

### Confirmations requested
- **AUTOMOC gotcha note:** CONFIRMED in `test/CMakeLists.txt` — explicit Q_OBJECT headers in `add_executable` sources at `jtml_test_session_state_controller` ("Q_OBJECT header in the source list (AUTOMOC gotcha)"), `jtml_test_list_models` ("Q_OBJECT headers go in the add_executable source list (AUTOMOC gotcha)"), `jtml_test_experimental_file_dialog` ("Q_OBJECT header listed for AUTOMOC (repo gotcha)"), and the commented-out `jtml_test_coordinator` block (:~449-457) lists `include/coordinator/optimize_coordinator.h` explicitly.
- **hegel in pixi.toml:** NO. `pixi.toml` deps have `catch2 = ">=3.16.0,<4"` (:~39) but no hegel; hegel arrives via CMake `FetchContent` (`test/CMakeLists.txt:26-35`, GIT_TAG v0.11.4, with the `dl` + rpath workarounds documented). Consequently, since `test/` is never configured, the FetchContent never even runs today.

### Merge verdict: OK with notes
The audit itself is the deliverable; no edits made (read-only). Two decisions to surface to the parent before deletion work: (1) whether `DirectOptimizer::SetBatchCost` (`include/domain/direct_optimizer.h:152-156`, defined at `src/domain/direct_optimizer.cpp:190`, zero production callers) survives the peel — its only test cannot compile; (2) whether `capture_coordinator.{h,cpp}` (zero consumers) is deleted alongside its orphan test. Supervisor commands required (I ran none per rules): none strictly needed to verify; to actually act on the verdicts the parent must wire `add_subdirectory(test)` behind `JTML_BUILD_TESTS` (or delete the option), delete the 22 orphan test files + graph-era golden artifacts, and re-verify via `pixi run configure && pixi run build && pixi run test` (currently `pixi.toml:131-134` runs `ctest -L headless` against a build that contains no tests).