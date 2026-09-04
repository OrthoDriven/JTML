#include "objectives/direct_dilation.hpp"

bool DirectDilationObjective::initialize(std::string& error_message) {
    cudaError cudaStatus = cudaSuccess;

    white_pixel_sum_A_ = metrics_->ComputeSumWhitePixels(
        comparison_A_->GetGPUImage(), &cudaStatus);

    if (comparison_B_ != nullptr) {
        white_pixel_sum_B_ = metrics_->ComputeSumWhitePixels(
            comparison_B_->GetGPUImage(), &cudaStatus);
    };

    error_message = cudaGetErrorString(cudaStatus);

    return (cudaStatus == cudaSuccess);
}

double DirectDilationObjective::evaluate(const gpu_cost_function::Pose& pose) {
    /*Render*/
    principal_model_->RenderPrimaryCamera(pose);

    double metric_score =
        (white_pixel_sum_A_ +
         metrics_->FastImplantDilationMetric(
             principal_model_->GetPrimaryCameraRenderedImage(),
             comparison_A_,
             spec_.dilation));

    if (comparison_B_ != nullptr) {};

    return metric_score;
}
