All evidence gathered. Note: no `structured_output` tool is available in this runtime, so I return the complete structured artifact inline per the fallback rule.

```json
{
  "seam": "Resource bundle / arena",
  "resource_blob": {
    "cfm_private_members": [
      {"member": "gpu_principal_model_", "decl": "include/compute/CostFunctionManager.h:172", "live": true, "readers": "all 7 objective fragments (e.g. src/compute/DIRECT_DILATION.cpp:48,66; DIRECT_MAHFOUZ.cpp:34; sym_trap_function.cpp:74,181; DD_NEW_POLE_CONSTRAINT.cpp:157; DIRECT_DILATION_SAME_Z.cpp:76; DIRECT_DILATION_T1.cpp:59; DIRECT_DILATION_POLE_CONSTRAINT.cpp:114); renders + pose set/get", "writers": "OptimizerManager::Initialize creates it once (src/coordinator/optimizer_manager.cpp ~710-727); bound by UploadData to all 3 CFMs (777-815)"},
      {"member": "gpu_non_principal_models_", "decl": "CostFunctionManager.h:174", "live": true, "readers": "sym_trap_function.cpp:20,77; DIRECT_DILATION_SAME_Z.cpp:21,95,117; DD_NEW_POLE_CONSTRAINT.cpp:17,69; DIRECT_DILATION_POLE_CONSTRAINT.cpp:17,71 (size() guard + 'other model' pose/z)", "writers": "optimizer_manager.cpp:707-727; bound to 3 CFMs"},
      {"member": "gpu_metrics_", "decl": "CostFunctionManager.h:158", "live": true, "readers": "every objective (ComputeSumWhitePixels, FastImplantDilationMetric, ImplantMahfouzMetric)", "writers": "optimizer_manager.cpp:730-738; shared pointer, 3 CFMs alias the SAME GPUMetrics"},
      {"member": "gpu_dilated_frames_A_/B_", "decl": "CostFunctionManager.h:162,168", "live": true, "readers": "all 7 objectives, indexed by current_frame_index_ (DIRECT_DILATION.cpp:22,28,60,75)", "writers": "per-stage VARIANT vectors created once in OptimizerManager::Initialize: gpu_dilated_frames_{trunk,branch,leaf}_{A,B}_ pushed at optimizer_manager.cpp:433-571; bound at 777-815; NEVER rewritten mid-run"},
      {"member": "gpu_intensity_frames_A_/B_", "decl": "CostFunctionManager.h:163,169", "live": "conditional", "readers": "ONLY DIRECT_MAHFOUZ.cpp:41,53 (ImplantMahfouzMetric); zero reads in the other 6 objectives", "writers": "3 stage-variant sets created in Initialize, optimizer_manager.cpp:305-402 (dark_silhouette differs per stage)"},
      {"member": "pose_storage_", "decl": "CostFunctionManager.h:190", "live": true, "readers": "3 objectives read other-model pose: sym_trap_function.cpp:22, DD_NEW_POLE_CONSTRAINT.cpp:19, DIRECT_DILATION_POLE_CONSTRAINT.cpp:19 (pose_storage_->GetModelPose(\"tibia\",...)); written by manager, not CFM", "writers": "optimizer_manager.cpp:138-143 (AddModel), 1260 (UpdatePrincipalModelPose)"},
      {"member": "biplane_mode_", "decl": "CostFunctionManager.h:192", "live": true, "readers": "all objectives (B-term gating); set from calibration_ at UploadData"},
      {"member": "gpu_edge_frames_A_/B_", "decl": "CostFunctionManager.h:160,166", "live": false, "evidence": "stored by UploadData (CostFunctionManager.cpp:125,129) and copied in operator= (78,82); repo-wide grep of src/ finds ZERO reads in any objective or CFM code — write-only. The underlying GPUEdgeFrame objects are likewise write-only in OptimizerManager (created 584-614, bound 778/781/790/793/802/805, deleted ~1542-1575)", "verdict": "DELETE"},
      {"member": "gpu_distance_maps_ / gpu_heatmaps_", "decl": "CostFunctionManager.h:170-171; bound via UploadDistanceMap", "live": false, "evidence": "only reader in the entire codebase is COMMENTED OUT: DIRECT_DILATION.cpp:37-38 (AllocateCurvatureHausdorfScore(heatmaps...)). GPUMetrics::DistanceMapMetric (compute/distance_map_metric.cu:66) and the CurvatureHausdorf kernel (compute/curvature_hausdorf_metric.cu) have no callers. See LEAK finding below", "verdict": "DELETE"},
      {"member": "prin_dist_", "decl": "CostFunctionManager.h:177", "live": false, "evidence": "never assigned anywhere (not in either ctor init list, CostFunctionManager.cpp:13-22/40-50, not in UploadData); only 'written' by copying it in operator= (CostFunctionManager.cpp:92) — an indeterminate-value member copied member-to-member. session_controller.cpp:56 hit is an unrelated local shadow", "verdict": "DELETE"},
      {"member": "current_frame_index_", "decl": "CostFunctionManager.h:182", "live": true, "readers": "objective fragments index A/B frame vectors with it; set per frame by the stage loop at optimizer_manager.cpp:929-931", "verdict": "PEEL — this is execution state, not a resource; becomes an argument to instance.initialize()/bind(frame) (the DirectDilationObjective worked example already takes the frame pointers at construction)"},
      {"member": "upload_epoch_", "decl": "CostFunctionManager.h:180", "live": false, "evidence": "see upload_epoch_verdict below", "verdict": "DELETE"},
      {"member": "stage_", "decl": "CostFunctionManager.h:186", "live": false, "evidence": "CFM.h:105-108 comment already documents it as dead constructor state; cfm_index in StageSpec is the source of truth", "verdict": "DELETE"}
    ]
  },
  "stage_frame_variants_who_writes_reads": {
    "writers": "Exactly one place: OptimizerManager::Initialize. Intensity: trunk 305-324, branch 344-363, leaf 383-402 (optimizer_manager.cpp; dark_silhouette differs per stage). Dilated: 420-580, leaf-first-then-branch-then-trunk order so the CPU dilation image 'shows trunk values' last (comment at 416-419). Edge: 584-614. Distance maps/heatmaps: 620-655. All created ONCE per run, bound to the 3 CFMs at 777-815.",
    "runtime_mutation": "NONE on GPU. ResetStageDilation (optimizer_manager.cpp:1284-1310) only rewrites the CPU-side cv::Mat dilation image + emits UpdateDilationBackground + BumpUploadEpoch; the GPU dilated/intensity variant vectors are never touched again for the rest of the run.",
    "readers": "The CFM objective fragments (member functions defined across src/compute/*.cpp, e.g. CostFunctionManager::costFunctionDIRECT_DILATION in DIRECT_DILATION.cpp:46) read (*gpu_dilated_frames_A_)[current_frame_index_] etc. at evaluate time; only the ACTIVE stage's variant is read during that stage.",
    "implication": "Variants are per-stage-CONFIG (dilation int / dark_silhouette bool baked into the uploaded pixels), not per-stage-lifetime. All 6 variant vectors (3 stages x A/B) are alive simultaneously for the whole run."
  },
  "upload_epoch_verdict": {
    "BumpUploadEpoch": "ONE production caller: ResetStageDilation, optimizer_manager.cpp:1308-1310 (confirmed in source; comment cites 'Plan 012 U2 (C7)'). Increment only.",
    "getUploadEpoch": "ZERO production callers. Defined CostFunctionManager.cpp:243-245, declared .h:103; consumers are test-only (test/unit/graph_key_assembler_test.cpp:199-203) and that test targets the REMOVED graph-recipe stack (AGENTS.md: graph_recipe/evaluation_* deleted 2026-08-28; test tree not built or run). No QML bridge, service, or header references it.",
    "verdict": "DEAD counter — increments that no one reads. It was the generation-identity input to the deleted graph-recipe key. DELETE (both methods and the member); note in history it existed solely to serve the removed graph cache."
  },
  "ownership_recommendation": {
    "choice": "PER-RUN, owned by the run-scoped binder living inside the fresh OptimizerManager (i.e. exactly where OptimizerManager::Initialize builds resources today); stages receive read-only references.",
    "reasoning": [
      "1. Lifetime is ALREADY run-scoped de facto: OptimizerRunDriver constructs a fresh OptimizerManager per run (src/coordinator/optimizer_run_driver.cpp:31) and ~OptimizerManager (optimizer_manager.cpp:1524-1588) frees every frame vector + models + metrics. A per-run bundle just replaces 17 hand-written delete loops with one owning struct; zero lifetime semantics change.",
      "2. Bundle CONTENTS are run-dependent: the stage variants bake in dilation/dark_silhouette derived from the run's active cost-function params (Initialize, optimizer_manager.cpp:283-296), and the dataset/models selected for that run. A session-scoped bundle would need in-place re-upload when parameters or datasets change — that is precisely the rewrite-in-place problem the dead upload_epoch_/C7 machinery was invented to paper over (buffered, not removed). Per-run sidesteps it entirely.",
      "3. Per-stage-frame ownership is wrong: the stage/frame INDEX is execution state (current_frame_index_, set per frame at 929-931). Resources are per-run, indexed at evaluate time; the objective instance receives its frame pointers at construction/binding.",
      "4. Fit with the owner vision: 'binder instantiates resources ONCE, passes by reference downward' maps 1:1 — the binder IS a reshaped OptimizerManager::Initialize; RunDirectStage (1290-1435) already passes the stage_manager by reference into jta::BuildGpuCostAdapter (1297-1300). Bundle-by-const-ref replaces CFM* in that signature."
    ]
  },
  "minimal_bundle_struct": {
    "note": "Plain structs of raw pointers/references — no new wrapping layer; every field is a today-existing CFM member or OptimizerManager vector, re-homed.",
    "struct": "struct StageFrameVariants {                 // one per stage (index = cfm_index)\n  std::vector<GPUDilatedFrame*> dilated_A;\n  std::vector<GPUDilatedFrame*> dilated_B;   // empty in monoplane\n  std::vector<GPUIntensityFrame*> intensity_A;  // needed only if a DIRECT_MAHFOUZ stage exists in the script\n  std::vector<GPUIntensityFrame*> intensity_B;\n};\nstruct ResourceBundle {\n  GPUModel* principal_model;\n  std::vector<GPUModel*> non_principal_models;\n  GPUMetrics* metrics;\n  PoseMatrix* pose_storage;                  // read-only for objectives (other-model pose)\n  bool biplane_mode;\n  std::array<StageFrameVariants, 3> stages;\n};",
    "cfm_members_to_bundle_fields": "gpu_principal_model_, gpu_non_principal_models_, gpu_metrics_, pose_storage_, biplane_mode_, and the six dilated/intensity frame pointer members become bundle fields (de-per-CFM'd: one shared bundle, stage-variant arrays keyed by cfm_index instead of one pointer member per CFM).",
    "cfm_members_that_die": "gpu_edge_frames_A_/B_ (write-only), gpu_distance_maps_/gpu_heatmaps_ (write-only + leak), prin_dist_ (indeterminate), upload_epoch_/BumpUploadEpoch/getUploadEpoch (unconsumed), current_frame_index_ (becomes instance.initialize(frame)/bind-time argument), stage_ (already dead).",
    "binder_once_wins": [
      "Bundle build can skip GPUIntensityFrame sets (6 images/frame/camera) unless the stage script's active objective is DIRECT_MAHFOUZ — today all 3 sets are always uploaded although only 1 of 7 objectives reads them.",
      "Binder can dedupe identical stage variants: if trunk/branch/leaf dark_silhouette are equal (common default), the 3 intensity sets are byte-identical uploads; same for dilated sets when stage dilation values coincide. Key the variant by its config value, alias when equal.",
      "Optional (bit-safe, flagged only): the 3 dilated sets are mutually exclusive in time — only the active stage's set is read during that stage — so a binder could upload per stage boundary instead of all up front. Same device bytes at first read; verify with the targeted A/B probe before adopting."
    ]
  },
  "memory_duplication": {
    "three_cfms": "NOT device duplication. All 3 CFMs store POINTERS to the same vectors/models/metrics (UploadData, CostFunctionManager.cpp:110-144; UploadDistanceMap 146-153). The 3-CFM duplication is 3 CPU copies of the cost-function config registry (available_cost_functions_ map) — config debt, not VRAM.",
    "real_device_copies_per_frame_per_camera": "1 GPUEdgeFrame (1 image, WRITE-ONLY dead) + 3 GPUDilatedFrames (1 image each, per-stage variant) + 3 GPUIntensityFrames (2 images each: base + inverted, gpu_intensity_frame.cuh:34-37) + 1 distance map (GPUFrame, dead) + 1 heatmap (dead) ~= 12 WxH grayscale images; in a default DIRECT_DILATION run only the 3 dilated variants are ever read (3x duplication of one logical image), the edge/intensity/distance-map/heatmap ~9 of 12 are dead weight unless DIRECT_MAHFOUZ is active (then intensity reads 2 of 6).",
    "per_run_leak": "gpu_distance_maps_ + gpu_heatmaps_ are NEVER deleted: ~OptimizerManager (optimizer_manager.cpp:1524-1588) frees intensity/edge/dilated frames, models, and metrics — no loop touches gpu_distance_maps_ or gpu_heatmaps_ (declared optimizer_manager.h:224-225). Under fresh-OptimizerManager-per-run every run leaks frames x (distance map + heatmap) of VRAM for resources that nothing reads."
  },
  "findings": [
    {"severity": "P1", "issue": "Distance maps + heatmaps: uploaded to GPU every run, never read, never freed (leak). Location: created optimizer_manager.cpp:620-655, bound 813-815, only reader is commented-out DIRECT_DILATION.cpp:37-38, no delete in ~OptimizerManager 1524-1588. Fix: DELETE both vectors, UploadDistanceMap, and the two bundle slots; keep the metric kernels (GPUMetrics::DistanceMapMetric, CurvatureHausdorf) on a delete list only if the owner wants them — they have zero callers.", "classification": "DELETE"},
    {"severity": "P1", "issue": "GPUEdgeFrame vectors are write-only: created (584-614), bound to 3 CFMs, deleted, never read by any code path. Fix: DELETE the upload of edge frames from the bundle; drop UploadData's two edge params. (Keep src/compute/gpu_edge_frame.cu until no test/oracle references it.)", "classification": "DELETE"},
    {"severity": "P2", "issue": "Intensity-frame stage variants always uploaded though read only by DIRECT_MAHFOUZ (DIRECT_MAHFOUZ.cpp:41,53). Fix: PEEL into the binder with need-based instantiation keyed off the stage script.", "classification": "PEEL"},
    {"severity": "P2", "issue": "upload_epoch_ machinery is dead consumption (Bump only, getUploadEpoch has zero production callers; consumer was the deleted graph-recipe key). Fix: DELETE BumpUploadEpoch/getUploadEpoch/member; the call site is inside ResetStageDilation 1305-1310 and goes with it.", "classification": "DELETE"},
    {"severity": "P2", "issue": "prin_dist_ is an indeterminate member (never assigned, copied in operator=, CostFunctionManager.cpp:92). Fix: DELETE with the CFM.", "classification": "DELETE"},
    {"severity": "P2", "issue": "3 CPU copies of the cost-function config registry (available_cost_functions_ per CFM) exist solely so DeriveStageParams/DeriveStageCostParams can re-read parameters at Initialize/stage time — config should come from the run's parameter snapshot, not 3 aliased registries.", "classification": "PEEL (config seam, noted here because it rides the same CFM instances)"}
  ],
  "bit_identity_caution": "Deleting edge frames / distance maps / heatmaps does NOT touch any read path (no reader exists), so cost values are bit-unchanged; still verify with a fresh targeted probe (goldens are not a gate). The bundle's per-stage variant ORDER of construction (leaf-dilate -> branch -> trunk, optimizer_manager.cpp:416-580) is an emit/behavior detail of Initialize only — the bundle binder must keep the CPU dilation-image end state 'trunk values' if anything downstream reads frames_A_ dilation images (ResetStageDilation + UpdateDilationBackground do at stage time, so safe)."
}

---

## Addendum — 2026-09-07 (post Tier-0 ownership hardening)

Status update after the GPU ownership-hardening pass
(`docs/solutions/architecture-patterns/ownership-hardening-compiler-as-probe-2026-09-07.md`;
full record in `docs/reviews/2026-09-07-gpu-ownership-hardening/`):

- **Borrowing convention now in force:** raw pointer = optional borrow, reference = required
  borrow, exclusive owners are `std::unique_ptr`. The "plain structs of raw
  pointers/references — no new wrapping layer" recommendation above stays valid **as
  borrows**: the bundle's pointer fields are views into optimizer-owned resources, not
  owners. Do not convert them to `unique_ptr`; prefer references where null is not
  meaningful.
- **Fixed by Tier-0:** the `GPUModel` biplane-constructor leak class — both render engines
  are now `std::unique_ptr` members, making construction exception-safe; `GPUModel` (like
  the other GPU frame classes) is non-copyable by construction.
- **Still open (not fixed by Tier-0):** the OptimizerManager leaks recorded above —
  `gpu_distance_maps_` / `gpu_heatmaps_` uploaded every run, never read, never freed
  (findings P1 #1), and the failed-Initialize manager leak. The Tier-0 pass deliberately did
  not touch them.
- **Tier-1 remains the named next step:** `cuda_device_buffer<T>` + `cuda_pinned_buffer<T>`
  RAII types for the CUDA-API-lifetime members (the `(void**)&member` allocation idiom
  blocks bare `unique_ptr`).
```