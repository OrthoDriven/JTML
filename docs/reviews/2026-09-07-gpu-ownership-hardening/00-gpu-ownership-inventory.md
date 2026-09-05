# GPU Ownership Inventory — `src/compute` + `include/compute`

**Date:** 2026-09-07
**Scope:** full sweep of raw-pointer ownership over `src/compute/` (`.cpp`/`.cu`) and
`include/compute/` (`.h`/`.cuh`/`.hpp`), 57 files / ~9.5k lines.
**Purpose:** supply the inventory for the *ownership-hardening migration cut* (informally
"Cut 1.5" of the living architecture work, `JTML_HEADLESS_RUNTIME_ARCHITECTURE.md`, 2026-09-02):
convert plainly-owning raw pointer members to `std::unique_ptr`, rebuild, and let compiler
errors enumerate every ownership call site, classifying each as *own / borrow / share*.
**Companion:** [01-tier0-ownership-pass.md](01-tier0-ownership-pass.md) — the executed Tier-0 pass.

Ownership vocabulary for this effort:

```cpp
std::unique_ptr<T>   // exclusive heap ownership
T&  / const T&       // required (mutable / read-only) borrow
T*  / const T*       // optional/nullable borrow only
```

## Method

This inventory was produced by a **subagent swarm** (top-level `workflow` run
`gpu_ownership_sweep`, 2026-09-07): five parallel read-only analyst agents, one per
ownership-focused lane, each returning structured findings (member, declared type, acquire
site, release site, classification, `unique_ptr` candidate, egregiousness 1–5, break sites).
Four lanes ran as subagents (`render-core`, `frames`, `model-camera`, `metrics`); the
`orchestration` lane's agent failed twice on runner infrastructure (async runner process died
with `MODULE_NOT_FOUND`, run id `e68f2841-80fe-43fa-98e8-cdded689eda6`) and was completed by
direct grep/read in-session. The pi-subagents async run artifacts were ephemeral (not
retained under `/tmp/pi-subagents-uid-1000/`), so the substantive findings are embedded
below rather than linked.

## Headline totals

- **41 live raw-pointer members** across 5 classes, plus 4 function-local owning pointers.
- **Only 6 are clean `unique_ptr = yes` flips** (plain `new`/`delete` pairs) — these became Tier 0.
- **~30 are CUDA-API lifetimes** (`cudaMalloc` / `cudaHostAlloc`) that want one small
  `cuda_device_buffer<T>` / `cuda_pinned_buffer<T>` RAII type first — Tier 1, **do not**
  convert to bare `unique_ptr` (the `(void**)&member` allocation idiom blocks a plain flip).
- Everything else is either a **genuine borrow** (leave raw/reference) or **dead** (delete,
  don't convert).
- **Cross-cutting:** no class in the sweep deleted its copy operations — every owning class
  compiled implicit shallow copies (double-free hazard). The `unique_ptr` conversions turn
  that class of bug into a compile error; that *is* the compiler-yelling phase working.
- **Cross-cutting:** `GPUFrame`'s destructor is **non-virtual** with three derived classes
  (`GPUIntensityFrame`, `GPUEdgeFrame`, `GPUDilatedFrame`) — deleting via base pointer is UB.
  Fix in the hardening cut (still open).

---

## Lane 1 — render-core (`RenderEngine`, `GPUImage`, `gpu_image_functions`)

### `RenderEngine` (`include/compute/render_engine.cuh`, `src/compute/render_engine.cu`)

22 raw-pointer members + 1 dead 17-field struct:

| Member | Type | Acquire / Release | Class | Candidate | Egr. |
|---|---|---|---|---|---|
| `renderer_output_` (cuh:141) | `GPUImage*` | `new` in `InitializeCUDA` (cu:357) / `delete` in dtor (cu:262) | owns | **yes** — textbook | 2 |
| `dev_triangles_` (159) | `float*` | cudaMalloc cu:316 / cudaFree cu:267 | owns | partial (deleter) | 2 |
| `dev_normals_` (165) | `float*` | cudaMalloc cu:318 / cudaFree cu:268 | owns | partial | 2 |
| `dev_backface_` (173) | `bool*` | cudaMalloc cu:320 / cudaFree cu:269 | owns | partial | 2 |
| `dev_projected_triangles_` (183) | `float*` | cudaMalloc cu:322 / cudaFree cu:270 | owns | partial | 2 |
| `dev_projected_triangles_snapped_` (187) | `int*` | cudaMalloc cu:325 / cudaFree cu:271 | owns | partial | 2 |
| `dev_bounding_box_triangles_` (194) | `int*` | cudaMalloc cu:329 / cudaFree cu:272 | owns | partial | 2 |
| `dev_bounding_box_triangles_sizes_` (198) | `int*` | cudaMalloc cu:332 / cudaFree cu:273; in/out of `cub::DeviceScan::ExclusiveSum` (cu:371–376) | owns | partial | 2 |
| `dev_bounding_box_triangles_sizes_prefix_` (203) | `int*` | cudaMalloc cu:336 / cudaFree cu:274 | owns | partial | 2 |
| `dev_bounding_box_` (207) | `int*` | cudaMalloc cu:341 / cudaFree cu:276; **handed to `GPUImage::SetDeviceBoundingBox` (cu:358)** | owns (cross-object borrow downstream) | partial | 3 |
| `dev_fragment_fill_` (208) | `int*` | cudaMalloc cu:343 / cudaFree cu:277 | owns | partial | 2 |
| `dev_stride_prefixes_` (216) | `int*` | cudaMalloc cu:345 (~10M ints) / cudaFree cu:278 | owns | partial | 2 |
| `dev_cub_storage_` (221) | `void*` | size-query then cudaMalloc cu:371–378 / cudaFree cu:275 | owns | partial (`unique_ptr<void, cudaFreeDeleter>`) | 2 |
| `dev_nextCandidate_` (225) | `int*` | cudaMalloc cu:380 / cudaFree+null cu:280,283 | owns | partial (tidy) | 1 |
| `dev_nextChunk_` (226) | `int*` | cudaMalloc cu:381 / cudaFree+null cu:281,284 | owns | partial (tidy) | 1 |
| `dev_overflowFlag_` (227) | `int*` | cudaMalloc cu:382 / cudaFree+null cu:282,285 | owns | partial (tidy) | 1 |
| `host_overflowFlag_` (228) | `int*` | cudaHostAlloc cu:383 / cudaFreeHost+null cu:289 | owns | partial (pinned) | 1 |
| `fragment_fill_` (135) | `int*` | cudaHostAlloc cu:311 / cudaFreeHost cu:288 (**not nulled** → double-free on failed init: error paths cu:292–296/349–353 + dtor cu:258–263) | owns | partial | 2 |
| `dev_bounding_box_` (207) → see above | | | | | |
| `dev_tangent_triangle_` (151) | `bool*` | **never allocated, used, or freed** | unclear (dead) | no — delete it | 3 |
| `active_output_device_` (269) | `unsigned char*` | never acquired/released; zero refs | unclear (dead, graph-stack residue) | no — delete | 2 |
| `active_bounding_box_host_` (270) | `int*` | never acquired/released; zero refs | unclear (dead) | no — delete | 2 |
| `RenderPointerSet` (249–267) | 17-field struct of raw ptrs | never instantiated | unclear (dead) | no — delete | 1 |

Systemic hazards in this lane:
1. Neither class deletes copy ops → implicit shallow copies compile today.
2. `FreeCuda()` does not null most pointers; failed `InitializeCUDA` calls `FreeCuda` then the
   dtor calls it again → **double cudaFree on failed init** (the U4 counter members, which are
   explicitly nulled, are the exception).
3. `~RenderEngine` frees `dev_bounding_box_` (via `FreeCuda`, cu:260) **before** deleting
   `renderer_output_` (cu:262) — the owned `GPUImage` holds a borrow of exactly that buffer;
   dangling-but-not-dereferenced teardown ordering. Preserve/annotate when converting.

Recommended shape for the CUDA members: one `cuda_device_buffer<T>` RAII type
(cudaFree / cudaFreeHost deleters) rather than raw `unique_ptr`, since 16 of 22 owning
members are CUDA-API lifetimes.

### `GPUImage` (`include/compute/gpu_image.cuh`, `src/compute/gpu_image.cu`)

| Member | Type | Detail | Class | Candidate | Egr. |
|---|---|---|---|---|---|
| `dev_image_` (cuh:63) | `unsigned char*` | cudaMalloc in both ctors (cu:43–44, 99) / cudaFree in dtor (cu:150) **and** ctor error paths (cu:51) **not nulled** → **concrete double-free on ctor failure** | owns | partial | 3 |
| `bounding_box_` (cuh:86) | `int*` | cudaHostAlloc (cu:21–22, 74–75) / cudaFreeHost in dtor (cu:151) **and** ctor-2 error paths (cu:109, 127) → **concrete double-free on ctor failure**; default ctor leaves `0` (dtor frees nullptr) | owns | partial (pinned) | **4 — worst member in sweep** |
| `dev_bounding_box_` (cuh:87) | `int*` | set via `SetDeviceBoundingBox(int*)` from `RenderEngine::InitializeCUDA` (cu:358); never freed by GPUImage | **borrows** | **no** — genuine non-owning borrow with the teardown-ordering hazard above | 2 |

### `gpu_image_functions.cu`

Function-local `dev_max`/`dev_min` (lines 371–374) properly cudaFreed (395–396), never
escape — excluded from the member inventory; a `unique_ptr` + cudaFree deleter would be free
hygiene if touched.

---

## Lane 2 — frame types

| Class / Member | Detail | Class | Candidate | Egr. |
|---|---|---|---|---|
| `GPUFrame::gpu_image_` (`gpu_frame.cuh:56` pre-pass numbering) | `new` in param ctor (cu:15) / `delete` in dtor (cu:44); `= 0` in default ctor; **no deleted copy ops** (implicit shallow copy = double delete); **non-virtual dtor with 3 derived classes** | owns | **yes** | 2 |
| `GPUIntensityFrame::gpu_inverted_image_` (`gpu_intensity_frame.cuh:37`) | `new` in param ctor / `delete` in dtor; **default ctor left it indeterminate → dtor deleted garbage on default-constructed objects (live latent bug)** | owns | **yes** — `unique_ptr` fixes for free | 3 |
| `GPUHeatmap::dev_heatmap_` (`gpu_heatmaps.cuh:30`) | cudaMalloc (cu:42–44) / cudaFree (dtor cu:74–76 + error paths cu:49, 63 — the :49 path would free a failed cudaMalloc result) | owns | partial (deleter) | 2 |
| `GPUEdgeFrame`, `GPUDilatedFrame` | int-only members, no pointers | — | none | — |
| host `Frame` (`frame.h`) | all `cv::Mat` / `std::vector` | — | **clean, nothing to do** | — |

---

## Lane 3 — model & camera

| Class / Member | Detail | Class | Candidate | Egr. |
|---|---|---|---|---|
| `GPUModel::primary_cam_render_engine_` (`gpu_model.cuh:137`) | `new` in both param ctors (cu:27, 69) / `delete` (cu:110); **biplane ctor leaks the primary engine if the secondary `new` throws** (no try/catch, raw assignment) | owns | **yes** — `make_unique` makes it exception-safe | 2 |
| `GPUModel::secondary_cam_render_engine_` (`gpu_model.cuh:139`) | `new` in biplane ctor only (cu:80) / `delete` (cu:111); null in monoplane/default (guarded everywhere by `biplane_mode_`) | owns (conditionally present) | **yes** | 2 |

Non-findings: `camera_calibration.h` — value-only, clean. `pose_matrix.h/.cpp` —
`GetModelPose(Pose*)` params are **non-owning out-slots, do not harden**. `gpu_arena.cuh` —
`DeviceBuffer<Triangle>` value member, but the file itself is a non-compiling fragment
(missing semicolon, unqualified names) — presumably in no build target.
`float* triangles / normals` in GPUModel ctor signatures are pass-throughs into RenderEngine —
caller-side ownership is a render-engine-lane question.

---

## Lane 4 — metrics

`GPUMetrics` (`include/compute/gpu_metrics.cuh:118–179`, `src/compute/gpu_metrics.cu`) is the
only live class with pointer members here — **12 owning CUDA allocations**, all freed in
`~GPUMetrics` (cu:143–161):

| Member | Kind | Note |
|---|---|---|
| `pixel_score_` (134) / `intersection_score_` (140) / `union_score_` (141) / `edge_pixels_count_` (161) / `distance_map_score_` (174) | pinned host (`cudaHostAlloc`) | ctor/dtor pairs, cudaFreeHost deleter needed |
| `dev_pixel_score_` (137) / `dev_intersection_score_` (144) / `dev_union_score_` (145) / `dev_white_pix_count_` (158) / `dev_edge_pixels_count_` (162) / `dev_distance_map_score_` (175) | device (`cudaMalloc`) | ctor/dtor pairs; `dev_white_pix_count_` also passed to kernels (cu:215/229/234 → `.get()` sites) |
| `curvature_hausdorf_score_` (178) / `dev_curvature_hausdorf_score_` (179) | pinned + device, **lazily allocated** in `AllocateCurvatureHausdorfScore` (cu:167–184) | **no free-before-alloc check → second call silently leaks the first allocation**; dtor unconditionally frees possibly-never-allocated pointers (egregiousness 3) |

Cross-cutting: GPUMetrics holds 12 owning pointers with **no deleted copy ops**. All kernel
`.cu` files in the lane (`fast_implant_dilation_metric.cu`, `implant_mahfouz_metric.cu`,
`distance_map_metric.cu`, `iou.cu`, `l_1_1_matrix_diff_norm.cu`,
`dilate_edge_detected_image.cu`, `edge_detect_rendered_implant_model.cu`,
`curvature_hausdorf_metric.cu`) contain only free kernels + GPUMetrics member-function
bodies — pointers there are **kernel parameters borrowing buffers owned by
GPUMetrics/GPUImage**; no members, statics, allocations, or leaks.
`registration_metric.cu/.cuh` and `metric_toolbox.cu/.cuh` are **fully commented out** —
historical, not inventoried.

### `curvature_utilities.cpp` (function-local owners)

| Local | Detail | Class | Candidate | Egr. |
|---|---|---|---|---|
| `contour` (:97–98) | `new std::vector<std::vector<cv::Point_<int>>>` passed as raw **owning out-param** into `extract_contour_points` (:101) / `draw_contours` (:110); leaked on any throw between new/delete | owns (function-local) | **stack value, not unique_ptr** | 3 |
| `curvature` (:113) / `smoothed_curvature` (:121) / `curvature_derivative` (:127) | `new float[N]` freed with **scalar `delete`** (:150–152) — **compiles but is UB (heap corruption)**; leaks on throw | owns (function-local) | `std::vector<float>` / `unique_ptr<float[]>` | **4** |
| `p1`/`p2` in `pick_three_points` (:22–23, 32–33) | well-behaved scoped `new cv::Point_<int>` | owns, tidy | fine as-is (or value) | 1 |

---

## Lane 5 — orchestration (completed in-session after agent failures)

**`CostFunctionManager`** (`include/compute/CostFunctionManager.h:130–155`) holds the entire
runtime-resource bundle: `gpu_metrics_`, `gpu_principal_model_`, `gpu_non_principal_models_`
(ptr-to-vector), six `std::vector<…Frame*>*` pointer-to-vectors, `gpu_distance_maps_`,
`gpu_heatmaps_`, `pose_storage_`, `prin_dist_`. **The dtor is `= default` — CFM owns
nothing.** All pointers are *bound from outside* (ctor / `UploadData`) and shallow-copied in
the copy-ctor (cu:58–97). This is the study/model-lifetime vs evaluation-lifetime boundary;
hardening here means **references / views, never ownership transfer**.

- `prin_dist_` (`:150`) — **indeterminate value** (never assigned; only copied member-to-member,
  cu:92). Already verdict'd **DELETE** in
  [`02-resource-bundle.md`](../2026-09-04-seam-investigation/02-resource-bundle.md).
- `gpu_distance_maps_` / `gpu_heatmaps_` — write-only + leak per the same review.
- `active_objective_instance_` (`:153`) — already `std::unique_ptr<ObjectiveInstance>`; the
  model citizen of the file.
- All other lane files (`CostFunction.h/.cpp`, `Parameter.h`, `Stage.h`,
  `DIRECT_DILATIONCustomVariables.h/.cpp`, `machine_learning_tools.h/.cpp`, `gpu_toolbox.h`,
  `launch_config.cuh`, `cuda_launch_parameters.h`, `objective_spec.*`,
  `objective_instance.*`) — **no raw-pointer members**. (`gpu_toolbox.h` was since deleted in
  the dead-code change `pxxyyuns` "remove dead code".)

---

## Recommended sequencing for the ownership-hardening cut

1. Delete the dead list (`dev_tangent_triangle_`, `active_output_device_`,
   `active_bounding_box_host_`, `RenderPointerSet`, `prin_dist_`, commented-out
   `registration_metric`/`metric_toolbox`, `gpu_arena.cuh` fragment) — zero-risk. Note: as of
   2026-09-07 the render-engine dead members are **still present** in the tree (verified);
   `pxxyyuns` "remove dead code" removed other dead material (legacy tests, `gpu_toolbox.h`,
   metric residue) but not these. Still open.
2. **Tier 0** — flip the six clean `new`/`delete` members; rebuild; classify every
   copy/assignment error as own/borrow/share. *(Executed 2026-09-07 — see
   [01-tier0-ownership-pass.md](01-tier0-ownership-pass.md).)*
3. **Tier 1** — introduce `cuda_device_buffer<T>` + `cuda_pinned_buffer<T>`; sweep
   RenderEngine → GPUMetrics → GPUImage/GPUHeatmap, one class per rebuild. Fixes the
   double-free / lazy-alloc bugs structurally.
4. Fix the non-virtual `GPUFrame` dtor in the same cut.
