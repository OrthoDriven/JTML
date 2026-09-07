#ifndef DIRECT_DILATION_H_
#define DIRECT_DILATION_H_

#include "compute/gpu_dilated_frame.cuh"
#include "compute/gpu_edge_frame.cuh"
#include "compute/gpu_frame.cuh"
#include "compute/gpu_heatmaps.cuh"
#include "compute/gpu_image.cuh"
#include "compute/gpu_metrics.cuh"
#include "compute/gpu_model.cuh"
#include "compute/objective_instance.hpp"
#include "compute/objective_spec.h"
#include "compute/render_engine.cuh"

class DirectDilationObjective final : public ObjectiveInstance {
public:
    DirectDilationObjective(const DirectDilationObjective&) = default;
    DirectDilationObjective(DirectDilationObjective&&) = default;
    DirectDilationObjective& operator=(const DirectDilationObjective&) = delete;
    DirectDilationObjective& operator=(DirectDilationObjective&&) = delete;

    DirectDilationObjective(
        const jta_cost_function::DirectDilationSpec& spec,
        gpu_cost_function::GPUModel* principal_model,
        gpu_cost_function::GPUDilatedFrame* comparison_A,
        gpu_cost_function::GPUMetrics* metrics,
        gpu_cost_function::GPUDilatedFrame* comparison_B = nullptr) :
        spec_(spec),
        principal_model_(principal_model),
        comparison_A_(comparison_A),
        comparison_B_(comparison_B),
        metrics_(metrics) {}

    bool initialize(std::string& error_message) override;
    double evaluate(const gpu_cost_function::Pose& pose) override;

private:
    jta_cost_function::DirectDilationSpec spec_;

    gpu_cost_function::GPUModel* principal_model_;
    gpu_cost_function::GPUDilatedFrame* comparison_A_;
    gpu_cost_function::GPUDilatedFrame* comparison_B_;
    gpu_cost_function::GPUMetrics* metrics_;

    int white_pixel_sum_A_{};
    int white_pixel_sum_B_{};
};

#endif  // DIRECT_DILATION_H_
