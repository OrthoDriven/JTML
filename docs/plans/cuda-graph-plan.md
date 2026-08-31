# JTML DIRECT_DILATION CUDA Graph implementation handoff

## Objective

Implement one narrowly scoped CUDA Graph fast path for the current **monoplane `DIRECT_DILATION` scalar evaluation**.

Do **not** redesign the renderer, metric, optimizer, batching system, launch grids, CUB scan, rasterizer, pose parameterization, or any other cost function.

The goal is only to replace repeated host submission of the already-optimized GPU chain with:

```cpp
update WorldToPixel pose parameters
cudaGraphLaunch(...)
cudaStreamSynchronize(...)
read pinned score
```

while keeping the current non-graph path intact as a correctness/performance fallback.

The current hot path has already been reduced to a fully device-resident render/metric pipeline with exactly one required host result at the end.

---

# 1. Scope

Implement graph acceleration for:

```text
DIRECT_DILATION
monoplane only
primary camera only
DistanceMapMetric disabled
```

Do **not** graph:

* biplane mode yet;
* `DIRECT_DILATION_T1`;
* pole-constraint variants;
* Mahfouz;
* DRR;
* distance-map metric;
* curvature;
* batching;
* multiple simultaneous graph executions.

If any unsupported condition occurs, use the existing scalar path.

This should be an optional fast path, not a replacement that destroys the known-good implementation.

---

# 2. Current production evaluation that must remain numerically identical

The current `DIRECT_DILATION` evaluation is:

```cpp
gpu_principal_model_->RenderPrimaryCamera(
    gpu_principal_model_->GetCurrentPrimaryCameraPose());

metric_score =
    DIRECT_DILATION_current_white_pix_sum_dilated_comparison_image_A_ +
    gpu_metrics_->FastImplantDilationMetric(
        gpu_principal_model_->GetPrimaryCameraRenderedImage(),
        gpu_dilated_frames_A_->at(current_frame_index_),
        DIRECT_DILATION_current_dilation_parameter);
```

Distance-map scoring is currently commented out.

The graph implementation must therefore compute exactly the same GPU quantity as:

```cpp
gpu_metrics_->FastImplantDilationMetric(...)
```

and the existing host-side constant:

```cpp
DIRECT_DILATION_current_white_pix_sum_dilated_comparison_image_A_
```

should continue to be added on the CPU after graph completion.

Do **not** put that white-pixel constant into the graph.

---

# 3. Current pose path — very important

The pose used for each evaluation currently comes from:

```cpp
gpu_principal_model_->GetCurrentPrimaryCameraPose()
```

and then:

```cpp
GPUModel::RenderPrimaryCamera(Pose model_pose)
```

calls:

```cpp
primary_cam_render_engine_->SetPose(model_pose);
primary_cam_render_engine_->Render();
```

`RenderEngine::SetPose()` stores all six pose values into:

```cpp
model_pose_
```

and converts the Euler angles into:

```cpp
model_rotation_mat_
```

on the host.

The rotation formula is currently:

```cpp
float cz = cos(model_pose_.z_angle_ * 3.14159265358979323846f / 180.0f);
float sz = sin(model_pose_.z_angle_ * 3.14159265358979323846f / 180.0f);
float cx = cos(model_pose_.x_angle_ * 3.14159265358979323846f / 180.0f);
float sx = sin(model_pose_.x_angle_ * 3.14159265358979323846f / 180.0f);
float cy = cos(model_pose_.y_angle_ * 3.14159265358979323846f / 180.0f);
float sy = sin(model_pose_.y_angle_ * 3.14159265358979323846f / 180.0f);

model_rotation_mat_ = RotationMatrix(
    cz * cy - sz * sx * sy,
    -1.0 * sz * cx,
    cz * sy + sz * cy * sx,
    sz * cy + cz * sx * sy,
    cz * cx,
    sz * sy - cz * cy * sx,
    -1.0 * cx * sy,
    sx,
    cx * cy);
```

Do **not** move this computation to the GPU for the first implementation.

Do **not** introduce a device pose struct for the first implementation.

Continue calling:

```cpp
RenderEngine::SetPose(pose)
```

once per evaluation.

The graph's `WorldToPixelKernel` node will then be updated with the new:

```cpp
model_pose_.x_location_
model_pose_.y_location_
model_pose_.z_location_
model_rotation_mat_
```

using:

```cpp
cudaGraphExecKernelNodeSetParams()
```

This is exactly the CUDA-supported use case for a graph whose topology is static but whose kernel parameters vary between launches. NVIDIA specifically documents updating an instantiated kernel node this way.

---

# 4. Current GPU chain that must be captured

The current `RenderEngine::Render()` contains:

```text
WorldToPixelKernel
BoundingBoxForTrianglesKernel
BoundingBoxSizesKernel
CUB DeviceScan::ExclusiveSum
FillTriangleKernel_new
```

`WorldToPixelKernel` now also:

1. clears the previous output image;
2. resets the global device bounding box;
3. transforms/projects the vertices;
4. writes snapped projected vertices;
5. computes backface flags.

So there is no separate image `cudaMemset()` or bbox-reset kernel in the current scalar render chain.

The metric then contains:

```text
FastImplantDilationMetric_EdgeKernel_new
FastImplantDilationMetric_DilateKernel_new
FastImplantDilationMetric_DifferenceKernel_new
cudaMemcpy score D2H
```

CUB itself currently produces two GPU kernels in Nsight:

```text
DeviceScanInitKernel
DeviceScanKernel
```

Therefore the captured graph should look approximately like:

```text
WorldToPixel
    ↓
BoundingBoxForTriangles
    ↓
BoundingBoxSizes
    ↓
CUB DeviceScanInit
    ↓
CUB DeviceScan
    ↓
FillTriangle_new
    ↓
Edge_new
    ↓
Dilate_new
    ↓
Difference_new
    ↓
4-byte D2H memcpy
```

That is approximately **10 graph nodes**.

The exact number of CUB-internal nodes may vary with CUDA/CCCL version; do not hard-code the expected node count.

---

# 5. Do not change launch geometry

The graph experiment must capture the current launch configuration exactly.

At present:

```cpp
FillTriangleKernel_new
```

uses occupancy-derived:

```cpp
fill_triangle_grid_ =
    props.multiProcessorCount * blocks_per_sm;
```

and on the current GPU this has been measured as:

```text
6 blocks/SM
504 blocks total
```

Current metric launch policy is:

```text
Edge:       occupancy-derived = 504 blocks
Dilate:     2 blocks/SM       = 168 blocks
Difference: 2 blocks/SM       = 168 blocks
```

Do not change any of these while implementing graphs.

Do not combine graph work with another occupancy experiment.

Do not replace CUB.

Do not fuse bounding-box kernels.

Do not use the warp rasterizer.

The graph benchmark must answer only:

> Does graph submission improve the current known-good GPU pipeline?

---

# 6. CUB must remain exactly the current CUB scan

The renderer already allocates CUB temporary storage once.

It first queries the required storage size and then:

```cpp
cudaMalloc(&dev_cub_storage_, cub_storage_bytes_);
```

That is ideal for stream capture because the actual hot-path CUB call does not need to allocate temporary storage.

The only necessary change is to explicitly provide the capture stream.

Current:

```cpp
cub::DeviceScan::ExclusiveSum(
    dev_cub_storage_,
    cub_storage_bytes_,
    dev_bounding_box_triangles_sizes_,
    dev_bounding_box_triangles_sizes_prefix_,
    triangle_count_);
```

Graph-capable version:

```cpp
cub::DeviceScan::ExclusiveSum(
    dev_cub_storage_,
    cub_storage_bytes_,
    dev_bounding_box_triangles_sizes_,
    dev_bounding_box_triangles_sizes_prefix_,
    triangle_count_,
    stream);
```

The CUB API explicitly exposes `cudaStream_t stream` as the final optional parameter.

Do not manually reconstruct CUB's graph nodes.

Let stream capture capture CUB's own kernel launches.

---

# 7. Create a stream-aware render enqueue path

Do not delete the current:

```cpp
cudaError_t RenderEngine::Render()
```

until the graph version has been proven.

Add:

```cpp
cudaError_t EnqueueRender(cudaStream_t stream);
```

to `RenderEngine`.

The body should be the current render chain, with the exact same arguments, except every kernel uses the supplied stream and CUB receives that stream.

Conceptually, it should be exactly:

```cpp
cudaError_t RenderEngine::EnqueueRender(cudaStream_t stream) {
    WorldToPixelKernel<<<
        dim_grid_vertices_,
        threads_per_block,
        0,
        stream>>>(
        dev_triangles_,
        dev_projected_triangles_,
        dev_projected_triangles_snapped_,
        3 * triangle_count_,
        dist_over_pix_pitch_,
        pix_conversion_x_,
        pix_conversion_y_,
        model_pose_.x_location_,
        model_pose_.y_location_,
        model_pose_.z_location_,
        model_rotation_mat_,
        dev_normals_,
        dev_backface_,
        use_backface_culling_,
        fx_,
        fy_,
        cx_,
        cy_,
        renderer_output_->GetDeviceImagePointer(),
        dev_bounding_box_,
        width_,
        height_);

    cudaError_t err = cudaGetLastError();
    if (err != cudaSuccess) {
        return err;
    }

    BoundingBoxForTrianglesKernel<<<
        dim_grid_bounding_box_,
        threads_per_block,
        0,
        stream>>>(
        dev_bounding_box_triangles_,
        dev_projected_triangles_snapped_,
        triangle_count_,
        width_,
        height_);

    err = cudaGetLastError();
    if (err != cudaSuccess) {
        return err;
    }

    BoundingBoxSizesKernel<<<
        dim_grid_triangles_,
        threads_per_block,
        0,
        stream>>>(
        dev_bounding_box_triangles_,
        dev_bounding_box_triangles_sizes_,
        triangle_count_,
        dev_bounding_box_,
        dev_backface_);

    err = cudaGetLastError();
    if (err != cudaSuccess) {
        return err;
    }

    err = cub::DeviceScan::ExclusiveSum(
        dev_cub_storage_,
        cub_storage_bytes_,
        dev_bounding_box_triangles_sizes_,
        dev_bounding_box_triangles_sizes_prefix_,
        triangle_count_,
        stream);

    if (err != cudaSuccess) {
        return err;
    }

    FillTriangleKernel_new<<<
        fill_triangle_grid_,
        threads_per_block,
        0,
        stream>>>(
        dev_bounding_box_triangles_sizes_,
        dev_bounding_box_triangles_sizes_prefix_,
        dev_bounding_box_triangles_,
        renderer_output_->GetDeviceImagePointer(),
        triangle_count_,
        width_,
        height_,
        dev_projected_triangles_);

    return cudaGetLastError();
}
```

The kernel arguments must remain identical to the current production call.

The current non-graph `Render()` may remain untouched during the first implementation.

**Bank-pointer infrastructure:** `RenderEngine` has `bank0_pointers_`, `CaptureBank0Pointers()`, `RestoreBank0Pointers()`, and `active_output_device_` / `active_bounding_box_host_` members from prior work. These are dead residue — delete them before the graph agent starts. Do not design around them.

---

# 8. Create a stream-aware metric enqueue path

The current metric performs a synchronous:

```cpp
cudaMemcpy(
    pixel_score_,
    dev_pixel_score_,
    sizeof(int),
    cudaMemcpyDeviceToHost);
```

That exact synchronous call **cannot be inside stream capture**. CUDA explicitly prohibits synchronous APIs such as `cudaMemcpy()` while capturing because they synchronize with the legacy stream.

The host result buffer is already pinned:

```cpp
cudaHostAlloc(
    (void**)&pixel_score_,
    sizeof(int),
    cudaHostAllocDefault);
```

Therefore add:

```cpp
cudaError_t GPUMetrics::EnqueueFastImplantDilationMetric(
    GPUImage* rendered_image,
    GPUDilatedFrame* comparison_frame,
    int dilation,
    cudaStream_t stream);
```

Its exact implementation should be:

```cpp
cudaError_t GPUMetrics::EnqueueFastImplantDilationMetric(
    GPUImage* rendered_image,
    GPUDilatedFrame* comparison_frame,
    int dilation,
    cudaStream_t stream) {

    const int height =
        rendered_image->GetFrameHeight();

    const int width =
        rendered_image->GetFrameWidth();

    unsigned char* image =
        rendered_image->GetDeviceImagePointer();

    const int* dev_bounding_box =
        rendered_image->GetDeviceBoundingBox();

    dim3 edge_block(16, 16);

    FastImplantDilationMetric_EdgeKernel_new<<<
        edge_grid_,
        edge_block,
        edge_threads * sizeof(unsigned char),
        stream>>>(
        image,
        dev_bounding_box,
        dev_pixel_score_,
        width,
        height,
        dilation);

    cudaError_t err = cudaGetLastError();
    if (err != cudaSuccess) {
        return err;
    }

    FastImplantDilationMetric_DilateKernel_new<<<
        dilate_grid_,
        threads_per_block,
        0,
        stream>>>(
        image,
        dev_bounding_box,
        width,
        height,
        dilation);

    err = cudaGetLastError();
    if (err != cudaSuccess) {
        return err;
    }

    FastImplantDilationMetric_DifferenceKernel_new<<<
        difference_grid_,
        threads_per_block,
        0,
        stream>>>(
        image,
        comparison_frame->GetDeviceImagePointer(),
        dev_pixel_score_,
        dev_bounding_box,
        width,
        height,
        dilation);

    err = cudaGetLastError();
    if (err != cudaSuccess) {
        return err;
    }

    return cudaMemcpyAsync(
        pixel_score_,
        dev_pixel_score_,
        sizeof(int),
        cudaMemcpyDeviceToHost,
        stream);
}
```

This is intentionally the exact current metric chain, merely moved to an explicit stream with an asynchronous pinned-memory tail copy.

**Stale U12 stream overload:** `gpu_metrics.cuh` declares `FastImplantDilationMetric(GPUImage*, GPUDilatedFrame*, int, cudaStream_t)` returning `double` (labeled "U12 enqueue/complete path") with no implementation. Delete this stale declaration before adding the clean enqueue API — different name means no C++ conflict, but the dangling declaration is cruft.

The Edge kernel already resets `dev_pixel_score_` itself, so do **not** add a reset kernel.

Also add:

```cpp
double GPUMetrics::ReadFastImplantDilationMetricResult() const {
    return -1.0 * pixel_score_[0];
}
```

This method must perform no CUDA call and no synchronization.

The graph owner will synchronize before calling it.

---

# 9. Use a dedicated non-default stream

Create exactly one stream for this graph:

```cpp
cudaStreamCreateWithFlags(
    &stream_,
    cudaStreamNonBlocking);
```

Use this stream for:

* graph capture;
* graph upload;
* graph replay;
* final synchronization.

Do not capture on stream 0 / `cudaStreamLegacy`.

CUDA documents that stream capture cannot begin on the legacy stream and also describes problems created by legacy-stream interaction during capture.

A non-blocking dedicated stream makes this much less likely to interact accidentally with unrelated legacy-stream work elsewhere in the application.

Use:

```cpp
cudaStreamCaptureModeThreadLocal
```

for this implementation.

Do not use Global mode unless there is a concrete reason.

The application has other infrastructure and possibly other threads; we only need to police unsafe CUDA calls made from the thread performing this capture.

---

# 10. Graph ownership: keep it tiny

Do not recreate a recipe framework.

Create one small class, for example:

```text
include/compute/direct_dilation_graph.cuh
src/compute/direct_dilation_graph.cu
```

with roughly:

```cpp
namespace gpu_cost_function {

class DirectDilationGraph {
public:
    DirectDilationGraph() = default;
    ~DirectDilationGraph();

    DirectDilationGraph(
        const DirectDilationGraph&) = delete;

    DirectDilationGraph& operator=(
        const DirectDilationGraph&) = delete;

    bool Build(
        RenderEngine* renderer,
        GPUMetrics* metrics,
        GPUDilatedFrame* comparison_frame,
        int dilation,
        const Pose& initial_pose);

    bool Matches(
        RenderEngine* renderer,
        GPUMetrics* metrics,
        GPUDilatedFrame* comparison_frame,
        int dilation) const;

    double Evaluate(
        const Pose& pose,
        cudaError_t* error);

    void Reset();

private:
    RenderEngine* renderer_ = nullptr;
    GPUMetrics* metrics_ = nullptr;
    GPUDilatedFrame* comparison_frame_ = nullptr;

    int dilation_ = 0;

    cudaStream_t stream_ = nullptr;
    cudaGraph_t graph_ = nullptr;
    cudaGraphExec_t exec_ = nullptr;
    cudaGraphNode_t world_to_pixel_node_ = nullptr;
};

}
```

This is enough.

No cache database.

No graph-key abstraction.

No bank state.

No capture coordinator.

No scheduler.

No multiple graph executions.

No asynchronous multi-pose executor.

---

# 11. Build the graph by stream capture

`Build()` should do this in order:

```text
Reset old graph
store renderer/metrics/comparison/dilation
create nonblocking stream
SetPose(initial_pose)
begin capture
EnqueueRender(stream)
EnqueueFastImplantDilationMetric(..., stream)
end capture
find WorldToPixel node
instantiate graph
upload graph
synchronize upload stream
```

The fundamental CUDA pattern is officially documented as:

```cpp
cudaStreamBeginCapture(stream);

kernel_A<<<..., stream>>>();
kernel_B<<<..., stream>>>();
libraryCall(stream);
kernel_C<<<..., stream>>>();

cudaStreamEndCapture(stream, &graph);

cudaGraphInstantiate(...);

cudaGraphLaunch(graphExec, stream);
```

The key point for JTML is that:

```cpp
cub::DeviceScan::ExclusiveSum(..., stream)
```

is the `libraryCall(stream)` in NVIDIA's example.

---

# 12. Suggested `Build()` structure

The implementation should be structurally equivalent to:

```cpp
bool DirectDilationGraph::Build(
    RenderEngine* renderer,
    GPUMetrics* metrics,
    GPUDilatedFrame* comparison_frame,
    int dilation,
    const Pose& initial_pose) {

    Reset();

    if (renderer == nullptr ||
        metrics == nullptr ||
        comparison_frame == nullptr) {
        return false;
    }

    renderer_ = renderer;
    metrics_ = metrics;
    comparison_frame_ = comparison_frame;
    dilation_ = dilation;

    cudaError_t err =
        cudaStreamCreateWithFlags(
            &stream_,
            cudaStreamNonBlocking);

    if (err != cudaSuccess) {
        Reset();
        return false;
    }

    renderer_->SetPose(initial_pose);

    err = cudaStreamBeginCapture(
        stream_,
        cudaStreamCaptureModeThreadLocal);

    if (err != cudaSuccess) {
        Reset();
        return false;
    }

    cudaError_t enqueue_error =
        renderer_->EnqueueRender(stream_);

    if (enqueue_error == cudaSuccess) {
        enqueue_error =
            metrics_->EnqueueFastImplantDilationMetric(
                renderer_->GetRenderOutput(),
                comparison_frame_,
                dilation_,
                stream_);
    }

    cudaGraph_t captured_graph = nullptr;

    const cudaError_t end_error =
        cudaStreamEndCapture(
            stream_,
            &captured_graph);

    if (enqueue_error != cudaSuccess ||
        end_error != cudaSuccess ||
        captured_graph == nullptr) {

        if (captured_graph != nullptr) {
            cudaGraphDestroy(captured_graph);
        }

        Reset();
        return false;
    }

    graph_ = captured_graph;

    if (!renderer_->FindWorldToPixelGraphNode(
            graph_,
            &world_to_pixel_node_)) {

        Reset();
        return false;
    }

    err = cudaGraphInstantiate(
        &exec_,
        graph_,
        nullptr,
        nullptr,
        0);

    if (err != cudaSuccess) {
        Reset();
        return false;
    }

    err = cudaGraphUpload(
        exec_,
        stream_);

    if (err != cudaSuccess) {
        Reset();
        return false;
    }

    err = cudaStreamSynchronize(stream_);

    if (err != cudaSuccess) {
        Reset();
        return false;
    }

    return true;
}
```

`cudaGraphUpload()` explicitly uploads an executable graph without running it, allowing the first-launch upload cost to be paid outside the optimizer's timed evaluation loop.

---

# 13. Locate the captured `WorldToPixelKernel` node

After stream capture, enumerate graph nodes.

Add to `RenderEngine`:

```cpp
bool FindWorldToPixelGraphNode(
    cudaGraph_t graph,
    cudaGraphNode_t* out_node) const;
```

Implementation:

```cpp
bool RenderEngine::FindWorldToPixelGraphNode(
    cudaGraph_t graph,
    cudaGraphNode_t* out_node) const {

    if (graph == nullptr ||
        out_node == nullptr) {
        return false;
    }

    *out_node = nullptr;

    size_t node_count = 0;

    if (cudaGraphGetNodes(
            graph,
            nullptr,
            &node_count) != cudaSuccess) {
        return false;
    }

    std::vector<cudaGraphNode_t> nodes(
        node_count);

    if (cudaGraphGetNodes(
            graph,
            nodes.data(),
            &node_count) != cudaSuccess) {
        return false;
    }

    for (cudaGraphNode_t node : nodes) {
        cudaGraphNodeType type{};

        if (cudaGraphNodeGetType(
                node,
                &type) != cudaSuccess) {
            return false;
        }

        if (type != cudaGraphNodeTypeKernel) {
            continue;
        }

        cudaKernelNodeParams params{};

        if (cudaGraphKernelNodeGetParams(
                node,
                &params) != cudaSuccess) {
            return false;
        }

        if (params.func ==
            reinterpret_cast<void*>(
                WorldToPixelKernel)) {

            *out_node = node;
            return true;
        }
    }

    return false;
}
```

Do not identify this node by graph order.

Do not assume it is node zero.

Match the actual kernel function pointer.

---

# 14. Per-evaluation pose update: exact `WorldToPixelKernel` arguments

Current `WorldToPixelKernel` has these arguments, in this exact order:

```text
0  float*          dev_triangles
1  float*          dev_projected_triangles
2  int*            dev_projected_triangles_snapped
3  int             vertex_count
4  float           dist_over_pix_pitch
5  float           pix_conversion_x
6  float           pix_conversion_y
7  float           x_location
8  float           y_location
9  float           z_location
10 RotationMatrix  model_rotation_mat
11 float*          dev_normals
12 bool*           dev_backface
13 bool            use_backface_culling
14 float           fx
15 float           fy
16 float           cx
17 float           cy
18 unsigned char*  dev_image
19 int*            dev_bounding_box
20 int             image_width
21 int             image_height
```

Only four kernel arguments vary from pose to pose:

```text
7   x_location
8   y_location
9   z_location
10  model_rotation_mat
```

The six Euler pose parameters are **not** all sent directly to CUDA.

The three rotation angles are converted by `SetPose()` into the 3×3 `RotationMatrix`.

That distinction must be preserved.

---

# 15. Update `WorldToPixelKernel` without changing the kernel

Add:

```cpp
cudaError_t UpdateWorldToPixelGraphNode(
    cudaGraphExec_t exec,
    cudaGraphNode_t node);
```

to `RenderEngine`.

Implementation should construct the exact current kernel launch parameters:

```cpp
cudaError_t RenderEngine::UpdateWorldToPixelGraphNode(
    cudaGraphExec_t exec,
    cudaGraphNode_t node) {

    if (exec == nullptr ||
        node == nullptr) {
        return cudaErrorInvalidValue;
    }

    int vertex_count =
        3 * triangle_count_;

    unsigned char* image =
        renderer_output_
            ->GetDeviceImagePointer();

    void* kernel_args[] = {
        &dev_triangles_,
        &dev_projected_triangles_,
        &dev_projected_triangles_snapped_,
        &vertex_count,
        &dist_over_pix_pitch_,
        &pix_conversion_x_,
        &pix_conversion_y_,
        &model_pose_.x_location_,
        &model_pose_.y_location_,
        &model_pose_.z_location_,
        &model_rotation_mat_,
        &dev_normals_,
        &dev_backface_,
        &use_backface_culling_,
        &fx_,
        &fy_,
        &cx_,
        &cy_,
        &image,
        &dev_bounding_box_,
        &width_,
        &height_
    };

    cudaKernelNodeParams params{};

    params.func =
        reinterpret_cast<void*>(
            WorldToPixelKernel);

    params.gridDim =
        dim_grid_vertices_;

    params.blockDim =
        dim3(
            threads_per_block,
            1,
            1);

    params.sharedMemBytes = 0;
    params.kernelParams = kernel_args;
    params.extra = nullptr;

    return cudaGraphExecKernelNodeSetParams(
        exec,
        node,
        &params);
}
```

NVIDIA documents that `cudaGraphExecKernelNodeSetParams()` modifies future launches of the instantiated executable graph without requiring recapture.

This is preferable for the first implementation to modifying `WorldToPixelKernel` to read a pose buffer from global memory.

We want to preserve the measured ~2 µs kernel exactly.

---

# 16. Graph evaluation

`DirectDilationGraph::Evaluate()` should be extremely small:

```cpp
double DirectDilationGraph::Evaluate(
    const Pose& pose,
    cudaError_t* error) {

    if (error != nullptr) {
        *error = cudaSuccess;
    }

    if (renderer_ == nullptr ||
        metrics_ == nullptr ||
        exec_ == nullptr ||
        stream_ == nullptr ||
        world_to_pixel_node_ == nullptr) {

        if (error != nullptr) {
            *error = cudaErrorInvalidResourceHandle;
        }

        return 0.0;
    }

    renderer_->SetPose(pose);

    cudaError_t err =
        renderer_->UpdateWorldToPixelGraphNode(
            exec_,
            world_to_pixel_node_);

    if (err != cudaSuccess) {
        if (error != nullptr) {
            *error = err;
        }

        return 0.0;
    }

    err = cudaGraphLaunch(
        exec_,
        stream_);

    if (err != cudaSuccess) {
        if (error != nullptr) {
            *error = err;
        }

        return 0.0;
    }

    err = cudaStreamSynchronize(
        stream_);

    if (err != cudaSuccess) {
        if (error != nullptr) {
            *error = err;
        }

        return 0.0;
    }

    return metrics_
        ->ReadFastImplantDilationMetricResult();
}
```

`cudaGraphLaunch()` executes the instantiated graph in the specified stream. CUDA guarantees graph launches are ordered with previous work in that stream and with previous launches of the same executable graph.

The final:

```cpp
cudaStreamSynchronize(stream_)
```

is currently unavoidable for scalar DIRECT because the optimizer needs the scalar result before choosing the next pose.

The objective is not to eliminate that logical dependency.

The objective is to reduce everything before that dependency from 7 application kernel launches + CUB dispatch + D2H copy to one graph submission.

---

# 17. Graph validity / rebuild conditions

The graph may be reused while these remain unchanged:

```text
RenderEngine object
GPUMetrics object
comparison-frame device pointer
dilation
render dimensions
triangle/model allocations
camera calibration
backface-culling configuration
launch dimensions
```

The pose may change every evaluation because its `WorldToPixel` node parameters are updated.

For the initial implementation, rebuild when either:

```cpp
comparison_frame != comparison_frame_
```

or:

```cpp
dilation != dilation_
```

or the renderer/metrics pointer changes.

Therefore:

```cpp
bool DirectDilationGraph::Matches(
    RenderEngine* renderer,
    GPUMetrics* metrics,
    GPUDilatedFrame* comparison_frame,
    int dilation) const {

    return exec_ != nullptr &&
        renderer == renderer_ &&
        metrics == metrics_ &&
        comparison_frame ==
            comparison_frame_ &&
        dilation == dilation_;
}
```

Do **not** try to update the Edge, Dilate, Difference, or memcpy nodes in version 1.

If the frame or dilation changes, graph reconstruction is fine.

Dilation is loaded once at stage initialization (`initializeDIRECT_DILATION` sets `DIRECT_DILATION_current_dilation_parameter`) and reused throughout all evaluations in that stage. Graph rebuilds amortize over thousands of pose evaluations.

---

# 18. Integrate only into monoplane `costFunctionDIRECT_DILATION()`

The current scalar code must remain available.

The graph path should conceptually become:

```cpp
double CostFunctionManager::
costFunctionDIRECT_DILATION() {

    const gpu_cost_function::Pose pose =
        gpu_principal_model_
            ->GetCurrentPrimaryCameraPose();

    if (!biplane_mode_) {
        auto* renderer =
            gpu_principal_model_
                ->GetPrimaryRenderEngine();

        auto* comparison_frame =
            gpu_dilated_frames_A_
                ->at(current_frame_index_);

        if (!direct_dilation_graph_) {
            direct_dilation_graph_ =
                std::make_unique<
                    gpu_cost_function::
                        DirectDilationGraph>();
        }

        if (!direct_dilation_graph_->Matches(
                renderer,
                gpu_metrics_,
                comparison_frame,
                DIRECT_DILATION_current_dilation_parameter)) {

            const bool built =
                direct_dilation_graph_->Build(
                    renderer,
                    gpu_metrics_,
                    comparison_frame,
                    DIRECT_DILATION_current_dilation_parameter,
                    pose);

            if (!built) {
                direct_dilation_graph_->Reset();
            }
        }

        if (direct_dilation_graph_->Matches(
                renderer,
                gpu_metrics_,
                comparison_frame,
                DIRECT_DILATION_current_dilation_parameter)) {

            cudaError_t graph_error =
                cudaSuccess;

            const double fidm =
                direct_dilation_graph_->Evaluate(
                    pose,
                    &graph_error);

            if (graph_error ==
                cudaSuccess) {

                return
                    DIRECT_DILATION_current_white_pix_sum_dilated_comparison_image_A_ +
                    fidm;
            }
        }
    }

    /*
     * Existing scalar path.
     * Do not remove.
     */
    gpu_principal_model_->RenderPrimaryCamera(
        gpu_principal_model_
            ->GetCurrentPrimaryCameraPose());

    double metric_score =
        DIRECT_DILATION_current_white_pix_sum_dilated_comparison_image_A_ +
        gpu_metrics_->FastImplantDilationMetric(
            gpu_principal_model_
                ->GetPrimaryCameraRenderedImage(),
            gpu_dilated_frames_A_
                ->at(current_frame_index_),
            DIRECT_DILATION_current_dilation_parameter);

    if (biplane_mode_) {
        gpu_principal_model_
            ->RenderSecondaryCamera(
                gpu_principal_model_
                    ->GetCurrentSecondaryCameraPose());

        double dist_score =
            DIRECT_DILATION_current_white_pix_sum_dilated_comparison_image_B_ +
            gpu_metrics_->FastImplantDilationMetric(
                gpu_principal_model_
                    ->GetSecondaryCameraRenderedImage(),
                gpu_dilated_frames_B_
                    ->at(current_frame_index_),
                DIRECT_DILATION_current_dilation_parameter);

        metric_score +=
            dist_score * dist_score;
    }

    return metric_score;
}
```

The bottom fallback should remain functionally identical to the current implementation.

If graph creation or launch fails, the optimization should continue through the scalar path rather than crash.

**CostFunctionManager header changes:** The plan shows `costFunctionDIRECT_DILATION()` using `direct_dilation_graph_` as a member but never specifies the corresponding header changes. The implementer needs to:

1. Add a forward declaration or include for `DirectDilationGraph`
2. Add `std::unique_ptr<gpu_cost_function::DirectDilationGraph> direct_dilation_graph_` as a private member of `CostFunctionManager`

Also: before the graph agent starts, **remove orphaned old-graph declarations** that have zero definitions/usages: `GraphRecipeCaptureInputs` forward-decl in `CostFunctionManager.h`, `EvaluationContext` forward-decl, `GetGraphRecipeCaptureInputs()` declaration.

---

# 19. Graph cleanup

Implement:

```cpp
void DirectDilationGraph::Reset() {
    if (exec_ != nullptr) {
        cudaGraphExecDestroy(exec_);
        exec_ = nullptr;
    }

    if (graph_ != nullptr) {
        cudaGraphDestroy(graph_);
        graph_ = nullptr;
    }

    if (stream_ != nullptr) {
        cudaStreamDestroy(stream_);
        stream_ = nullptr;
    }

    world_to_pixel_node_ = nullptr;

    renderer_ = nullptr;
    metrics_ = nullptr;
    comparison_frame_ = nullptr;
    dilation_ = 0;
}
```

Destructor:

```cpp
DirectDilationGraph::~DirectDilationGraph() {
    Reset();
}
```

Graph objects are not thread-safe, so version 1 should remain strictly scalar / single-owner.

---

# 20. CMake

Add:

```text
direct_dilation_graph.cu
```

to:

```cmake
add_library(jtml_compute SHARED ...)
```

in:

```text
src/compute/CMakeLists.txt
```

Do not create another CUDA library.

Do not alter linker topology.

The existing `jtml_compute` target already links `CUDA::cudart` and uses CUDA separable compilation.

---

# 21. Correctness validation — mandatory before benchmarking

Do not trust graph output merely because it looks plausible.

Add a temporary validation mode that evaluates the **same current pose twice**:

```text
scalar path
graph path
```

This does not require a synthetic pose harness.

The current pose already exists at the exact point `costFunctionDIRECT_DILATION()` is called.

For perhaps the first 100–1000 real optimizer queries:

```cpp
const Pose pose =
    gpu_principal_model_
        ->GetCurrentPrimaryCameraPose();

const double scalar_score =
    EvaluateUsingExistingScalarPath(pose);

const double graph_score =
    EvaluateUsingGraphPath(pose);

if (scalar_score != graph_score) {
    std::cerr
        << "CUDA graph mismatch: "
        << "scalar=" << scalar_score
        << " graph=" << graph_score
        << " pose=("
        << pose.x_location_ << ", "
        << pose.y_location_ << ", "
        << pose.z_location_ << ", "
        << pose.x_angle_ << ", "
        << pose.y_angle_ << ", "
        << pose.z_angle_ << ")\n";
}
```

Exact equality is appropriate here because:

* the same integer image/score algorithm is being run;
* no floating-point reduction order is being changed;
* CUB is scanning integers;
* launch geometry is unchanged;
* only submission mechanism changes;
* **CRITICAL INVARIANT:** `WorldToPixelKernel` clears the render buffer and resets the bounding box at the start of every evaluation (verified in §4). If any future code removes this clear, exact equality may silently fail — the validation should verify this invariant explicitly.

The rendered image from the first run does not contaminate the second because `WorldToPixelKernel` clears the render buffer at the beginning of every evaluation.

After proving equality, disable dual execution.

---

# 22. Performance validation — three-arm experiment

Produce three measurements from the exact same codebase:

```text
A — SCALAR
existing known-good path

B — GRAPH/FULL (the implementation we actually want)
SetPose
SetParams(WorldToPixel)
GraphLaunch
  [render + CUB + metric + D2H]
StreamSynchronize

C — GRAPH/NO-D2H-NODE (diagnostic only)
SetPose
SetParams(WorldToPixel)
GraphLaunch
  [render + CUB + metric]
cudaMemcpyAsync(score)
StreamSynchronize
```

`B` is the implementation we ship.

`C` answers the memcpy-node concern empirically: if B and C are basically identical in timing, the D2H node in the graph is not a measurable cost. If C launches noticeably cheaper but loses that gain by requiring external copy submission, we know exactly where the tradeoff is.

Record separately (outside Nsight, using a large sample):

```text
CPU SetPose time
cudaGraphExecKernelNodeSetParams time
cudaGraphLaunch time
full evaluation wall time
```

Use the normal DIRECT optimizer throughput — not Nsight throughput, because profiling launch overhead is very large relative to these tiny kernels.

Current non-profiled target is approximately:

```text
~26k evaluations/sec
~38 us/evaluation (approximate — do not treat as exact)
```

**Important caveat — constant-time launch claim may not apply.** NVIDIA's "2.5 us + ~1 ns per node" figure from the Ampere blog applies to straight-line CUDA graphs with kernel nodes only. NVIDIA explicitly states "graphs with non-kernel nodes would have different performance characteristics." This graph contains a D2H memcpy node, so the actual graph launch overhead could be meaningfully higher. That 2.5 us number is evidence that Ampere graph dispatch *can* be cheap — not a prediction for JTML.

If `SetParams` emerges as a significant fraction of the total graph advantage, implement:

```text
D — immutable graph + pose H2D node
```

That turns the SetParams concern into a clean experimental fork.

---

# 23. What constitutes success

Do not require a giant improvement.

This workload is already heavily optimized.

Compare graph vs scalar in the same build/run and report **percentage + absolute delta**:

```text
graph_time vs scalar_time
improvement: X us (Y%)
```

**These are aspirational targets, not calibrated predictions.** The actual improvement depends on measured SetParams + GraphLaunch overhead and the graph launch cost for non-kernel-node graphs. Measure first, then assess.

The reason this is still attractive is that it replaces approximately 7 application kernel submissions + CUB dispatch + D2H copy with one graph submission.

NVIDIA reports that on Ampere, repeat launches of straight-line CUDA graphs approach roughly **2.5 us + ~1 ns per node** for kernel-only graphs. Their benchmark is not identical to JTML — see caveat above.

This workload is nevertheless almost exactly the shape CUDA Graphs are intended for: a repeatedly executed straight-line chain of very small dependent GPU operations.

---

# 24. Important performance caveat: pose node update

Every evaluation will call:

```cpp
cudaGraphExecKernelNodeSetParams(...)
```

before:

```cpp
cudaGraphLaunch(...)
```

That has some host overhead and may cause some graph update/upload bookkeeping.

Do not prematurely optimize it away.

Version 1 should measure:

```text
SetParams + GraphLaunch + Sync
```

with the existing `WorldToPixelKernel` completely unchanged.

If graphs are promising but the node update is measurably expensive, version 2 can investigate a pinned-host/device pose parameter buffer captured as an H2D memcpy node.

Do **not** begin there because that would modify a currently ~2 µs kernel to load pose state from device memory, contaminating the graph experiment with a kernel redesign.

We want to preserve the measured ~2 us kernel exactly.

---

# 24. Important performance caveat: pose node update

Every evaluation will call:

```cpp
cudaGraphExecKernelNodeSetParams(...)
```

before:

```cpp
cudaGraphLaunch(...)
```

That has some host overhead and may cause some graph update/upload bookkeeping.

**This is the single biggest unknown in the experiment.** NVIDIA describes individual node updates as lightweight, but provides no number. The call reconstructs all 22 kernel arguments, not just the 4 that change. Explicitly measure CPU duration of:

```cpp
auto t0 = std::chrono::high_resolution_clock::now();
cudaGraphExecKernelNodeSetParams(...);
auto t1 = std::chrono::high_resolution_clock::now();
```

over a large sample, separately from the full graph evaluation timing.

Do not prematurely optimize it away.

Version 1 should measure:

```text
SetParams + GraphLaunch + Sync
```

with the existing `WorldToPixelKernel` completely unchanged.

**Plan B (named alternative, not discovered later):** If SetParams costs too much, switch from:

```text
CPU pose -> SetPose() -> cudaGraphExecKernelNodeSetParams() -> graph
```

to:

```text
CPU pose -> tiny pinned host pose buffer -> captured H2D memcpy -> device pose struct -> WorldToPixel reads pose struct
```

The graph becomes completely immutable between evaluations:

```text
Pose H2D -> WorldToPixel -> BBox -> BBoxSizes -> CUB -> Fill -> Edge -> Dilate -> Difference -> Score D2H
```

and every evaluation is literally:

```cpp
host_pose_ = pose;
cudaGraphLaunch(exec_, stream_);
cudaStreamSynchronize(stream_);
return pixel_score_[0];
```

No graph update API whatsoever.

Still do not start there — it changes `WorldToPixelKernel` and kernel arguments have nice CUDA parameter-memory behavior. First determine whether SetParams actually costs enough to care about.

NVIDIA's own dynamic-parameter guidance recommends individual executable-node updates as a low-cost way to reuse a captured topology, but their graph-performance discussion notes that graph-update activity can cause part of the normally one-time upload/launch work to be repaid. The empirical measurement decides.

---

# 25. Things the agent must explicitly NOT do

Do not:

```text
replace CUB
rewrite Fill
change Fill grid
change Edge grid
change Dilate grid
change Difference grid
change threads_per_block
add DistanceMapMetric
add another score reduction
add mapped host-memory atomics
put pixel_score_ in unified memory
change pose Euler math
change RotationMatrix convention
change WorldToPixel math
change bbox math
change backface behavior
fuse any kernels
add batching
add multiple streams for concurrency
capture on stream 0
use synchronous cudaMemcpy inside capture
reintroduce old GraphRecipe/Bank/EvaluationContext architecture
implement a generic graph framework
```

The first PR should be boring.

That is a feature.

---

### Consolidated prohibitions (complete list from all sections)

The following prohibitions from other sections should be honored alongside §25:

* §6: Do not manually reconstruct CUB's graph nodes.
* §9: Do not use Global capture mode unless there is a concrete reason.
* §13: Do not identify the WorldToPixel node by graph order — match the actual kernel function pointer.
* §20: Do not create another CUDA library.
* §20: Do not alter linker topology.
* §24: Do not prematurely optimize the SetParams cost away.

---

# 26. Expected graph-static versus graph-dynamic state

## Changes every pose evaluation

Only:

```text
Pose:
    x_location_
    y_location_
    z_location_
    x_angle_
    y_angle_
    z_angle_

Derived host value:
    model_rotation_mat_
```

The actual CUDA node receives only:

```text
x_location_
y_location_
z_location_
model_rotation_mat_
```

as changing kernel arguments.

## Fixed for one graph

```text
dev_triangles_
dev_projected_triangles_
dev_projected_triangles_snapped_
dev_normals_
dev_backface_
renderer output device pointer
dev_bounding_box_

triangle_count_
width_
height_

dist_over_pix_pitch_
pix_conversion_x_
pix_conversion_y_

fx_
fy_
cx_
cy_

use_backface_culling_

dev_bounding_box_triangles_
dev_bounding_box_triangles_sizes_
dev_bounding_box_triangles_sizes_prefix_

dev_cub_storage_
cub_storage_bytes_

fill_triangle_grid_
edge_grid_
dilate_grid_
difference_grid_

comparison frame device pointer
dilation

dev_pixel_score_
pixel_score_ pinned host pointer
```

The comparison frame pointer and dilation are static **for the lifetime of one instantiated graph**, but graph reconstruction is required when either changes.

---

# 27. Why the current code is now a good graph candidate

Several things that previously made graphing this ugly have already been removed.

The current code has:

* device-resident global bbox;
* image clear inside `WorldToPixelKernel`;
* bbox reset inside `WorldToPixelKernel`;
* no fragment-count D2H;
* no host-derived Fill launch size;
* fixed Fill worker grid;
* fixed Edge/Dilate/Difference worker grids;
* CUB temp storage allocated ahead of time;
* only one required D2H result;
* pinned host storage for that result.

The current output image also exposes the renderer-owned device bbox directly via `GPUImage::GetDeviceBoundingBox()`.

That means there is now no host conversation anywhere between:

```text
pose projection
```

and:

```text
final score
```

which is exactly the architecture needed for a clean graph replay.

---

# 28. Official documentation

Use these, not random examples from Stack Overflow.

CUDA Programming Guide — CUDA Graphs:

[CUDA Programming Guide: CUDA Graphs](https://docs.nvidia.com/cuda/cuda-programming-guide/04-special-topics/cuda-graphs.html?utm_source=chatgpt.com)

CUDA Runtime API — Stream Management (`cudaStreamBeginCapture`, `cudaStreamEndCapture`):

[CUDA Runtime API: Stream Management](https://docs.nvidia.com/cuda/cuda-runtime-api/group__CUDART__STREAM.html?utm_source=chatgpt.com)

CUDA Runtime API — Graph Management (`cudaGraphInstantiate`, `cudaGraphExecKernelNodeSetParams`, `cudaGraphLaunch`, `cudaGraphUpload`, destruction APIs):

[CUDA Runtime API: Graph Management](https://docs.nvidia.com/cuda/cuda-runtime-api/group__CUDART__GRAPH.html?utm_source=chatgpt.com)

NVIDIA technical blog — dynamic graph parameters:

[Constructing CUDA Graphs with Dynamic Parameters](https://developer.nvidia.com/blog/constructing-cuda-graphs-with-dynamic-parameters/?utm_source=chatgpt.com)

NVIDIA technical blog — recent straight-line graph launch performance:

[Constant Time Launch for Straight-Line CUDA Graphs](https://developer.nvidia.com/blog/constant-time-launch-for-straight-line-cuda-graphs-and-other-performance-enhancements/?utm_source=chatgpt.com)

CUB `DeviceScan::ExclusiveSum` API, including the explicit `cudaStream_t` parameter:

[CCCL CUB DeviceScan documentation](https://nvidia.github.io/cccl/unstable/cub/api/structcub_1_1DeviceScan.html?utm_source=chatgpt.com)

---

# 29. Recommended implementation order

The agent should make this in very small stages.

**Preliminary cleanup (before graph work starts):**

0. Remove orphaned old-graph declarations (`GraphRecipeCaptureInputs`, `EvaluationContext` forward-decl, `GetGraphRecipeCaptureInputs()`) from `CostFunctionManager.h` — zero definitions/usages, dead cruft.
0. Delete stale bank-pointer infrastructure from `RenderEngine` (`bank0_pointers_`, `CaptureBank0Pointers()`, `RestoreBank0Pointers()`, `active_output_device_`, `active_bounding_box_host_`, `bank0_pointers_captured_`).
0. Delete stale U12 `FastImplantDilationMetric(..., cudaStream_t)` declaration from `gpu_metrics.cuh` (no implementation exists).

**Implementation:**

1. Add `RenderEngine::EnqueueRender(cudaStream_t)` and verify it compiles.
2. Add `GPUMetrics::EnqueueFastImplantDilationMetric(..., cudaStream_t)` and `ReadFastImplantDilationMetricResult()`.
3. Test those explicit-stream methods *without graphs* once to prove they produce the same score.
4. Add tiny `DirectDilationGraph`.
5. Capture render + metric + D2H.
6. Find `WorldToPixelKernel` node.
7. Instantiate and upload.
8. Add `WorldToPixel` parameter update.
9. Run scalar-vs-graph same-pose validation (§21).
10. Only after exact agreement, run the three-arm benchmark (§22: A=scalar, B=graph/full, C=graph/no-D2H-node).
11. Separately measure CPU SetParams cost over a large sample.
12. Only after graph performance is known should any cleanup/refactoring happen.

If anything fails, stop changing architecture and report the exact CUDA error and the exact API call that failed.

---

# 30. Definition of done

The first graph implementation is done when all of these are true:

```text
[ ] Existing scalar DIRECT_DILATION still works.
[ ] Graph path is monoplane-only.
[ ] No DistanceMapMetric in graph.
[ ] No host synchronization occurs inside capture.
[ ] CUB is captured using its explicit stream parameter.
[ ] Graph includes final async D2H score copy.
[ ] pixel_score_ remains existing pinned host memory.
[ ] Pose is updated with RenderEngine::SetPose().
[ ] Only WorldToPixel graph-node parameters are updated each evaluation.
[ ] Graph rebuilds if comparison frame or dilation changes.
[ ] Scalar and graph scores match exactly over many real optimizer poses.
[ ] Graph can fail back to scalar without killing optimization.
[ ] Nsight shows one graph launch per scalar cost evaluation.
[ ] Three-arm benchmark complete (A=scalar, B=graph/full, C=graph/no-D2H-node).
[ ] CPU SetParams cost measured and reported.
[ ] WorldToPixel image-clear + bbox-reset invariant verified in validation.
[ ] Old orphaned graph declarations removed from headers.
[ ] Dead bank-pointer infrastructure removed from RenderEngine.
```

That is the entire job.

Do not solve anything else in the same change.

