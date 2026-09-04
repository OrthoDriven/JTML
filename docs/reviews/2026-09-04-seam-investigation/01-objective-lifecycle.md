All evidence gathered. Since no `structured_output` tool is exposed in this session, returning the complete structured artifact here per the runtime fallback rule.

```json
{
  "seam": "Objective lifecycle",
  "call_site_inventory_production": [
    {"site": "src/coordinator/optimizer_manager.cpp:1028-1032", "api": "InitializeActiveCostFunction", "context": "Trunk spec: init UNCONDITIONAL per frame"},
    {"site": "src/coordinator/optimizer_manager.cpp:1044-1045", "api": "(search gate)", "context": "RunDirectStage gated on !error_occurrred_"},
    {"site": "src/coordinator/optimizer_manager.cpp:1046-1051", "api": "DestructActiveCostFunction", "context": "Trunk: destruct UNCONDITIONAL (even on error) — trunk side of the asymmetry"},
    {"site": "src/coordinator/optimizer_manager.cpp:1075-1082", "api": "InitializeActiveCostFunction", "context": "Branch: gated on enable_branch_ && number_branches > 0 && !error_occurrred_ (optimizer_settings_ raw gate, not script)"},
    {"site": "src/coordinator/optimizer_manager.cpp:1058-1123", "api": "(absent)", "context": "Branch case contains NO DestructActiveCostFunction at all — branch never destructs"},
    {"site": "src/coordinator/optimizer_manager.cpp:1141-1147", "api": "InitializeActiveCostFunction", "context": "Leaf: gated on enable_leaf_ && !error_occurrred_"},
    {"site": "src/coordinator/optimizer_manager.cpp:1157-1163", "api": "CalculateSymTrap", "context": "Gated ONLY on sym_trap_call — runs even after leaf-init failure (the hazard)"},
    {"site": "src/coordinator/optimizer_manager.cpp:1166-1196", "api": "(search) + DestructActiveCostFunction:1191", "context": "Leaf search gated enable_leaf_ && !error_occurrred_ && !sym_trap_call && spec.repeat > 0; leaf destruct gated enable_leaf_ && !error_occurrred_ — leaf side of the asymmetry"},
    {"site": "src/coordinator/optimizer_manager.cpp:1477-1497", "api": "callActiveCostFunction", "context": "EvaluateCostFunctionAtPoint dispatches trunk/branch/leaf (1487/1490/1493); ONLY caller is CalculateSymTrap (1434)"},
    {"site": "src/coordinator/optimizer_manager.cpp:1613-1641", "api": "callActiveCostFunction", "context": "jta::BuildGpuCostAdapter lambda — the one DIRECT cost funnel; sets primary pose (+biplane secondary via convert_Pose_A_to_Pose_B) then stage_manager.callActiveCostFunction()"},
    {"site": "src/coordinator/optimizer_manager.cpp:929-931", "api": "setCurrentFrameIndex", "context": "Per-frame binding of all 3 CFMs inside the Optimize() frame loop"},
    {"site": "src/coordinator/optimizer_manager.cpp:777-815", "api": "UploadData x3 + UploadDistanceMap x3", "context": "Resource binding: each stage CFM gets its OWN dilated/intensity frame vectors + shared edge frames/model/metrics"},
    {"site": "test/oracle/*, test/unit/*", "api": "all three APIs", "context": "Legacy oracle probes (bit_identity_test.cpp:266,348; oracle_test.cpp:264,354,395; z_profile_test.cpp:628+; throughput_serial_pipeline.cpp:183; layered_correctness_test.cpp:204) — suite NOT built/run (AGENTS.md), not a gate"}
  ],
  "error_gating_asymmetries_precise": [
    "Trunk init error -> emit OptimizerError + error_occurrred_=true (1028-1032); trunk destruct still runs (1046-1051).",
    "Branch has NO destruct call whatsoever — only trunk and leaf destruct (verified: DestructActiveCostFunction call sites are exactly optimizer_manager.cpp:1047 and :1191).",
    "Leaf destruct is gated on enable_leaf_ && !error_occurrred_ (1190) — skipped entirely when the stage errored; trunk destruct is not.",
    "HAZARD: leaf init failure (1142-1146) sets error_occurrred_ and skips the search, but `if (sym_trap_call) CalculateSymTrap();` (1161-1163) fires regardless. Path: CalculateSymTrap (1402) -> EvaluateCostFunctionAtPoint(pose, 2) (1434) -> leaf_manager_.callActiveCostFunction() (1493) -> costFunctionDIRECT_DILATION() (CostFunctionManager.cpp:248) scores with whatever the SHARED globals hold — i.e., stale white-sums from the last successful init of ANY stage (they are process-wide globals, see below), or 0 on the very first frame.",
    "All the white-sum/dilation warm state is NOT per-manager: include/compute/DIRECT_DILATIONCustomVariables.h:25-27 defines non-extern, non-static globals in a header (`int DIRECT_DILATION_current_white_pix_sum_dilated_comparison_image_A_/B_ = 0; DIRECT_DILATION_current_dilation_parameter = 0;`), included by CostFunctionManager.h:143-149 into every TU. All CFM instances — across OptimizerManager, MainScreen (include/view/mainscreen.h:320-323), SettingsControl (include/view/settings_control.h:40-43), SettingsBridge (SettingsBridge.h:271-273) — share one copy. Correctness currently holds ONLY by strict single-threaded init->search sequencing."
  ],
  "lifetime_answer": {
    "exact_lifetime": "Per (stage, frame) instance, scoped init->search->destruct inside the per-spec stage case, inside the per-frame loop. Evidence: setCurrentFrameIndex per frame (929-931); init at stage start; destruct at stage end; the 'init' work is only (a) ComputeSumWhitePixels on the current frame's dilated comparison image (A, and B if biplane) and (b) reading the registry 'Dilation' param (src/compute/DIRECT_DILATION.cpp:15-40); the 'destruct' is a literal no-op returning true (DIRECT_DILATION.cpp:43-45, CostFunctionManager.cpp:280-289). The durable GPU resources (per-stage dilated/intensity frame vectors, model, metrics) live for the whole run and are bound once in Initialize (777-815) — they are NOT what init/destruct manage.",
    "owner_today": "CostFunctionManager (ambient): warm state in process-wide globals + registry read at init; the prepared swap (commented code, CostFunctionManager.cpp:262-274) puts it in CFM's active_objective_instance_ (CostFunctionManager.h:181).",
    "owner_target": "The stage executor (the Optimize() loop's replacement) owns a std::optional<ObjectiveInstance> per (stage, frame), constructed from explicitly-passed resources: {DirectDilationSpec, GPUModel*, GPUDilatedFrame* comparison_A, GPUMetrics*, GPUDilatedFrame* comparison_B|nullptr}. Construction IS Initialize(); instance destruction IS DestructActiveCostFunction(); for DIRECT_DILATION the destructor is trivially safe, so the trunk/leaf gating asymmetry collapses to 'run the dtor unconditionally' with zero observable change TODAY — but the gating must be transcribed verbatim until deletion because a future objective's teardown may fail.",
    "bit_identity_condition": "Bit-identity holds iff (1) exactly one instance is live per stage at a time (reproduces the shared-global sequencing), (2) evaluate() re-renders before the metric (DirectDilationObjective::evaluate does: src/objectives/direct_dilation.cpp:27-29, matching costFunctionDIRECT_DILATION's RenderPrimaryCamera(GetCurrentPrimaryCameraPose()) — same GPUModel::RenderPrimaryCamera(Pose) entry, src/compute/gpu_model.cu:116-121), and (3) spec_.dilation == registry 'Dilation' for that stage (CURRENTLY VIOLATED — see finding F1)."
  },
  "minimal_swap_in_adapter_lambda": {
    "option_prepared": "Flip CFM internals to the already-written commented code (CostFunctionManager.cpp:248-290): init constructs DirectDilationObjective(objective_spec, gpu_principal_model_, gpu_dilated_frames_A_->at(current_frame_index_), gpu_metrics_, biplane_mode_ ? gpu_dilated_frames_B_->at(current_frame_index_) : nullptr) + initialize(); call returns active_objective_instance_->evaluate(gpu_principal_model_->GetCurrentPrimaryCameraPose()); destruct resets the unique_ptr. BuildGpuCostAdapter's lambda body (optimizer_manager.cpp:1619-1641) stays untouched — pose setting is already explicit and correct.",
    "option_target_shape": "Bind the instance OUTSIDE CFM: the stage executor constructs the ObjectiveInstance at stage init and BuildGpuCostAdapter takes it by reference (`std::function` capturing `ObjectiveInstance&`), returning `instance.evaluate(pose)`; CFM's callActiveCostFunction is then bypassed, not re-routed. Same lambda body, one capture changes from `CostFunctionManager&` to `ObjectiveInstance&`.",
    "blockers_before_either": "F1 (spec/registry divergence — branch/leaf would run dilation 6 instead of 4/1) and F2 (null-instance deref on the Sym-Trap-after-leaf-init-error path) must be resolved; the prepared CFM-internal option inherits F1 verbatim (CostFunctionManager.cpp:268 uses std::get<DirectDilationSpec>(objective_spec), never synced from the registry)."
  },
  "ab_parity_probe_design": {
    "pose_set": "Per (stage, frame): (a) the stage starting point; (b) current_optimum_location_ after a short deterministic DIRECT run; (c) the full sym-trap pose_list (create_vector_of_poses(pose_6D, 20) -> 60 evals, optimizer_manager.cpp:1415-1434) reused verbatim; (d) a fixed grid of ~16 poses across the stage search range (corner + interior, catches bounding-box kernel divergence). Run monoplane first (biplane is blocked by the B stub).",
    "dual_path_wiring": "Same GPUModel*, same GPUMetrics*, same GPUDilatedFrame* per (stage, frame). Legacy leg: InitializeActiveCostFunction + BuildGpuCostAdapter lambda. Candidate leg: DirectDilationObjective(spec_from_registry, model, dilated_A[frame], metrics, nullptr) + initialize() + evaluate(pose). Interleave per pose: set pose -> legacy call -> set pose -> instance evaluate. Both legs re-render before their metric (in-place dilate kernels in FastImplantDilationMetric mutate the render buffer, src/compute/fast_implant_dilation_metric.cu:493-554), so alternation is safe only because each leg re-renders — assert that stays true.",
    "warm_up_cold_start": "First call in process = CUDA context + module load + first-touch of metric scratch buffers (GPUMetrics ctor cudaMallocs, gpu_metrics.cu:38-86): do one throwaway eval per leg before capture. Second cold-start surface: first eval after each fresh Initialize (white-sum kernel ordering + error_message string) — capture initialize() return AND error_message parity per (stage, frame). No per-call warm-up state exists inside FastImplantDilationMetric/Render (stateless kernel chains), so after warm-up, values must be exact.",
    "comparison": "Bitwise, not tolerance: memcpy each double to uint64_t, require 100% identical across all (pose, stage, frame, dilation) cells; also uint64-compare the int white-sums and -1.0*pixel_score composition separately to localize any mismatch to init vs evaluate. Values are int-sourced doubles (FastImplantDilationMetric returns -1.0 * int, DIRECT_DILATION.cpp:55-61), so any bit difference is a real divergence.",
    "matrix": "3 stages x dilation {6,4,1} x N frames x pose set, monoplane; repeat the whole matrix after a fresh Initialize to cover cold-start.",
    "gate_before_deletion": "Probe parity 100% bit-identical PLUS one full scripted monoplane run with the instance path swapped in reproducing current_optimum_location_/current_optimum_value_/cost_function_calls_/emit sequence exactly (cumulative 20k/25k/30k budgets, per-repeat re-seed from CURRENT optimum). Only then delete init/destruct/call from CFM. No ObjectiveInstance tests exist today (grep test/ for DirectDilationObjective|ObjectiveInstance: 0 hits) — the probe is net-new."
  },
  "biplane_b_term_stub": {
    "what_is_skipped": "DirectDilationObjective::evaluate's `if (comparison_B_ != nullptr) {};` (src/objectives/direct_dilation.cpp:34) is a total no-op where legacy costFunctionDIRECT_DILATION runs, under biplane_mode_ (src/compute/DIRECT_DILATION.cpp:63-79): (1) gpu_principal_model_->RenderSecondaryCamera(GetCurrentSecondaryCameraPose()) — the secondary pose the adapter sets (optimizer_manager.cpp:1619-1630); (2) dist_score = white_pix_sum_B + FastImplantDilationMetric(secondary render, dilated_B[frame], dilation); (3) metric_score += dist_score * dist_score. Init-side, legacy also computes white_pix_sum_B under biplane_mode_ (DIRECT_DILATION.cpp:25-31); the instance computes it under comparison_B_ != nullptr (direct_dilation.cpp:13-17) — equivalent iff binding passes B only in biplane mode, which the prepared CFM swap does (CostFunctionManager.cpp:271-272).",
    "a_term_alone_attainable": "YES for monoplane: with comparison_B_ = nullptr the instance computes exactly white_sum_A + FastImplantDilationMetric(primary_render, dilated_A, dilation) — expression-for-expression identical to DIRECT_DILATION.cpp:53-61 given (3) dilation parity. NO for biplane: legacy score = A_score + dist_score^2 with dist_score != 0 in general, so A-term-only parity is unreachable there; the probe must be monoplane-gated until B lands. Note B landing is local: the secondary pose is already set explicitly by the adapter, and the pose->secondary transform (convert_Pose_A_to_Pose_B, called with `mutable` calibration capture, optimizer_manager.cpp:1619-1630) does not need the unified transform rep — the 'unified rigid-transform rep' prerequisite is about representation cleanliness, not about this one call site."
  },
  "findings": [
    {
      "id": "F1",
      "severity": "P1",
      "classification": "PEEL (blocker for the swap)",
      "issue": "objective_spec is NEVER assigned from the registry — it is default DirectDilationSpec{dilation=6} on every CFM forever. Production dilation lives in the CostFunction registry: trunk 6 default (CostFunctionManager.cpp:294-297), branch 4 / leaf 1 (mainscreen.cpp:4779-4781), user-overridable via settings load (mainscreen.cpp:4690, SettingsBridge.cpp:544+ no-op per brief). The prepared swap reads std::get<DirectDilationSpec>(objective_spec) (CostFunctionManager.cpp:268), so branch/leaf would evaluate with dilation 6 — silent bit-divergence. The stage loop's own ObjectiveSpec-vs-Legacy debug prints (optimizer_manager.cpp:1013-1027, 1060-1074, 1125-1140) are evidence someone is already watching this seam.",
      "evidence": "grep '.objective_spec =' over src/ + include/: only the copy-assignment (CostFunctionManager.cpp:72). objective_spec.h:9 default 6.",
      "smallest_fix": "Build the instance spec from the same source DeriveStageParams reads (getActiveCostFunctionClass()->getIntParameterValue(\"Dilation\")) — e.g. a tiny `DirectDilationSpecFromRegistry(manager)` helper next to DeriveStageCostParams — or sync objective_spec in Initialize/load. Do NOT wire the swap until this lands."
    },
    {
      "id": "F2",
      "severity": "P1",
      "classification": "PEEL (behavior-preservation decision required)",
      "issue": "Sym-Trap-after-leaf-init-error: in the ObjectiveInstance world there is no live instance to score with, so callActiveCostFunction would deref a null unique_ptr where legacy scores with stale/zero shared globals. Any swap must decide: reproduce the legacy values (requires keeping a legacy-warm fallback) or document it as the intended fix of a flagged hazard. Constraint says reproduce EXACTLY — so the transitional executor needs an explicit guard with a defined fallback, not a crash.",
      "evidence": "optimizer_manager.cpp:1142-1147 (init failure), 1157-1163 (sym trap ignores error), 1434->1493->CostFunctionManager.cpp:248 chain; DIRECT_DILATIONCustomVariables.h:25-27 globals."
    },
    {
      "id": "F3",
      "severity": "P2",
      "classification": "DELETE",
      "issue": "Process-wide shared warm-state globals in a header (DIRECT_DILATIONCustomVariables.h:25-27, plus DD_NEW_POLE_CONSTRAINT/T1/SAME_Z/POLE_CONSTRAINT/sym_trap_function CustomVariables includes, CostFunctionManager.h:143-149): ODR hazard (non-extern definitions in a header, multi-TU) and the reason correctness hangs on sequencing. The ObjectiveInstance members (direct_dilation.hpp:47-48) delete this class of state — that deletion is the whole point of the peel.",
      "smallest_fix": "Delete the globals with the CFM impl files once the probe passes; until then treat every init/destruct ordering as load-bearing."
    },
    {
      "id": "F4",
      "severity": "P2",
      "classification": "KEEP (transcription constraint)",
      "issue": "Destruct asymmetry is currently unobservable (destructDIRECT_DILATION is `return true`, DIRECT_DILATION.cpp:43-45) BUT branch never destructs at all (no DestructActiveCostFunction in optimizer_manager.cpp:1058-1123) — any replacement that 'symmetrizes' destructs per stage changes the call sequence for non-DIRECT_DILATION objectives and breaks the verbatim transcription constraint. Transcribe trunk=unconditional, branch=none, leaf=gated verbatim.",
      "evidence": "Call-site grep: destruct only at optimizer_manager.cpp:1047, 1191."
    },
    {
      "id": "F5",
      "severity": "P2",
      "classification": "KEEP (inventory confirmation)",
      "issue": "CostFunctionType was trimmed to ONLY DirectDilation (include/compute/CostFunction.h:23-27 X-macro list), so callActiveCostFunction's single-case switch is total and the other 5 CFM impl files (DIRECT_DILATION_T1/SAME_Z/POLE_CONSTRAINT, DD_NEW_POLE_CONSTRAINT, DIRECT_MAHFOUZ, sym_trap_function in src/compute/) are unreachable dead code — safe to DELETE in the same pass as the globals. Their CustomVariables headers are included by CostFunctionManager.h and should go with them.",
      "evidence": "Enum list + listCostFunctions registers only DIRECT_DILATION (CostFunctionManager.cpp:293-297); setActiveCostFunction call sites only ever pass types parsed from strings that must match the X-macro list (cost_function_type_from_string returns nullopt otherwise)."
    },
    {
      "id": "F6",
      "severity": "P2",
      "classification": "PEEL (known-gap confirmation)",
      "issue": "Per-frame rebinding not wired: current_frame_index_ is ambient CFM state set per frame (optimizer_manager.cpp:929-931) and the prepared init reads through it (gpu_dilated_frames_A_->at(current_frame_index_), CostFunctionManager.cpp:269). Target shape: comparison_A/B become explicit ctor args of the executor-owned instance — already the DirectDilationObjective signature; only the ambient read-through must go when CFM dies. Note SettingsBridge::applyCostFunctionEntries mutates the by-value map from getAvailableCostFunctions (SettingsBridge.cpp:544-566; CostFunctionManager.cpp:209-211 returns by value) — parameter writes are silent no-ops, confirming the brief; this affects which dilation value the registry actually holds at swap time (F1's source of truth)."
    }
  ],
  "merge_verdict": "OK with notes — no edits made (read-only review). The seam is ready for the swap ONLY after F1 (dilation spec source) and F2 (sym-trap fallback decision) are resolved; F4 gating and F5 dead-code deletion are sequencing constraints for the deletion pass, gated on the A/B probe going 100% bit-identical on monoplane."
}
```

**Key deltas from the brief (spot-check results):**
- Brief said trunk/leaf destruct asymmetry — verified at :1047 (trunk, unconditional) and :1191 (leaf, gated). **Addition:** branch never destructs at all (no call in the branch case) — must be transcribed.
- Brief's "engine-runs-CalculateSymTrap-after-leaf-init-error" hazard verified, and its blast radius is sharper than stated: the warm state it scores with is **process-wide shared globals** (`DIRECT_DILATIONCustomVariables.h:25-27`), not per-manager members — stale values from any earlier stage/frame, ODR-hazardous in a header.
- **New P1:** the prepared CFM swap reads `objective_spec` (never assigned anywhere → always dilation 6) instead of the registry that actually drives legacy branch=4/leaf=1 — the prepared commented code would silently break bit-identity if flipped today. The stage loop's debug prints already compare the two.
- B-term stub confirmed as skipping the entire biplane block (`DIRECT_DILATION.cpp:63-79`); A-term-only parity is attainable in **monoplane only** — biplane adds `dist_score²` unconditionally.