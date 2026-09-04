// Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
// SPDX-License-Identifier: AGPL-3.0-only OR MIT

/*DIRECT_DILATION Source*/
#include <cmath>
#include <fstream>
#include <iostream>
#include <string>

#include "CostFunctionManager.h"
#include "DIRECT_DILATIONCustomVariables.h"
#include "compute/objective_instance.hpp"

namespace jta_cost_function {
bool CostFunctionManager::initializeDIRECT_DILATION(
    std::string& error_message) {
    cudaError cudaStatus;

    /*Compute the sum of the white pixels in the comparison dilated frame*/
    DIRECT_DILATION_current_white_pix_sum_dilated_comparison_image_A_ =
        gpu_metrics_->ComputeSumWhitePixels(
            (*gpu_dilated_frames_A_)[current_frame_index_]->GetGPUImage(),
            &cudaStatus);
    /*If Biplane Mode*/
    if (biplane_mode_) {
        DIRECT_DILATION_current_white_pix_sum_dilated_comparison_image_B_ =
            gpu_metrics_->ComputeSumWhitePixels(
                (*gpu_dilated_frames_B_)[current_frame_index_]->GetGPUImage(),
                &cudaStatus);
    }
    error_message = cudaGetErrorString(cudaStatus);

    /*Store Current Dilation Value*/
    this->getActiveCostFunctionClass()->getIntParameterValue(
        "Dilation", DIRECT_DILATION_current_dilation_parameter);

    // gpu_metrics_->AllocateCurvatureHausdorfScore(
    //     gpu_heatmaps_->at(current_frame_index_)->GetNumKeypoints());

    /*Return if success or not*/
    return (cudaStatus == cudaSuccess);
}
bool CostFunctionManager::destructDIRECT_DILATION(std::string& error_message) {
    return true;
}
double CostFunctionManager::costFunctionDIRECT_DILATION() {
    /*Render*/
    gpu_principal_model_->RenderPrimaryCamera(
        gpu_principal_model_->GetCurrentPrimaryCameraPose());

    /*Dilate rendered image to 1 dilation if in trunk mode*/
    double metric_score;

    /*(DIFFERENT FROM JTA PAPER) Dilate rendered image to same dilation as
     * comparison image*/
    metric_score =
        (DIRECT_DILATION_current_white_pix_sum_dilated_comparison_image_A_ +
         gpu_metrics_->FastImplantDilationMetric(
             gpu_principal_model_->GetPrimaryCameraRenderedImage(),
             gpu_dilated_frames_A_->at(current_frame_index_),
             DIRECT_DILATION_current_dilation_parameter));

    /*Biplane Mode Only*/
    if (biplane_mode_) {
        /*Render*/
        gpu_principal_model_->RenderSecondaryCamera(
            gpu_principal_model_->GetCurrentSecondaryCameraPose());

        /*(DIFFERENT FROM JTA PAPER) Dilate rendered image to same dilation as
         * comparison image*/
        double dist_score =
            (DIRECT_DILATION_current_white_pix_sum_dilated_comparison_image_B_ +
             gpu_metrics_->FastImplantDilationMetric(
                 gpu_principal_model_->GetSecondaryCameraRenderedImage(),
                 gpu_dilated_frames_B_->at(current_frame_index_),
                 DIRECT_DILATION_current_dilation_parameter));
        metric_score += (dist_score * dist_score);
    }

    return metric_score;
}
}  // namespace jta_cost_function
