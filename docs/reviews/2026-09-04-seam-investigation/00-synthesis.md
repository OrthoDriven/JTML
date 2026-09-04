# JTML seam investigation — cross-seam synthesis (2026-09-04)

Fresh-eyes 6-agent swarm over the explicit-inputs peel. Full seam reports:
`01-objective-lifecycle.md` … `06-test-trust-audit.md` (same directory). All evidence
re-verified against source; line numbers in this file are spot-checked. No code was changed.

## 0. Headline

The owner's target shape is **feasible with zero new wrapping layers** — every seam
converged on "reshape existing objects, pass resources by reference, delete ambient
state." But the swarm surfaced **two live P0 correctness bugs that block the prepared
swap path** and a cluster of dead-but-leaking resources. Fix Phase 0 before any swap.

## 1. The one bug everyone found (P0): `objective_spec` is a never-written mirror

- Declared `CostFunctionManager.h:132`; the only write is copy-assignment
  (`CostFunctionManager.cpp:72`). Every CFM holds `DirectDilationSpec{dilation=6}` forever.
- **7 widgets UI reads are pinned to constant 6** (mainscreen.cpp:1782, 2508, 3881, 3960,
  4039, 4093, 4848 — segment, load-image, 4 edge handlers, UpdateDilationFrames): user-set
  trunk Dilation silently does not reach segmentation. QML reads the live CF param
  (SettingsBridge.cpp:606-616, MlBridge.cpp:517) → **the two front-ends diverge**.
- The engine's own A/B debug prints expose it live ("ObjectiveSpec dilation: 6" vs
  "Legacy dilation: <user>", optimizer_manager.cpp:1013-1030, 1060-1077, 1126-1140).
- **Swap blocker:** the prepared (commented) CFM internal swap reads
  `std::get<DirectDilationSpec>(objective_spec)` (CostFunctionManager.cpp:268) — flipping
  it today would run branch/leaf at dilation 6 instead of 4/1: **silent bit-divergence**.
- Fix: single registry→spec sync at the write path (`setIntParameterValue` + session load);
  OR revert the 7 mainscreen reads to the CF param until spec is wired. Probe: set trunk
  Dilation=9, resegment, expect 9.

## 2. Second P0: `applyCostFunctionEntries` + `updateCostFunctionParameterValues` no-op class

- SettingsBridge.cpp:536-560 mutates the by-value map from `getAvailableCostFunctions()`;
  QML session-restore drops all parameter writes. Fix = mutate via
  `getCostFunctionClass(type)` pointers (mirrors widgets load, mainscreen.cpp:4700-4712).
- `updateCostFunctionParameterValues` (3 overloads, CostFunctionManager.cpp:162-204) is
  the same no-op class AND dead in production (test/oracle-only callers). DELETE.

## 2a. AMENDMENT (2026-09-04, post-report): `src/app/experimental/` is NOT production

The owner confirmed the QML experiment — every file under `src/app/experimental/` — is
irrelevant to the production system. Verified: all four bridges (SettingsBridge,
OptimizerBridge, MlBridge, AppBridge) live there; the production coordinator references
them only in comments. Production clients are exactly: `src/view/` (mainscreen,
settings_control), `src/coordinator/`, `src/services/`, `src/compute/`, `src/domain/`.
Reclassifications (a follow-up 4-agent pass is refining these; deltas land in §7):

- **P0-2 demotes to DELETE-WITH-EXPERIMENT**: the only production manifestation of the
  `applyCostFunctionEntries` no-op lived in SettingsBridge. The
  `updateCostFunctionParameterValues` DELETE stands regardless (dead; oracle-only callers).
- **The "two front-ends diverge" sharpening of P0-1 is retracted** — there is ONE
  production front-end (widgets). P0-1 still stands on its own: user-set Dilation never
  reaches widgets segmentation (7 mainscreen.cpp reads of the never-written spec).
- **Seam 4 inventory shrinks**: `MlBridge.cpp:515` was experimental and drops from the
  production dilation-read inventory; `ml_orchestrator` (production) is being re-checked.
- **Default-config duplication shrinks**: 2 of 4 sites (SettingsBridge ctor + reset)
  leave with the experiment; the factory now covers mainscreen + settings_control only.
- **Seam 5**: the QML-specific evaporate list (5-hop seed relay, QML double gate,
  SettingsBridge-as-ViewModel) reclassifies PEEL → DELETE-WITH-EXPERIMENT. The seed
  core/shell apparatus REMAINS load-bearing: widgets uses it (mainscreen.cpp:146
  `clearSeedPose()`; plan-006-U8 SavePose→seed wiring).
- **Seam 5 blast radius shrinks**: OptimizerBridge launch-fill drops; the sole
  production launch-fill site is mainscreen.cpp:4286-4296.
- **Seam 6**: the 6 `experimental_*_test.cpp` pins are being re-classified in the
  follow-up pass (pins of the experiment itself vs pins of the production core reached
  through it).
- **Build wiring (verified directly)**: the experimental QML app is gated by
  `JTML_BUILD_EXPERIMENTAL` (src/app/CMakeLists.txt:21-24, default **ON**), but the
  `release` preset — the one `pixi run configure` uses — does NOT set it, so
  `jtml_experimental` is compiled in every production build today
  (CMakePresets.json:37-46 vs profile :64 turning it off). Step 0.5 of the peel is
  therefore: set `JTML_BUILD_EXPERIMENTAL=false` in the release preset (or delete the
  subtree + option) — a zero-risk build hygiene step that also retires every
  experimental call-site concern in one move.
  through it).

## 3. Unified ownership / lifetime model (the answer to "who owns what")

| Object                    | Lifetime            | Owner                                                                                                                                                                                   | Notes                                                                                                                                                                                                                                                                                                                                                |
|---------------------------|---------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `ResourceBundle`          | per run             | Run-scoped binder inside the fresh `OptimizerManager` (binder = reshaped `Initialize`; construction at the only ctor site `optimizer_run_driver.cpp:31`, Initialize `driver.cpp:66-83`) | Stages get read-only refs. NOT session-scoped: stage variants bake in run params; session scope would need in-place re-upload (the dead C7/epoch problem again). NOT per-stage: frame index is execution state, not a resource.                                                                                                                      |
| Bundle fields             | —                   | —                                                                                                                                                                                       | `GPUModel* principal; vector<GPUModel*> non_principal; GPUMetrics* metrics; PoseMatrix* pose_storage; bool biplane; 3× {dilated_A/B, intensity_A/B vectors}`. Per-stage **variants are per-stage config** (dilation/dark_silhouette baked at upload, `optimizer_manager.cpp:305-580`), never mutated mid-run; all 6 vectors alive for the whole run. |
| `ObjectiveInstance`       | per (stage × frame) | Stage executor (the `Optimize()` loop's replacement): `std::optional<ObjectiveInstance>`; **ctor = Initialize, dtor = Destruct**                                                        | Ctor args explicit: `(spec, principal_model*, dilated_A[frame], metrics*, dilated_B[frame]                                                                                                                                                                                                                                                           | nullptr)`. Bit-identity conditions: (1) one live instance per stage at a time; (2) evaluate re-renders before metric (it does, direct_dilation.cpp:27-29); (3) spec.dilation == registry Dilation (currently VIOLATED — §1). |
| Config (dilation, params) | run snapshot        | One spec source of truth + `MakeDefaultStageConfigs()` factory (services; kills the 4× default duplication)                                                                             | Registry→spec sync at write path; `DeriveStageCostParams` = parity shim until probes pass, then DELETE.                                                                                                                                                                                                                                              |
| Optimizer                 | unchanged seam      | `DirectOptimizer` (domain)                                                                                                                                                              | Per-stage selection = `DirectOptimizer::Options` on `StageSpec` (already the variant slot; Rust path already derives `use_bobyqa` from stage kind, optimizer_manager.cpp:1352-1353). `direct_options_` member → PEEL. Callbacks stay at the construction site (caller's outputs).                                                                    |
| Run shape                 | —                   | `BuildStageScript` stays the single live source until a declarative graph replaces it                                                                                                   | StageSpec grows `ObjectiveSpec objective` + bundle id; `cfm_index` retires (the 3 CFMs differ ONLY in which per-stage frame vectors they point at — optimizer_manager.cpp:777-815).                                                                                                                                                                  |
| Thread + relays           | —                   | Controller shell                                                                                                                                                                        | QTBUG-2842 chain is load-bearing; shell/core split survives; driver seam = bundle injection point.                                                                                                                                                                                                                                                   |
|                           |                     |                                                                                                                                                                                         |                                                                                                                                                                                                                                                                                                                                                      |

## 4. Total peel order (risk per step; oracle/golden NOT a gate — fresh probes only)

**Phase 0 — unblock (do before any swap)**
1. Fix `applyCostFunctionEntries` (getCostFunctionClass mutation) — *low*. Probe: QML
   session-restore round-trip sets Dilation=9, read back = 9.
2. DELETE `updateCostFunctionParameterValues` ×3 (dead) — *low*.
3. Fix the `objective_spec` sync (or revert the 7 mainscreen reads) — *medium* (restores
   intended behavior; segmentation output changes for users with non-default Dilation).
   Probe: Dilation=9 → segment → 9.
4. DELETE inert dead code, each verified no-reader: named-graph registry decls
   (`optimizer_stage_script.h:144-168` — functions **declared, never defined**; phantom API),
   upload-epoch machinery (BumpUploadEpoch callers optimizer_manager.cpp:1308-1310,
   getUploadEpoch zero production callers), `prin_dist_`, `stage_`, **edge-frame vectors
   (write-only)**, **distance maps + heatmaps (unread AND leaked — never freed in
   ~OptimizerManager 1524-1588)**, engine debug spec prints, `value_or(3)` fallback +
   stale MAHFOUZ comment. *Low* (no read path exists); probe: monoplane score parity
   before/after each deletion.
5. Stage-ladder single-source (delete mainscreen.cpp:4536-4556 recompute; consume
   controller's label) — also kills the widgets divide-by-zero UB (mainscreen.cpp:4545 vs
   core clamp controller_core.cpp:46). *Low*.

**Phase 1 — bundle + adapter swap (the bit-identity core)**
6. Build `ResourceBundle` in the binder; `BuildGpuCostAdapter`/`RunDirectStage` take it by
   ref. GPU-side unchanged. *Low-med.* Probe: byte-exact per-pose scores.
7. **A/B parity probe** (one instrument, oracle-labeled, `test/oracle/objective_parity_test.cpp`):
   monoplane-gated (biplane B is stubbed; legacy adds `dist_score²` unconditionally, so
   A-only parity is unreachable in biplane). Pose set: stage start, post-run optimum,
   full sym-trap pose list verbatim, ~16-pose range grid. One throwaway eval per leg
   (CUDA context + scratch first-touch); capture initialize() return + error_message
   parity per (stage, frame); `memcpy` doubles to uint64, require 100% identical.
   Gate: probe passes **and** one full scripted monoplane run bit-identical
   (budgets 20k/25k/30k, emit order, per-repeat re-seed from CURRENT optimum). *High*.
8. Swap the adapter lambda → executor-owned `ObjectiveInstance`. Prereqs: §1 sync landed;
   **F2 decision**: `CalculateSymTrap` runs after leaf-init failure and scores with
   process-wide shared globals (DIRECT_DILATIONCustomVariables.h:25-27 — header-defined,
   ODR-hazardous; correctness by sequencing alone). Transitional executor needs an
   explicit guard with a defined fallback — do NOT crash, do NOT silently change. *High*.
9. Stage-loop gates onto spec fields: drop only the **tautological** `enable_branch_`/
   `enable_leaf_` conjuncts (scripts contain a Branch spec iff enabled, so while
   BuildStageScript is the source they are always true) and the trunk budget reset
   (→ `trunk_spec.budget`); **KEEP verbatim**: `!error_occurrred_` everywhere,
   `!sym_trap_call`, `spec.repeat > 0`, trunk-destruct-UNCONDITIONAL / **branch-never-
   destructs** / leaf-destruct-gated. Any "symmetrizing fix" of the asymmetry breaks the
   transcription. *Medium.* `CumulativeStageCaps` stays as the test-only executable pin
   of the cumulative budget shape.

**Phase 2 — config decoupling**
10. `MakeDefaultStageConfigs()` factory (services; also writes specs from birth);
    `getAvailableCostFunctions` → const ref (prereq: constify pure getters — mechanical);
    SettingsBridge dilation trio → `getIntParameterValue("Dilation", v)`; DELETE the
    3 no-op overloads if not already (step 2). *Low-med.*
11. StageSpec carries `ObjectiveSpec` + bundle id; retire `cfm_index` +
    `EvaluateCostFunctionAtPoint`'s stage switch; `DeriveStageCostParams` → shim → DELETE.
    *Medium.*

**Phase 3 — coordinator teardown**
12. Directive string layer: DELETE (string + compares + 5 locals); enum KEEP. *Low.*
13. Launch struct split: immutable dataset by const ref/shared_ptr + 4-field intent
    (drop hop-2 full copy at controller.cpp:58 and hop-3 storage copy NOW). *Medium* —
    lifetime moves to the bundle (shared_ptr), NOT to the UI freeze that currently makes
    const refs "safe".
14. Seed pipeline → explicit run input (snapshot/restore + 5-hop relay evaporate);
    delete ghost-thread quirk + M6 bind + 8 binds + H1 guard (they defend each other —
    remove as one unit); delete QML pre-check gate. Switch bindManager to
    pointer-to-member connects **in the same change** (string connects fail silently at
    runtime). *Medium-high.*

**Phase 4 — test tree**
15. `add_subdirectory(test)` behind `JTML_BUILD_TESTS` (root CMakeLists has NO consumer
    of the option today — the suite was never configured, not merely OFF); DELETE 22
    orphan files (10 unit + 12 oracle, all include deleted graph-era headers) + graph-era
    goldens; rebuild the surviving pins. hegel is FetchContent (v0.11.4), not pixi.
    *Medium.* Open decisions: `DirectOptimizer::SetBatchCost` (zero production callers)
    and `capture_coordinator` (zero consumers) — delete with their orphan tests.

## 5. Cross-seam conflicts (flagged)

1. **Seam 1 (prepared CFM-internal swap) vs Seam 2 (bundle):** the commented CFM swap
   reads per-CFM members; the bundle model keys resources by `cfm_index` in ONE shared
   bundle. Resolved in favor of the bundle ("same lambda, capture `ObjectiveInstance&`
   instead of `CostFunctionManager&`") — but **do not flip the commented code as-is**:
   it inherits the §1 dilation bug verbatim.
2. **Seam 1 vs Seam 3/5 (F2 fallback):** seam 1 demands exact reproduction of
   CalculateSymTrap-after-leaf-init-error; seams 2/3 have no instance live on that path.
   Decision required: transitional warm-globals guard until CFM dies, or documented
   defined-fallback value. Flagged to the owner — it's a behavior-vs-cleanup call.
3. **Seam 2 vs Seam 5:** bundle is per-run inside the manager, but seam 5's copy-chain
   teardown wants `shared_ptr` dataset hops NOW. Sequencing: hop-2/hop-3 intent-split
   can land in Phase 0/3; hop-4's single-move-from-bundle must wait for Phase 1. The
   lifetime must move to the bundle, not the UI `DisableAll` freeze.
4. **Seam 3 vs Seam 4 (spec ownership):** both make `objective_spec` the replacement
   key — consistent, but the write path must be SINGLE (setIntParameterValue + load).
   Seam 4's P0-1 fix is the same change seam 3 needs before spec-carried migration.
5. **Seam 3 vs Seam 6:** `CumulativeStageCaps` is KEEP (test-only pin) but lives in
   production `optimizer_stage_script.cpp`; and the deleted registry functions are
   declared in the same header the surviving tests pin — deleting the decls requires
   touching `test_stage_script.cpp:410-452` in the same change.
6. **Seam 1 probe vs Seam 6 pins:** they are the same instrument — converge
   `objective_parity_test.cpp` with the cross-cutting bit-identity pin (one oracle-labeled
   GPU target, Catch2, label `oracle;gpu`).

## 6. Verification inventory (fresh probes that gate the whole peel)

1. Objective A/B parity (monoplane, 100% bitwise, cold-start covered) — seam 1.
2. Full scripted-run bit-identity (budgets, emit order, re-seed, asymmetries) — seam 1/3.
3. Score parity after each dead-resource deletion — seam 2.
4. Dilation=9 → segment → 9 (spec sync) — seam 4.
5. Session-restore round-trip via getCostFunctionClass mutation — seam 4.
6. Stage-label equality probe (widgets ladder vs controller label) — seam 5.
7. Declarative-graph script pins: extend `test_stage_script(.properties).cpp` — seam 3/6.
8. Bundle-construction pin (needs-only, once, by reference) — seam 2/6.

---

## 7. Follow-up pass results (2026-09-04, §2a refined — reports 07-10)

Four one-client re-audits after the experimental rule. Net effect: **the peel gets
smaller** — a large QML-only slice of the coordinator/config/test surface is already
inert today and classifies as plain DELETE, not PEEL.

### Corrected facts (line drift + inventory completion)

- `objective_spec` is declared at `CostFunctionManager.h:128` (not :132); the sole write
  remains copy-assign `CostFunctionManager.cpp:72`; the dormant prepared-swap read is the
  commented `CostFunctionManager.cpp:259`.
- **Total dead spec reads: 10** — 7 view sites (mainscreen.cpp:1782, 2508, 3881, 3960,
  4039, 4093, 4848) **+ 3 coordinator diagnostic pairs** (optimizer_manager.cpp:971/974,
  1018/1021, 1083/1086 — corrected lines; prior synthesis cited 1013-1030/1060-1077/
  1126-1140, same code). All read constant 6.
- **Mechanism correction (seam 4):** the 7 mainscreen reads are `getDilation(objective_spec)`
  spec reads, NOT `getActiveCostFunctionClass()->getIntParameters()` scans. The fix lands
  as spec-sync at the **production write paths** — `settings_control.cpp:912` (the only
  sink for user Dilation edits) and the registry load `mainscreen.cpp:4690` — not at any
  bridge.
- **P1 restated (single front-end):** user-set Dilation reaches the engine (via the live
  CF param through the `DeriveStageCostParams` scan — the one real production scan, 6
  invocations: pre-run optimizer_manager.cpp:279/284/288, per-stage 992/1043/1107) and the
  settings dialog, but NEVER reaches segmentation (1782, feeding the ML segment tail),
  image loading (2508), edge processing (3881-4101), or dilation-frame display (4848).
  The divergence is mainscreen-vs-engine, not front-end-vs-front-end.
- Registry is single-function: `COST_FUNCTION_TYPE_LIST(X) X(DirectDilation)`
  (CostFunction.h:21); single-case dispatch switches (CostFunctionManager.cpp:237-290);
  the 6 other cost TUs are compiled but unreachable (confirms seam-1 F5 deletion).
- `MlOrchestrator` (production, src/services/) reads **nothing ambient** — fully
  explicit-inputs (SegmentFrame/EstimateFrame take dilation/ops as parameters);
  `src/services/` adds zero ambient reads. `session_controller.cpp:56/58` are dead
  locals (P2 delete), not CFM reads.
- CFM member copies land at optimizer_manager.cpp:116-118 (not :132-135).
- `jtml_experimental` confirmed built in the default release flow with hard evidence
  (.build/CMakeCache.txt:611 = ON; .build/bin/jtml_experimental exists); nothing in
  production links/includes it (comments only). Production write path is NOT affected by
  the SettingsBridge by-value bug (mainscreen.cpp:4683 mutates via pointer).

### Reclassifications (PEEL → DELETE-WITH-EXPERIMENT or plain DELETE)

- **Seed pipeline (seam 5, biggest delta):** the widgets path NEVER uses the controller
  seed — `setSeedPose` has ZERO production call sites (only `clearSeedPose`,
  mainscreen.cpp:146); the estimate reaches the run purely via the LocationStorage copy
  (SavePose → launch.pose_matrix → refresh controller.cpp:118-125 → Initialize). So the
  ENTIRE controller/core seed surface (takeSeedForRun, stale guards, shell
  snapshot/restore, seedApplied/seedRestored), the `clear_seed` hook +
  `ResetForDatasetClear` (zero production callers; only StudyBridge.cpp:247), the
  mainscreen lambda at :146, and `MlEstimateOutcome::seed` (consumed only by MlBridge)
  are **DELETE now** — behaviorally inert for widgets. Keep: `storage_` + terminal-frame
  SavePose (controller.cpp:302-311) + the P1-2 payload refresh.
- **QML-only coordinator surface:** core progress machine (`StageLabel`/`refreshProgress`/
  `budgets_`/`BudgetsFromSettings`), `runStateChanged`/`progressChanged` + 6 read
  accessors, `Severity::Info` + its view branch, the H2 `previous==current` gate check
  (tautology), the "pinned QML semantics" Error-preservation in onTerminalFrame, the
  `StopOptimizer` signal bind (collapsible into `driver_->Stop()`). With one client,
  option (b) from seam 5 stands: delete the widgets ladder, consume the core's label via
  `updateDisplayRelayed` (fixes the divide-by-zero UB and the double compute together),
  then the core progress machine becomes the single source — or invert. Decide at
  implementation time; either way only ONE stage-label computation survives.
- **Tests (seam 6):** all 6 `experimental_*_test.cpp` + `test/qml/` (fakes only, zero
  production code) + `qml_parity_check` + `qml_render_smoke` + `qml_lint` (10 CMake
  target blocks reference experimental — they all fail to configure if the subtree is
  deleted) → **DELETE-WITH-EXPERIMENT**, unconditionally. Salvage: none needed — the
  golden 51-entry registry fixture is already duplicated verbatim in
  `cost_function_registry_test.cpp:77-133`, and the fractional-double invariant is
  pinned by `settings_service_properties.cpp`. Bonus finding: `experimental_settings_test`
  cases (b)/(c) are **latent-failing as written** — they assert the no-op
  `applyCostFunctionEntries` actually applied (pins a fiction). The KEEP-AS-PIN set is
  confirmed fully independent of the experimental tree.
- **P2, decision needed:** `optimizer_launch_slot()` is never connected (mainscreen.h:388,
  mainscreen.cpp:1522-1527, no connect, no .ui connection) → `Directive::SymTrap` is
  production-UNREACHABLE from the widgets UI today, cascading to `sym_trap_running`, the
  Sym_Trap branch (optimizer_manager.cpp:186-199), `CalculateSymTrap` (1359-1403), and
  the Sym_Trap script shape (optimizer_stage_script.cpp:61-70). Flag, don't silently
  delete — the planned oracle path references the Sym_Trap handling (manager.cpp:905-909
  comments); decide whether it stays as headless-only or dies.
- **P2, pre-delete prerequisite:** the controller SHELL has no direct test — the stale
  comment at test/CMakeLists.txt:465-488 points to `test/lifecycle/optimizer_run_controller_test.cpp`
  which does not exist; the only shell observers today are the bridge tests being
  deleted. Write the shell pin (or accept zero coverage) BEFORE deleting the bridge tests.
- **Copy-chain simplification (one client):** hop-2 (controller.cpp:58 full launch copy)
  can be deleted NOW more aggressively than prior advice — `start()` takes
  `OptimizerRunRequest&` and mutates the 4 fields in place (the single caller owns `req`).
  Hop-3 (storage refresh) stays while the mirror lives in `start()`; hop-4 stays until
  the bundle.
- **8 binds + H1 + ghost trio:** unchanged conclusion (widgets-reachable — Initialize can
  fail from widgets), with one refinement: the guard defends only the CROSS-RUN straggler
  slice (a ghost's same-run relays pass `isCurrentRun`); removal still requires the
  manager thread-chain surgery (wired pre-validation, optimizer_manager.cpp:44-52) or a
  failed Initialize leaks the manager+thread from the driver ctor.

### Revised step list (supersedes the phase-0/3 steps it touches)

- **Step 0.5 (new, zero-risk):** add `"JTML_BUILD_EXPERIMENTAL": false` to the release
  preset (`CMakePresets.json`, release block ~:37-46) or delete the subtree + option.
- **Phase 0 additions:** delete the 10 CMake experimental test targets + `test/qml/` +
  2 oracle QML targets + qml_lint with the subtree; delete the seed surface +
  `ResetForDatasetClear` + `MlEstimateOutcome::seed` + `Severity::Info` + H2 tautology
  check (all inert today); delete `session_controller.cpp:56/58` dead locals.
- **Phase 3 shrinks:** the seed pipeline is no longer a PEEL — it is deleted in phase 0.
  What remains of phase 3: ghost trio removal (+ manager thread-chain surgery),
  directive string layer, hop-2/3 launch simplification, pointer-to-member connects,
  widgets-ladder/core-label single-source.
- **Unchanged:** phases 1-2 (bundle, A/B parity probe, adapter swap, spec-sync at the
  production write paths, MakeDefaultStageConfigs owning defaults 6/4/1 — noting
  `domain/settings_constants.h` TRUNK_DILATION/Z_SEARCH_DILATION are referenced only by
  experimental code today), and all bit-identity constraints.

Full follow-up reports: `07-followup-production-surface.md`, `08-followup-ml-path.md`,
Full follow-up reports: `07-followup-production-surface.md`, `08-followup-ml-path.md`,
`09-followup-coordinator-one-client.md`, `10-followup-test-reclassification.md`.

---

## 8. STATUS RESET (2026-09-05): post-cppcheck-pass tree + FRESH TODO LIST

Two working-copy passes landed since §7: the owner's "remove dead code" change
(experimental tree + QML tests deleted, `JTML_BUILD_EXPERIMENTAL=false` in release AND
profile presets, core seed impl removed) and the joint cppcheck-cruft pass (orphan cost
TUs ×6 + CustomVariables headers, CaptureCoordinator module, the full batch seam
(`batch_outcome.h`, `SetBatchCost`, the never-taken branch, the `CoordinatorBatchAbort`
catch), the phantom graph registry + its unbuildable test section, ~30 dead
accessors/methods across Viewer/MainScreen/controller/CFM). **Verified green:**
`pixi run build` passes; only `joint-track-machine-learning` + `Study2Grid` build.
Regression caught and fixed during the pass: deleting `optimizer_launch_slot` also
deleted the `public Q_SLOTS:` access specifier, killing every subsequent auto-connect
slot (see `docs/solutions/` entry).

### Completed since the original plan
- Step 0.5 (experimental off in both presets), P0-2 (moot with the experiment deleted),
  the entire seed pipeline (§7's PEEL→DELETE), phase-0 dead-code deletions (batch seam,
  epoch getters + counter, orphan TUs, phantom registry), dead Viewer/UI accessors.

### Fresh TODO (deepened 2026-09-05 — verified against the tree; each item = one
### logical jj change, ordered; do NOT start any item without re-verifying its
### line numbers — the tree has drifted three times this week)

**1. [P0] `objective_spec` sync — the live correctness bug**
   State: 7 mainscreen reads frozen at 6 (`mainscreen.cpp:1742, 2468, 3837, 3916,
   3995, 4049, 4804` — ML segment, load-image, 4 edge handlers, UpdateDilationFrames);
   `objective_spec` has exactly one write (copy-assign `CostFunctionManager.cpp:72`).
   Sub-steps:
   a. Add the sync: re-derive `objective_spec` from the CF's live params inside the two
      production write paths — `settings_control.cpp:912` (the ONLY user-Dilation sink)
      and the registry load `mainscreen.cpp:~4690` (`getCostFunctionClass` loop).
      Implementation shape: a tiny CFM helper (`SyncObjectiveSpecFromActive()`) called
      after any `setIntParameterValue`/`setDoubleParameterValue`/`setBoolParameterValue`
      on the ACTIVE cost function + after the load loop — NOT a new wrapping layer.
   b. Delete the 3 coordinator debug-print pairs (`optimizer_manager.cpp:975/983,
      1022/1030, 1087/1095` — they exist only to watch this bug).
   c. Probe: set trunk Dilation=9 via the settings dialog → resegment → the produced
      dilated image must be dilated by 9 (compare against Dilation=6 output); repeat for
      the registry-load path.
   d. Bit-identity note: this CHANGES segment/dilation-frame behavior for users with
      non-default Dilation — that is the intended fix; the optimizer path is untouched
      (it already reads the live CF param via the scan).

**2. [P0] Stage-ladder single-source (also kills a divide-by-zero UB)**
   State: widgets ladder recomputes the 4-way stage label from
   `display_optimizer_settings_` (`mainscreen.cpp:~4479-4492`) — raw division, UB when
   `enable_branch_ && branch_budget==0`; the controller core already computes the same
   label with a `max(1,...)` clamp.
   Sub-steps:
   a. Consume the controller's label: read `stageText()` off the controller in
      `onUpdateDisplay` (the `updateDisplayRelayed` hop already fires per tick) OR add
      stageText to the relay payload — pick one, keep ETA/pose text view-side.
   b. Delete the widgets ladder block; keep `display_optimizer_settings_` only if the
      ETA math needs it (it does — it's the budget sum above).
   c. Decision bundled here (see item 7e): whether the CORE progress machine is the
      single source (then delete `budgets_`/`StageLabel` duplication) or the inverse.
      Recommend core-as-source since it is Qt-free and pinned.
   d. Probe: run with branch enabled + budget 0 (the degenerate case) — no crash;
      labels identical to pre-change for normal budgets (log-diff the emitted strings).

**3. [P2] CFM/View carry-overs (small, one sweep)**
   a. `gpu_distance_maps_`/`gpu_heatmaps_`: upload paths are gone; drop the two member
      pointers (CFM.h:143-144) + the `UploadDistanceMap` params from `UploadData`'s
      signature (CFM.h + all 3 call sites — verify they're truly gone first).
   b. CFM.h:49 ctor comment still advertises `updateCostFunctionParameterValues(...)`
      (deleted) — rewrite the comment to the registry-load reality.
   c. `mainscreen.cpp:4804` `value_or(3)` fallback: dead branch (spec is never
      MahfouzVariantSpec) — decide: drop to `value_or(6)` for honesty or keep + comment.
      Also fix the stale DIRECT_MAHFOUZ comments in `optimizer_stage_script.cpp:152-158`.
   d. `session_controller.cpp:56/58` dead locals — verify they survived the owner's
      pass; delete if present.
   e. Probe for (a): monoplane score parity before/after (goldens NOT a gate — targeted
      probe only).

**4. [Phase 1a] ResourceBundle — binder-owns, pass by ref**
   a. Define the plain struct (no new layer): `ResourceBundle{ GPUModel* principal;
      vector<GPUModel*> non_principal; GPUMetrics* metrics; PoseMatrix* pose_storage;
      bool biplane; 3x StageFrameVariants{dilated_A/B, intensity_A/B vectors} }` in
      include/compute/ — every field is a today-existing CFM member or manager vector.
   b. Reshape `OptimizerManager::Initialize` to build it once; stages receive
      `const ResourceBundle&`; the per-CFM `UploadData` binding (3 CFMs aliasing
      pointers) collapses to bundle fields keyed by stage.
   c. Skip intensity-set upload unless a DIRECT_MAHFOUZ stage exists in the script
      (needs-only rule); consider variant aliasing when stage configs coincide.
   d. Probe: byte-exact per-pose scores before/after (monoplane).

**5. [Phase 1b] A/B parity probe (GATES the swap — do not skip)**
   a. New instrument `test/oracle/objective_parity_test.cpp`, Catch2, label
      `oracle;gpu`. Monoplane ONLY (biplane B term is stubbed; legacy adds
      dist_score² unconditionally — A-only parity is unreachable in biplane).
   b. Pose set per (stage, frame): stage start; post-run optimum; the full sym-trap
      pose list verbatim; ~16-pose range grid (corner + interior).
   c. Legs: legacy `InitializeActiveCostFunction` + `BuildGpuCostAdapter` lambda vs
      `DirectDilationObjective(spec, model, dilated_A[frame], metrics, nullptr)` +
      evaluate. One throwaway eval per leg first (CUDA context + scratch first-touch);
      capture initialize() return + error_message parity per (stage, frame).
   d. Comparison: memcpy doubles → uint64, require 100% identical; compare int
      white-sums separately to localize init-vs-evaluate divergence.
   e. Gate: probe 100% + ONE full scripted monoplane run bit-identical (budgets
      20k/25k/30k, emit order, per-repeat re-seed from CURRENT optimum).

**6. [Phase 1c] ObjectiveInstance swap in the stage loop**
   Prereq: item 1 (spec sync — the prepared CFM-internal swap reads the frozen spec
   and would run branch/leaf at 6), item 5 (probe green), and decision 9 (Sym_Trap
   fallback). Sub-steps:
   a. Executor owns `std::optional<ObjectiveInstance>` per (stage, frame); ctor =
      Initialize, dtor = Destruct; frame pointers from the bundle at construction.
   b. Adapter lambda: same body, capture `ObjectiveInstance&` instead of
      `CostFunctionManager&` (`optimizer_manager.cpp:~1569-1615`).
   c. Transcribe the asymmetries VERBATIM: trunk init unconditional + trunk destruct
      unconditional; branch init gated, branch NEVER destructs; leaf init/destruct
      gated on `enable_leaf_ && !error_occurrred_`; CalculateSymTrap fires regardless
      of leaf-init error (guard with the defined fallback from decision 9).
   d. Probe: full scripted-run bit-identity (emit order + optimum sequence + call
      count). Only THEN delete `InitializeActiveCostFunction`/`DestructActiveCost
      Function`/`callActiveCostFunction` + the 6 remaining dispatch scaffolding.

**7. [Phase 1d] Stage-loop gates onto spec fields (dual-sourcing resolution)**
   State verified: raw-settings gates still live (`optimizer_manager.cpp:234, 890,
   1031`, + leaf/branch counterparts at ~1141/1190 equivalents — re-grep). Sub-steps:
   a. Enumerate each raw gate; drop ONLY the tautological conjuncts (`enable_branch_`/
      `enable_leaf_` — scripts contain the spec iff enabled while BuildStageScript is
      the source) and move the trunk budget reset (234/890) onto `trunk_spec.budget`.
   b. KEEP verbatim: `!error_occurrred_` everywhere, `!sym_trap_call`,
      `spec.repeat > 0`, and the destruct asymmetry (see 6c).
   c. `CumulativeStageCaps` stays as the test-only executable pin of the budget shape.
   d. Probe: scripted-run bit-identity again (gates touched the loop).

**8. [Phase 2] Config decoupling**
   a. `MakeDefaultStageConfigs()` factory in services (owns defaults 6/4/1; ALSO
      writes each spec — the P0-1 write path exists from birth). Note
      `domain/settings_constants.h` TRUNK_DILATION/Z_SEARCH_DILATION now have zero
      consumers (experimental deleted) — the factory becomes their consumer or they
      die; add BRANCH_DILATION=4.
   b. `getAvailableCostFunctions` → const ref (prereq: constify the pure getters on
      CostFunction/Parameter — mechanical) — structurally kills the by-value-map bug
      class that bit SettingsBridge.
   c. StageSpec carries `ObjectiveSpec` + bundle id; retire `cfm_index` +
      `EvaluateCostFunctionAtPoint`'s stage switch; `DeriveStageCostParams` →
      parity shim for item 5's dual-path leg, then DELETE with the scan sites
      (optimizer_manager.cpp pre-run 279/284/288 + per-stage 992/1043/1107, current
      lines to re-verify).

**9. [Phase 3] Coordinator remainder (one unit each)**
   a. Ghost trio: 8 binds (`bindManager`, controller.cpp ~386-455) + H1 epoch/sender
      guard (`isCurrentRun` + 7 call sites) + ghost quirk (~166-190) + the
      manager-INTERNAL thread chain wired pre-validation in `Initialize`
      (optimizer_manager.cpp:44-52) — remove ALL TOGETHER or a failed Initialize
      leaks the driver's manager+thread. Runtime probe: fail Initialize (bad
      directive) → no leak, no straggler relays.
   b. Directive string layer: delete `DirectiveToString` + the manager's string
      compares + the 5 locals + the two "Backward" re-compares; enum survives.
      Bit-identity: the manager's directive dispatch order must not change.
   c. Launch copy chain: `start(OptimizerRunRequest&)` in-place (single caller owns
      `req`) kills hop-2 (controller.cpp:58 full copy); hop-3 storage refresh stays
      while the mirror lives in start(); hop-4 dies only with the bundle (item 4).
   d. Pointer-to-member connects in the same change as (a) — string connects fail
      silently at runtime (no test net).
   e. Progress machine single-source decision (tied to item 2): either core-as-source
      (delete widgets ladder + keep StageLabel) or ladder-as-source (delete core
      StageLabel/budgets_/accessors — they have zero production consumers now).

**10. [Phase 4] Test tree revival**
   a. Wire `add_subdirectory(test)` behind `JTML_BUILD_TESTS` in the root CMakeLists
      (the option is a dead knob today) OR delete the option; default OFF.
   b. DELETE the 15 orphan/experimental files in test/unit (verified remaining:
      test_bank_binding_api, test_bank_state, evaluation_context_{,lease}_test,
      graph_key_assembler_test, graph_recipe_preflight_test, hook_feeder_test,
      test_cost_capacity_service, test_direct_optimizer_batch, capture_coordinator
      test (gone), 6 experimental_* tests) + the 12 graph-era oracle files +
      test_direct_optimizer_batch references + graph-era goldens
      (graph_performance_baseline.json, graph_layer_verdict.json,
      graph_pre_registration.json, bit_identity_baseline.txt, oracle_multistage*.json).
   c. Write the controller-SHELL pin first (QtTest, worker-thread relays per
      QTBUG-2842) — the shell has ZERO direct tests today and the stale comment at
      test/CMakeLists.txt:465-488 points to a file that does not exist.
   d. Then `pixi run configure && pixi run build && pixi run test` (headless label)
      and fix the fallout — the 22 orphans were never compiling; the survivors
      (stage-script pins, metric semantics, controller-core, domain/services) should
      go green as-is.
   e. Keep the fresh pins: objective parity probe (item 5) is the anchor instrument.

**11. [Decision owed — owner] Sym_Trap chain + F2 fallback (one decision, two heads)**
   `optimizer_launch_slot` is deleted → `Directive::SymTrap` is FULLY unreachable from
   the UI. Choose:
   (i) KEEP as headless-only: delete nothing; the oracle path (manager.cpp:905-909
       comments) drives it; then F2's fallback = the executor keeps a defined
       zero-score path for the CalculateSymTrap-after-leaf-init-error case
       (reproduce legacy stale-globals semantics, documented).
   (ii) DELETE: the Sym_Trap branch in Initialize, `CalculateSymTrap`, the script
       shape, `sym_trap_running`, the Sym_Trap oracle comments — then F2 evaporates
       (no leaf-error sym-trap path exists).
   Do NOT leave it half-deleted: the chain spans view/coordinator/stage-script and
   any stage-loop replacement must transcribe it exactly.
`09-followup-coordinator-one-client.md`, `10-followup-test-reclassification.md`.
