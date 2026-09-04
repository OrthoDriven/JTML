#include <cuda.h>
#include <cuda_runtime.h>

#include "compute/gpu_metrics.cuh"

/*Grayscale Colors*/
#include "compute/pixel_grayscale_colors.h"

/*Launch Parameters*/
#include "cuda_launch_parameters.h"

__global__ void CurvatureHausdorfMetric_Kernel(
    unsigned char* projected_image,
    unsigned char* heatmaps,
    int* hausdorf_score,
    int width,
    int height,
    int left_x,
    int bottom_y,
    int diff_cropped_width) {
    // Have to do some fun math here to get the cropped
    // with to match up with the original image locations
    int thread_x = threadIdx.x + (blockIdx.x * blockDim.x);
    int thread_y = threadIdx.y + (blockIdx.y * blockDim.y);

    int orig_x = thread_x + left_x;
    int orig_y = thread_y + bottom_y;

    int orig_idx = orig_x + orig_y * width;

    // Grabbing keypoint locations by adding full image pixels to move
    // to the next keypoint value
    int kp_loc = orig_idx + blockDim.z * height * width;
};
__global__ void Reset_CurvatureHausdorfScore_Kernel(int* dev_curv_haus_score) {
    int i = threadIdx.x;
    dev_curv_haus_score[i] = 0;
};  // namespace gpu_cost_function
