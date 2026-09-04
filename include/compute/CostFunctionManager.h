/*
 * Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
 * SPDX-License-Identifier: AGPL-3.0-only OR MIT
 */

#ifndef COSTFUNCTIONMANAGER_H
#define COSTFUNCTIONMANAGER_H

/*Class for Storing Cost Function Info*/
#include <cstdint>
#include <memory>

#include "CostFunction.h"
#include "compute/objective_spec.h"
#include "domain/preprocessor-defs.h"

/*Cost Function Tools Library*/

#include "compute/gpu_dilated_frame.cuh"
#include "compute/gpu_edge_frame.cuh"
#include "compute/gpu_frame.cuh"
#include "compute/gpu_heatmaps.cuh"
#include "compute/gpu_image.cuh"
#include "compute/gpu_intensity_frame.cuh"
#include "compute/gpu_metrics.cuh"
#include "compute/gpu_model.cuh"
#include "compute/objective_instance.hpp"
#include "compute/render_engine.cuh"
#include "objectives/direct_dilation.hpp"

/*Stage Enum*/
#include "Stage.h"

/*Standard Library*/

#include <map>
#include <vector>

namespace jta_cost_function {

class CostFunctionManager {
public:
    /*Constructor
    Called once when the client initially loads and populates the list of
    available cost functions. There will be three instances, one for each stage
    of DIRECT. Also sets an active cost function (default is the
    DIRECT_DILATION). The parameters are all default. To load previously saved
    session parameters, the constructor for the client will call the
    updateCostFunctionParameterValues(...)*/
    JTML_DLL CostFunctionManager(Stage stage);
    JTML_DLL CostFunctionManager();
    JTML_DLL CostFunctionManager(const CostFunctionManager& other);
    JTML_DLL CostFunctionManager& operator=(const CostFunctionManager& other);

    CostFunctionManager(CostFunctionManager&&) noexcept = default;
    CostFunctionManager& operator=(CostFunctionManager&&) noexcept = default;
    ~CostFunctionManager();

    /*Set Active Cost Function*/
    JTML_DLL void setActiveCostFunction(CostFunctionType cf_type);

    /*Update Cost Function Values from Saved Session*/

    /*Call Initialization for Active Cost Function*/
    JTML_DLL bool InitializeActiveCostFunction(std::string& error_message);

    /*Call Destructor for Active Cost Function*/
    JTML_DLL bool DestructActiveCostFunction(std::string& error_message);

    /*Call Active Cost Function*/
    JTML_DLL double callActiveCostFunction();

    /*Get Active Cost Function*/
    JTML_DLL CostFunctionType getActiveCostFunction();

    /*Get Active Cost Function Class*/
    JTML_DLL CostFunction* getActiveCostFunctionClass();

    /*Get Cost Function Class*/
    JTML_DLL CostFunction* getCostFunctionClass(
        CostFunctionType cost_function_type);

    /*Get Vector of Cost Functions*/
    JTML_DLL std::map<CostFunctionType, CostFunction>
    getAvailableCostFunctions();

    /*Set Current Frame Index*/
    JTML_DLL void setCurrentFrameIndex(unsigned int current_frame_index);

    /*Stage accessor — plan 008 U2 second documented wizard-region exception:
    minimal getStage() makes the stage-guard pin observable (stage_ is dead
    constructor state today; cfm_index is the future source of truth).*/
    Stage getStage() {
        return stage_;
    }

    /*Upload Data (Images,Poses etc.)*/
    JTML_DLL void UploadData(
        std::vector<gpu_cost_function::GPUEdgeFrame*>* gpu_edge_frames_A,
        std::vector<gpu_cost_function::GPUDilatedFrame*>* gpu_dilated_frames_A,
        std::vector<gpu_cost_function::GPUIntensityFrame*>*
            gpu_intensity_frames_A,
        std::vector<gpu_cost_function::GPUEdgeFrame*>* gpu_edge_frames_B,
        std::vector<gpu_cost_function::GPUDilatedFrame*>* gpu_dilated_frames_B,
        std::vector<gpu_cost_function::GPUIntensityFrame*>*
            gpu_intensity_frames_B,
        gpu_cost_function::GPUModel* gpu_principal_model,
        std::vector<gpu_cost_function::GPUModel*>* gpu_non_principal_models,
        gpu_cost_function::GPUMetrics* gpu_metrics,
        PoseMatrix* pose_storage,
        bool biplane_mode);

    JTML_DLL ObjectiveSpec objective_spec;

private:
    void listCostFunctions();

    /*Vector of Cost Functions*/
    std::map<CostFunctionType, CostFunction> available_cost_functions_;

    /*Active Cost Function*/
    CostFunctionType active_cost_function_;

#include "DIRECT_DILATIONCustomVariables.h"

    /*Stage Enum*/
    Stage stage_;

    /*Storage for GPU Metrics class*/
    gpu_cost_function::GPUMetrics* gpu_metrics_;

    /*Storage for Data (images, poses ,etc.)*/
    /*Pointer to Vector of GPU Frame Pointers*/
    /*Camera A*/
    std::vector<gpu_cost_function::GPUEdgeFrame*>* gpu_edge_frames_A_;
    std::vector<gpu_cost_function::GPUDilatedFrame*>* gpu_dilated_frames_A_;
    std::vector<gpu_cost_function::GPUIntensityFrame*>* gpu_intensity_frames_A_;
    /*Camera B*/
    std::vector<gpu_cost_function::GPUEdgeFrame*>* gpu_edge_frames_B_;
    std::vector<gpu_cost_function::GPUDilatedFrame*>* gpu_dilated_frames_B_;
    std::vector<gpu_cost_function::GPUIntensityFrame*>* gpu_intensity_frames_B_;

    std::vector<gpu_cost_function::GPUFrame*>* gpu_distance_maps_;
    std::vector<gpu_cost_function::GPUHeatmap*>* gpu_heatmaps_;

    /*Pointer to Vector of principal GPU Model Pointer*/
    gpu_cost_function::GPUModel* gpu_principal_model_;
    /*Pointer to Vector of non-principal GPU Model Pointers*/
    std::vector<gpu_cost_function::GPUModel*>* gpu_non_principal_models_;
    float* prin_dist_;
    /*Current Frame Index (0 based)*/
    unsigned int current_frame_index_ = 0;
    std::unique_ptr<ObjectiveInstance> active_objective_instance_;
    /*Pose Matrix*/
    PoseMatrix* pose_storage_;

    /*Biplane Mode?*/
    bool biplane_mode_;
    double costFunctionDIRECT_DILATION();
    bool initializeDIRECT_DILATION(std::string& error_message);
    bool destructDIRECT_DILATION(std::string& error_message);
};
}  // namespace jta_cost_function

#endif  // COSTFUNCTIONMANAGER_H
