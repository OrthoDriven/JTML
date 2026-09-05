# Tier-0 Ownership Hardening Pass — Execution Report

**Date:** 2026-09-07
**Change:** `ozxrsznx` / `ed0045e` — `fix(ownership): fixing up ownership, lifetimes, and
dangling ptr` (working-copy change; also contains the earlier dead-code prep `pxxyyuns`
"remove dead code" as parent). This pass *continued and completed* a half-finished
conversion already present in that change.
**Scope discipline:** semantics-preserving RAII encoding of ownership that already exists.
No redesign, no `shared_ptr`, no new abstractions, no Tier-1 CUDA buffers touched.
**Inventory:** [00-gpu-ownership-inventory.md](00-gpu-ownership-inventory.md).

## 0. Context: the half-migrated working copy

On inspection the working copy was **not compiling** — an earlier in-progress session had
half-converted two items:

- `RenderEngine::renderer_output_`: header + `.cu` fully converted (`std::unique_ptr`,
  `make_unique`, `GPUImage& GetRenderOutput()`), stale `// delete /= 0` comments left behind.
- `GPUFrame::gpu_image_`: **header** converted to `std::unique_ptr`, but the `.cu` still did
  `new`/`delete`/`= 0` against it, and called `GetGPUImage()->WriteImage(...)` — impossible
  against the reference-returning accessor. The header had also been given an *inline*
  `IsInitializedCorrectly()` body while the `.cu` still defined it out-of-line (duplicate
  definition).

This pass finished that conversion instead of restarting it, and removed the residue.

## 1. Pre-edit ownership plan (verified against live source before editing)

| Item | Owner | Alloc site (pre-pass) | Destruct site (pre-pass) | Nullable? | Became | Borrow |
|---|---|---|---|---|---|---|
| `RenderEngine::renderer_output_` | RenderEngine | `render_engine.cu:357` (already `make_unique`) | automatic, after explicit `FreeCuda()` in dtor body — **ordering preserved** (members die after dtor body; the owned GPUImage borrows `dev_bounding_box_`) | no (after init) | `unique_ptr` (done; residue comments removed) | `GPUImage& GetRenderOutput()` (already) |
| `GPUFrame::gpu_image_` | GPUFrame | `gpu_frame.cu:15` (`new`) | `gpu_frame.cu:44` (`delete`) | no (param ctor); null on default ctor | `unique_ptr` (header done; `.cu` fixed) | `GPUImage& GetGPUImage()` (already) |
| `GPUIntensityFrame::gpu_inverted_image_` | GPUIntensityFrame | `gpu_intensity_frame.cu:16–17` (`new`) | `:34` (`delete`) | **yes** — default ctor leaves it absent | `unique_ptr` | `GPUImage* GetInvertedGPUImage()` **kept as pointer** (`.get()`) — genuine optional; no live callers |
| `GPUModel::primary_cam_render_engine_` | GPUModel | `gpu_model.cu:27, 69` (`new`) | `:110` (`delete`) | no | `unique_ptr` | internal `->` unchanged |
| `GPUModel::secondary_cam_render_engine_` | GPUModel | `gpu_model.cu:80` (`new`) | `:111` (`delete`) | **yes** (monoplane ⇒ null; `biplane_mode_`-guarded) | `unique_ptr` | signatures unchanged; null state stays internal |
| `generate_curvature_heatmaps::contour` | function local | `curvature_utilities.cpp:97–98` (`new`) | `:107`, `:149` (`delete`; leaks on throw) | no | **stack `std::vector` value** (not unique_ptr) | out-params `*` → `&` in `extract_contour_points` / `draw_contours` (each has exactly one caller, verified repo-wide) |

Copy/move implications going in: all five owning classes lose implicit copy ops; live storage
is exclusively `new T` behind raw pointers (`optimizer_manager.cpp` frame/model vectors,
CostFunctionManager binds), so no copies were expected. `test/oracle/*` binds `GPUImage*`
from the `GPUImage&` getter — legal implicit address-of; suite not built.

## 2. Files changed (this pass)

- `src/compute/gpu_frame.cu` — make_unique; `GetGPUImage().WriteImage`; removed `= 0`/`delete`;
  default-ctor comment corrected.
- `include/compute/gpu_frame.cuh` — restored `IsInitializedCorrectly()` from erroneous inline
  definition to declaration (residue of the earlier half-conversion).
- `src/compute/gpu_intensity_frame.cu` — make_unique; dtor delete removed.
- `include/compute/gpu_intensity_frame.cuh` — member → `std::unique_ptr<GPUImage>`; `+<memory>`;
  accessor comment now states the nullability contract.
- `src/compute/gpu_model.cu` — both ctors → `make_unique`; `= 0` sentinels and dtor deletes
  removed; dtor shell + explanatory comment kept.
- `include/compute/gpu_model.cuh` — both members → `std::unique_ptr<RenderEngine>`; `+<memory>`.
- `src/compute/gpu_metrics.cu` + `include/compute/gpu_metrics.cuh` —
  `ComputeSumWhitePixels(GPUImage* → GPUImage&, cudaError* stays pointer)` (see §4).
- `src/compute/curvature_utilities.cpp` + `include/compute/curvature_utilities.h` —
  `contour` → stack value; `extract_contour_points`/`draw_contours` `*contour` → `&contour`;
  both `delete contour` sites removed.
- `src/compute/render_engine.cu` — removed 5 stale `// delete renderer_output_; /
  // renderer_output_ = 0;` comment lines (residue only).

*(Pre-existing in the same change, not this pass: `bacon.toml`, `gpu_toolbox.h` deletion,
`tools/check-affected-cpp.py`, `fast_implant_dilation_metric.cu`, `implant_mahfouz_metric.cu`,
`render_engine.cuh` conversion, `FastImplantDilationMetric`/`ImplantMahfouzMetric`
`GPUImage&` signatures.)*

## 3. Ownership conversions performed

5 member conversions to `std::unique_ptr` (renderer_output_ ✅ completed, gpu_image_ ✅
completed, gpu_inverted_image_, primary/secondary_cam_render_engine_) + 1 stack-value
conversion (contour). 6 `make_unique` sites; 0 `new GPUImage` / `new RenderEngine` remain;
0 manual deletes of the converted members remain; 0 `shared_ptr`; 0 Tier-1 CUDA allocations
touched (verified by diff greps: no added `cudaMalloc`/`cudaHostAlloc`).

## 4. Accessor / borrow changes performed

- `GetRenderOutput()` / `GetGPUImage()` — already returned `GPUImage&` (required borrows);
  consumers (`DIRECT_DILATION.cpp`, `objectives/direct_dilation.cpp`, `Study2Grid`,
  `shape_sensitivity`, oracle tests) compile unchanged.
- `GetInvertedGPUImage()` — kept `GPUImage*` via `.get()`: null is meaningful (default-
  constructed frame has no inverted image). Explicit optional accessor.
- `ComputeSumWhitePixels(GPUImage* image, cudaError*)` → `(GPUImage& image, cudaError*)` —
  the one API change the compiler forced. The image is a required, non-null, non-mutating
  borrow at all four call sites; this matches the class's existing
  `FastImplantDilationMetric(GPUImage&, …)` convention. The `cudaError*` out-param stays a
  pointer.
- `extract_contour_points(cv::Mat, std::vector<…>&)` and
  `draw_contours(std::vector<…>&)` — required-borrow out-params (were raw owning-looking
  pointers).

## 5. Copy/move assumptions exposed by the compiler

- Exactly four non-toolchain errors: `no viable conversion from 'GPUImage' to 'GPUImage*'`
  at the `ComputeSumWhitePixels` call sites (`objectives/direct_dilation.cpp:7,11`,
  `DIRECT_DILATION.cpp:22,28`) — classified **required borrow**, resolved via §4. All four
  call sites then compiled unchanged.
- **Zero deleted-copy breakage:** nothing in live source copies `GPUFrame` /
  `GPUIntensityFrame` / `GPUModel` / `RenderEngine` by value. All four classes are now
  non-copyable by construction — the pre-pass implicit shallow-copy double-delete hazard is
  structurally gone. Per instruction, copyability was **not** restored anywhere.

## 6. Latent bugs fixed incidentally by correct RAII semantics

1. **`GPUIntensityFrame` garbage delete** — default ctor left `gpu_inverted_image_`
   indeterminate; dtor deleted it on default-constructed objects. Default-initialized
   `unique_ptr` removes the hazard.
2. **GPUModel biplane ctor leak** — primary engine leaked if the secondary `new RenderEngine`
   threw (raw assignment, no try/catch). `make_unique` members make construction
   exception-safe.
3. **`contour` leak on throw** — heap vector leaked if anything between `new`/`delete` threw
   (e.g. `cv::imwrite`). Stack value eliminates it.
4. **Half-migration artifacts** — non-compiling `GetGPUImage()->WriteImage(...)`, and the
   duplicate `GPUFrame::IsInitializedCorrectly` definition (header inline + `.cu`
   out-of-line); both fixed as part of completing the conversion.
5. Preserved-by-design (not a bug fixed, but worth restating): `~RenderEngine` keeps its
   explicit `FreeCuda()` so the `dev_bounding_box_` borrow held by the owned `GPUImage` is
   freed while the GPUImage still exists but is never dereferenced during teardown — exact
   pre-pass ordering retained.

## 7. Deliberately deferred

- **Tier 1 CUDA buffers (untouched, confirmed):** RenderEngine's 16 device/pinned buffers,
  GPUMetrics' 12, GPUImage's `dev_image_`/`bounding_box_`, GPUHeatmap's `dev_heatmap_`.
  Plan: `cuda_device_buffer<T>` + `cuda_pinned_buffer<T>` RAII types, then sweep
  RenderEngine → GPUMetrics → GPUImage/GPUHeatmap, one class per rebuild. This structurally
  fixes the inventory's double-free-on-ctor-failure bugs (`GPUImage` ×2, `RenderEngine
  fragment_fill_`) and the GPUMetrics lazy-alloc leak.
- **Adjacent, out of scope, still open:**
  - `curvature_utilities.cpp` `curvature`/`smoothed_curvature`/`curvature_derivative`:
    `new float[N]` freed with **scalar `delete`** (`:150–152`) — live UB, compiles cleanly.
    Fix as `std::vector<float>` in a follow-up.
  - Non-virtual `GPUFrame` dtor with three derived classes (base-pointer delete = UB).
  - `GetSecondaryCameraRenderedImage()` returns `GPUImage&` on a null-capable secondary
    engine (pre-existing semantics preserved; all live call sites biplane-guarded).
  - CostFunctionManager's external borrow bundle (`02-resource-bundle.md` territory):
    references/views, never ownership transfer.
  - `calculate_curvature_along_contour` taking `std::vector<...>` by value (perf nit, not
    ownership).

## 8. Final checker / build status

- `pixi run ./tools/check-affected-cpp.py -B .build` — 36 TUs; **0 errors attributable to
  this change**. 15 CUDA TUs fail on a pre-existing clang↔CUDA-13 toolchain mismatch inside
  CUDA SDK headers (`math_functions.hpp` / `texture_fetch_functions.h`; clang in pixi
  partially supports ≤12.8) — identical failure set before and after the change.
- `pixi run ninja-check` — **exit 0**; full build, all executables linked (incl.
  `bin/joint-track-machine-learning`).

## Related

- [00-gpu-ownership-inventory.md](00-gpu-ownership-inventory.md) — full Tier-0/1/borrow/dead inventory.
- [`../2026-09-04-seam-investigation/02-resource-bundle.md`](../2026-09-04-seam-investigation/02-resource-bundle.md) — CFM member verdicts (prin_dist_ DELETE, etc.).
- `JTML_HEADLESS_RUNTIME_ARCHITECTURE.md` (2026-09-02 living architecture doc) — the
  lifetime-distinction framing this cut serves; the ownership-hardening step ("Cut 1.5")
  was discussed but had not been written into it as a numbered migration cut at the time of
  the inventory.
- `docs/jtml_cuda_d2h_hotpath_notes.org` — hot-path notes (unrelated to this pass).
