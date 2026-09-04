All evidence gathered. No `structured_output` tool is available in this session, so per the runtime rule I return the complete structured artifact in this final response for the runtime to persist.

```json
{
  "seam": "Coordinator teardown",
  "scope": "src/coordinator/, include/coordinator/, QML bridges in src/app/experimental/, widgets consumer in src/view/mainscreen.cpp",
  "evaporate_candidates": [
    {
      "item": "The 8 binds + H1 epoch/sender guard",
      "exists": "bindManager(): src/coordinator/optimizer_run_controller.cpp:386-428 (finished first :390, then 7 string-based connects); guard isCurrentRun(): :242-247 (driver_ non-null && run_epoch_ == core_.epoch() && sender == driver_->Manager() && state != Idle); epoch stamped at :131 (run_epoch_ = core_.epoch()); 7 guard call sites :252,266,283,302,349,362,369",
      "defends": "Straggler relays from (a) the failed-Initialize ghost thread started at :171 and (b) a previous run's manager whose connections stay alive until the next start() releases the driver (include/coordinator/optimizer_run_controller.h:96-101)",
      "moot_after_fresh_manager_per_run?": "MOSTLY. onManagerFinished() already Waits the thread (:369-374) and the start() gate rejects while ThreadActive (:40-53), so the only surviving stale-sender source is the ghost quirk itself. Remove the quirk and the sender+epoch guard has no remaining trigger.",
      "class": "PEEL (together with the ghost quirk; they defend each other)"
    },
    {
      "item": "Ghost-thread quirk (R13-preserved)",
      "exists": "src/coordinator/optimizer_run_controller.cpp:166-190: on Initialize failure the driver is STARTED (:171) before the error box, so the manager's internal finished chain (wired in OptimizerManager::Initialize at src/coordinator/optimizer_manager.cpp:44-52: started->Optimize, finished->quit/deleteLater x2) runs and terminates the zombie",
      "defends": "A manager+thread pair the manager itself constructed-and-wired before validating inputs; Initialize wires the thread chain BEFORE any validation, so failure leaves live QObjects that only die if the thread runs",
      "moot_after_fresh_manager_per_run?": "Yes — the quirk is scaffolding around the manager OWNING the thread chain. In the target shape the binder prepares resources and the run is short-lived state; failure = dispose the bundle, no thread ever started. The M6 finished-before-Initialize bind (:390) exists only to observe the ghost.",
      "class": "DELETE (with the manager-internal thread chain that forces it)"
    },
    {
      "item": "Request / launch / gate-input struct family",
      "exists": "OptimizerRunRequest: include/coordinator/optimizer_run_controller.h:48-76 (save mirror args + gate values + storage ptr + launch); OptimizerRunLaunch: include/coordinator/optimizer_run_driver.h:41-69 (17-field shuttle of Initialize's 17 params); GateInput: include/coordinator/optimizer_run_controller_core.h:89-98",
      "defends": "The seam between views (state owners) and the manager's 17-parameter Initialize; GateInput exists because the gate lives in the controller but every input it reads lives in the views; Launch exists to thread BY-VALUE copies across the driver seam",
      "moot_after_fresh_manager_per_run?": "Partly now, fully after peel. GateInput collapses when gate inputs are explicit run inputs; Launch collapses to (bundle ref/intent) once a resource bundle replaces the 13 data params. The Directive enum + intent fields survive as the run intent.",
      "class": "PEEL (keep a minimal run-intent struct)"
    },
    {
      "item": "Seed pipeline split core/shell (M10a)",
      "exists": "core takeSeedForRun with stale guards: include/coordinator/optimizer_run_controller_core.h:168-186, impl src/coordinator/optimizer_run_controller_core.cpp:83+; shell apply + snapshot + restore: src/coordinator/optimizer_run_controller.cpp:104-122 (takeSeedForRun after gate, snapshot :108-112, refresh :118-125, restore on Initialize failure :176-181); out-of-run applySeedPose delegate :212-221",
      "defends": "One-shot pending AMBIENT state: the estimate must survive until the next run() but not past a stale frame/model change, and the run must not start from a stale/drifted copied pose (payload refresh P1-2, :118-125)",
      "moot_after_fresh_manager_per_run?": "The whole one-shot/stale/snapshot apparatus exists only because the run reads its start pose from a copied LocationStorage relayed by value. In the target shape the seed is an explicit run input (initial pose in the intent/bundle): set-at-estimate-time, consumed at run, no snapshot/restore needed (a failed run never wrote anything).",
      "class": "PEEL"
    },
    {
      "item": "5-hop seed relay MlBridge->AppBridge->OptimizerBridge->controller->core",
      "exists": "Set path: MlBridge.cpp:413-419 (optimizer_bridge_->setSeedPose(x..za)) -> OptimizerBridge.cpp:226-239 (controller_->setSeedPose with study_bridge current frame/model) -> controller shell controller.h:146-160 -> core (controller_core.h:155). Clear path adds AppBridge.cpp:37 (dataset clear), :115-131 (manual pose write D4 -> clearSeedPose x2). Apply-back: seedApplied/seedRestored -> OptimizerBridge.cpp:402-419 (scene re-sync)",
      "defends": "Moving ONE Point6D from the ML estimate to the next run through pending ambient state, plus dropping it on dataset/manual-pose invalidation",
      "moot_after_fresh_manager_per_run?": "Yes: the estimate becomes the run input at launch time; the clear-hops become trivial (nothing pending to invalidate). The AppBridge invalidation hops (H5/M10b/D4) evaporate with the pending state.",
      "class": "PEEL"
    },
    {
      "item": "Directive enum->enum->string->string-compare 4-representation chain",
      "exists": "Widgets buttons -> Directive enum (core enum, controller_core.h:66-73) -> req.directive (controller.h:53) -> DirectiveToString (src/coordinator/optimizer_run_controller.cpp:~424-444: 'Single'/'All'/'Each'/'From'/'Backward'/'Sym_Trap') -> QString launch.directive (driver.h:67) -> manager string compares (src/coordinator/optimizer_manager.cpp:153-207) producing 5 locals (progress_next_frame_, init_prev_frame_, start_frame_index_, end_frame_index_, sym_trap_call) -> BuildStageScript built FROM THE SAME STRING (:222-224). Controller also string-compares 'Backward' in onManagerOptimizedFrame for advance direction (optimizer_run_controller.cpp:~300-310)",
      "defends": "Bit-identity with the legacy string-driven behavior; the string is load-bearing TODAY: unknown-directive error path (optimizer_manager.cpp:207), the advance-direction compare, and BuildStageScript's input",
      "moot_after_fresh_manager_per_run?": "After the peel the manager consumes the StageScript built from the TYPED directive (already built at :222); the string layer and the 5 locals become dead reps. The enum survives.",
      "class": "PEEL (string + compares DELETE; enum KEEP)"
    },
    {
      "item": "QML double gate evaluation",
      "exists": "OptimizerBridge.cpp:141 (EvaluateGate pre-check, incl. SingleModelOnly bridge policy :147-153) AND controller.cpp:76-80 re-evaluates the same gate inside start() (plus re-runs the SaveLastPose mirror :66-74 and seed)",
      "defends": "The bridge maps 2 statuses into 3 (adding SingleModelOnly policy), then re-checks for a nicer message before paying the mirror/seed cost",
      "moot_after_fresh_manager_per_run?": "Fixable NOW without the peel: give the shared gate a single-model policy flag (or return all 3 statuses) and delete the bridge pre-check — the controller gate already emits the right messages.",
      "class": "DELETE"
    }
  ],
  "remain_candidates": [
    {
      "item": "Worker thread + queued by-value relays (QTBUG-2842)",
      "exists": "Manager emits from worker (optimizer_manager.cpp:44-46 started->Optimize); controller re-emits by value on its own thread (controller.h:18-21, documented QTBUG-2842; all 7 relay slots); OptimizerBridge re-emits again for QML (OptimizerBridge.cpp:83-124); widgets consumes controller-thread relays (mainscreen.cpp:184-210)",
      "class": "KEEP — load-bearing TODAY. Any view/QSignalSpy must observe controller-thread signals, never worker emissions. After the peel this shrinks to a thin UI adapter over the engine's callback interface; the relay contract itself remains the Qt-side seam."
    },
    {
      "item": "Controller shell/core split",
      "exists": "Qt-free core (controller_core.h, pinned by test/unit/optimizer_run_controller_core_test.cpp, registered test/CMakeLists.txt:489-501) + QObject shell (driver seam, thread lifecycle, destructor contract H3 driver.cpp:36-48)",
      "class": "KEEP — the run-state machine and progress mapping stay the UI's source of truth regardless of engine internals; the shell is where the bundle injection lands (it owns the only driver factory call site)."
    },
    {
      "item": "SettingsBridge (the one real ViewModel) and MainScreen's CFM trio",
      "exists": "SettingsBridge.cpp:35-41,106-114 (3 CFMs = config holders), exposed as trunkManager()/branchManager()/leafManager() (:456-464); widgets counterpart mainscreen.cpp:4772-4774,4814-4820",
      "class": "KEEP the bridges TODAY (they are the only config owners the views read); their CFM CONTENTS evaporate after the peel (the config moves into declarative stage params); the bridges stay as thin setting editors."
    },
    {
      "item": "SessionStateController + SaveLastPose service + OptimizeIntentController gate",
      "exists": "src/coordinator/session_state_controller.*; services/save_last_pose; domain/optimize_intent_controller",
      "class": "KEEP — selection mirrors and the gate are client-side session logic, not run scaffolding; the gate survives as input validation."
    },
    {
      "item": "OptimizerRunDriver seam",
      "exists": "driver.h:68-91; production adapter driver.cpp:28-115 (fresh manager+thread per run; H3 bounded-wait :95-105)",
      "class": "KEEP TODAY — it is the ONLY place OptimizerManager is constructed or Initialize'd (driver.cpp:31, :66-83), so it is the natural injection point for the resource bundle. Deletion-eligible after the peel, when the oracle/clients drive the bundle-consuming engine directly: note the advertised fake-driver testability is UNEXERCISED — test/CMakeLists.txt:466-480 mentions a fake driver but no FakeDriver implementation exists anywhere in test/ (grep: 0 matches); the suite is unbuilt (AGENTS.md 2026-08-28)."
    }
  ],
  "load_bearing_blast_radius": {
    "construction": "OptimizerManager is constructed at exactly ONE site: src/coordinator/optimizer_run_driver.cpp:31 (new OptimizerManager()). Initialize called at exactly ONE site: driver.cpp:66-83. Views and controller never name the type (they hold QObject* Manager()).",
    "contained_to": "Adding a resource-bundle parameter to the ctor/Initialize touches ONLY: (1) the adapter driver.cpp:28-115, (2) OptimizerRunLaunch (driver.h:41-69), (3) the two launch-fill sites mainscreen.cpp:4286-4296 and OptimizerBridge.cpp:205-215, (4) test/oracle/multistage_oracle_test.cpp which builds OptimizerRunRequests against the production factory (:961, :1303, :1466) and includes optimizer_manager.h directly.",
    "second_coupling_axis": "The manager SIGNAL surface: bindManager uses STRING-based connects (optimizer_run_controller.cpp:386-428). A signature change on any of the 7+1 signals fails at RUNTIME (connect returns false, silently), not compile time. Any bundle refactor touching signals needs a runtime probe — the CTest suite that would catch it is not built (AGENTS.md).",
    "insulated": "Controller, core, both bridges, MainScreen relays, SettingsBridge — all insulated behind the driver seam; the manager ctor can change without recompiling their logic (only the adapter + launch struct)."
  },
  "copy_chain": {
    "trace": [
      "Hop 1 view->request: mainscreen.cpp:4286-4291 (calibration_file_, loaded_frames, loaded_frames_B, loaded_models, model_locations_ = full copies) + :4293-4295 (trunk/branch/leaf CFMs = full registry copies); QML: OptimizerBridge.cpp:205-214 (incl. *settings_bridge_->trunkManager() deref-copies :212-214)",
      "Hop 2 request->launch: optimizer_run_controller.cpp:58 'jta::OptimizerRunLaunch launch = req.launch;' — FULL struct copy (every container again)",
      "Hop 3 controller refresh: optimizer_run_controller.cpp:125 'launch.pose_matrix = *req.storage;' — 4th full LocationStorage copy (seed+mirror landed in storage after hop 1)",
      "Hop 4 driver->Initialize: driver.cpp:66-83 forwards launch members into Initialize's BY-VALUE parameters (optimizer_manager.h:78-95: Calibration, vector<Frame> x2, vector<Model>, LocationStorage by value) — copy 3; CFMs arrive as const& (h:88-90) but are copied into members optimizer_manager.cpp:132-135 — CFM copy 3, and each CFM copy re-runs listCostFunctions() via copy-ctor delegation (CostFunctionManager.cpp:58-61) then deep-copies the registry (:70-73)",
      "Terminal: std::move into members (calibration_/frames_A_/frames_B_/all_models_, optimizer_manager.cpp:65,78-79,88) — no 4th copy for those; pose_matrix is consumed element-wise (per-selected-model pose re-serialization :137-160 + SetStartingPoint read :246-249), never stored whole"
    ],
    "totals": "frames x2, models, calibration: 3 full copies each; LocationStorage: 4 full copies; CFMs: 3 copies each, each re-listing the registry",
    "droppable_now": "Hop 2 (controller.cpp:58) is pure waste: start() only mutates launch.{current_frame_index, primary_model_index, directive, pose_matrix} (:129-132,:141-143,:125) — split those 4 into a tiny per-run intent and pass the dataset by const ref TODAY (driver::Initialize already takes const OptimizerRunLaunch&). Hop 3: pass storage ptr/seed pose instead of re-copying the matrix. Hop 4: manager must own its containers while the worker runs, but a single move from a caller-built bundle = 1 copy instead of 3. Constraint: lifetime must move to the bundle (shared_ptr), not to the UI freeze (DisableAll, mainscreen.cpp:4317) that currently makes const refs 'safe'.",
    "droppable_after_peel": "All of it: binder owns GPU resources + dataset once; run receives refs/shared_ptr; CFMs disappear from the launch entirely (their config-only content is already superseded by StageScript/DeriveStageCostParams data); 0 dataset copies per launch."
  },
  "stage_math_double_compute": {
    "confirmed": "Core: StageLabel (optimizer_run_controller_core.cpp:43-61) + refreshProgress (:64-74) recomputed on EVERY UpdateDisplay relay (controller.cpp:250-256) from BudgetsFromSettings(req.launch.settings) (controller.cpp:~400-415). View: MainScreen::onUpdateDisplay recomputes the IDENTICAL 4-way ladder ('Trunk'/'Branch N'/'Extra Z-Translation'/'Finished') from display_optimizer_settings_ (mainscreen.cpp:4536-4556; snapshot copied at :4314 after start()).",
    "divergence": "Core clamps branch_budget max(1,...) when enable_branch (controller_core.cpp:46); widgets divides raw (mainscreen.cpp:4545) -> divide-by-zero UB on enable_branch_ && branch_budget==0. Labels and thresholds otherwise identical.",
    "single_source_fix": "Delete the mainscreen.cpp:4536-4556 ladder; have onUpdateDisplay consume the controller's already-computed label — either add stageText to updateDisplayRelayed or read optimizer_run_controller_.stageText() (already refreshed by the same relay). Keep the ETA/pose text view-side. Behavior delta confined to the degenerate zero-branch-budget case (core safe, widgets UB today); pin label equality with a targeted probe."
  },
  "findings": [
    {
      "severity": "P1",
      "issue": "QML run path copies CFM config whose parameter edits are known silent no-ops: SettingsBridge::applyCostFunctionEntries mutates the by-value map returned by getAvailableCostFunctions (SettingsBridge.cpp:536 + following writes on the local copy), and OptimizerBridge then launches with *settings_bridge_->trunkManager() copies (OptimizerBridge.cpp:212-214) — the run's CFM config reflects only setActiveCostFunction, not loaded/edited params.",
      "location": "src/app/experimental/SettingsBridge.cpp:536; src/app/experimental/OptimizerBridge.cpp:212-214",
      "evidence": "Same finding class as the brief's verified no-op bug; confirmed here on the QML launch path (widgets path presumably feeds the same registry).",
      "smallest_fix": "Belongs to the CFM seam's fix (make getAvailableCostFunctions return a reference or add a mutate-in-place entry API); once fixed, this launch path inherits correctness. Do NOT fix by re-copy tricks at the bridge."
    },
    {
      "severity": "P2",
      "issue": "Divide-by-zero UB in widgets progress ladder when branch enabled with zero budget; the controller core already clamps (max(1,...)) so the two stage computations diverge in exactly this degenerate case.",
      "location": "src/view/mainscreen.cpp:4545 vs src/coordinator/optimizer_run_controller_core.cpp:46",
      "smallest_fix": "The single-source fix above removes the widgets ladder entirely."
    },
    {
      "severity": "P2",
      "issue": "bindManager uses string-based SIGNAL/SLOT connects; any future reshape of the manager's 7+1 signal signatures fails silently at runtime (the built suite is the only net and is currently not built).",
      "location": "src/coordinator/optimizer_run_controller.cpp:386-428",
      "smallest_fix": "When the bundle refactor lands, switch to pointer-to-member connects in the same change (the manager type is concrete in the adapter; the fake-driver story is unexercised, test/CMakeLists.txt:466-480 documents an intent with no in-tree FakeDriver)."
    },
    {
      "severity": "P2",
      "issue": "Pure-waste full struct copy of the entire launch payload at controller start (second full copy of frames/models/storage/CFMs) for 4 field mutations.",
      "location": "src/coordinator/optimizer_run_controller.cpp:58",
      "smallest_fix": "Split OptimizerRunLaunch into immutable dataset (const ref/shared) + 4-field run intent; pass dataset by const ref now."
    }
  ],
  "keep_peat_delete_summary": {
    "KEEP": "QThread + QTBUG-2842 relay chain; shell/core split; driver seam (as the bundle injection point); SettingsBridge + MainScreen CFM ownership (contents peel later); SessionStateController/SaveLastPose/gate",
    "PEEL": "8 binds + H1 guard (with ghost quirk); request/launch/gate-input struct family -> bundle + intent; seed core/shell split + 5-hop relay -> explicit run input; directive string layer (enum survives); CFM copies from every launch; settings 5-copies-during-run (optimizer_settings_ x2 in MainScreen, launch.settings, controller budgets_, manager optimizer_settings_)",
    "DELETE": "Ghost-thread quirk (with manager-internal thread chain); M6 finished-first bind (its only purpose); QML bridge pre-check gate; widgets stage-ladder recompute; hop-2 launch copy"
  },
  "merge_verdict": "OK with notes — read-only inventory seam; no code changes requested. The fresh-manager-per-run design already moots most of the hazard apparatus (epoch guard, ghost quirk, seed snapshot/restore are mutually-reinforcing scaffolding around the manager owning its thread chain and the by-value pose relay); the truly load-bearing remainder is the thread+relay contract, the shell/core split, and the driver seam as the single construction site (small blast radius for the bundle parameter, with the string-connect surface as the one runtime-only hazard)."
}
```