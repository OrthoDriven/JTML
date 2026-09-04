// Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
// SPDX-License-Identifier: AGPL-3.0-only OR MIT

/*Cost Function Manager*/
#include "CostFunctionManager.h"

#include <limits>

namespace jta_cost_function {
/*Constructor/Destructor*/
CostFunctionManager::CostFunctionManager(Stage stage) :
    stage_(stage),
    gpu_metrics_(nullptr),
    gpu_edge_frames_A_(nullptr),
    gpu_dilated_frames_A_(nullptr),
    gpu_intensity_frames_A_(nullptr),
    gpu_edge_frames_B_(nullptr),
    gpu_dilated_frames_B_(nullptr),
    gpu_intensity_frames_B_(nullptr),
    gpu_principal_model_(nullptr),
    gpu_non_principal_models_(nullptr),
    pose_storage_(nullptr) {
    /*Load the listed cost functions to the vector of available cost functions*/
    listCostFunctions();

    setActiveCostFunction(CostFunctionType::DirectDilation);

    if (stage_ != Stage::Trunk && stage_ != Stage::Branch &&
        stage_ != Stage::Leaf) {
        stage_ = Stage::Trunk;
    }

    /*Current Frame Index (0 based)*/
    current_frame_index_ = 0;

    ///*Pose Storage*/
};
CostFunctionManager::CostFunctionManager() :
    stage_(Stage::Trunk),
    gpu_metrics_(nullptr),
    gpu_edge_frames_A_(nullptr),
    gpu_dilated_frames_A_(nullptr),
    gpu_intensity_frames_A_(nullptr),
    gpu_edge_frames_B_(nullptr),
    gpu_dilated_frames_B_(nullptr),
    gpu_intensity_frames_B_(nullptr),
    gpu_principal_model_(nullptr),
    gpu_non_principal_models_(nullptr),
    current_frame_index_(0),
    pose_storage_(nullptr),
    biplane_mode_(false) {
    listCostFunctions();

    setActiveCostFunction(CostFunctionType::DirectDilation);
};
CostFunctionManager::~CostFunctionManager() = default;

CostFunctionManager::CostFunctionManager(const CostFunctionManager& other) :
    CostFunctionManager() {
    *this = other;
}

CostFunctionManager& CostFunctionManager::operator=(
    const CostFunctionManager& other) {
    if (this == &other) {
        return *this;
    }

    /* Configuration */
    available_cost_functions_ = other.available_cost_functions_;
    active_cost_function_ = other.active_cost_function_;
    objective_spec = other.objective_spec;
    stage_ = other.stage_;

    /* Bound runtime resources */
    gpu_metrics_ = other.gpu_metrics_;

    gpu_edge_frames_A_ = other.gpu_edge_frames_A_;
    gpu_dilated_frames_A_ = other.gpu_dilated_frames_A_;
    gpu_intensity_frames_A_ = other.gpu_intensity_frames_A_;

    gpu_edge_frames_B_ = other.gpu_edge_frames_B_;
    gpu_dilated_frames_B_ = other.gpu_dilated_frames_B_;
    gpu_intensity_frames_B_ = other.gpu_intensity_frames_B_;

    gpu_distance_maps_ = other.gpu_distance_maps_;
    gpu_heatmaps_ = other.gpu_heatmaps_;

    gpu_principal_model_ = other.gpu_principal_model_;
    gpu_non_principal_models_ = other.gpu_non_principal_models_;

    prin_dist_ = other.prin_dist_;
    pose_storage_ = other.pose_storage_;

    current_frame_index_ = other.current_frame_index_;
    upload_epoch_ = other.upload_epoch_;
    biplane_mode_ = other.biplane_mode_;

    /*
     * ObjectiveInstance is bound/executable runtime state.
     * Never copy it. InitializeActiveCostFunction() constructs a fresh
     * instance after the copied manager is prepared for a run.
     */
    active_objective_instance_.reset();

    return *this;
}

/*Upload Data (Images,Poses etc.)*/
void CostFunctionManager::UploadData(
    std::vector<gpu_cost_function::GPUEdgeFrame*>* gpu_edge_frames_A,
    std::vector<gpu_cost_function::GPUDilatedFrame*>* gpu_dilated_frames_A,
    std::vector<gpu_cost_function::GPUIntensityFrame*>* gpu_intensity_frames_A,
    std::vector<gpu_cost_function::GPUEdgeFrame*>* gpu_edge_frames_B,
    std::vector<gpu_cost_function::GPUDilatedFrame*>* gpu_dilated_frames_B,
    std::vector<gpu_cost_function::GPUIntensityFrame*>* gpu_intensity_frames_B,
    gpu_cost_function::GPUModel* gpu_principal_model,
    std::vector<gpu_cost_function::GPUModel*>* gpu_non_principal_models,
    gpu_cost_function::GPUMetrics* gpu_metrics,
    PoseMatrix* pose_storage,
    bool biplane_mode) {
    /*Storage for Data (images, poses ,etc.) set to null*/
    /*Pointer to Vector of GPU Frame Pointers*/
    /*Camera A*/
    gpu_edge_frames_A_ = gpu_edge_frames_A;
    gpu_dilated_frames_A_ = gpu_dilated_frames_A;
    gpu_intensity_frames_A_ = gpu_intensity_frames_A;
    /*Camera B*/
    gpu_edge_frames_B_ = gpu_edge_frames_B;
    gpu_dilated_frames_B_ = gpu_dilated_frames_B;
    gpu_intensity_frames_B_ = gpu_intensity_frames_B;
    /*Pointer to Vector of principal GPU Model Pointer*/
    gpu_principal_model_ = gpu_principal_model;
    /*Pointer to Vector of non-principal GPU Model Pointers*/
    gpu_non_principal_models_ = gpu_non_principal_models;
    /*GPU Metrics Initialize*/
    gpu_metrics_ = gpu_metrics;

    /*Pose Storage Initialize*/
    pose_storage_ = pose_storage;

    /*Biplane Mode*/
    biplane_mode_ = biplane_mode;
};

void CostFunctionManager::UploadDistanceMap(
    std::vector<gpu_cost_function::GPUFrame*>* gpu_distance_maps,
    std::vector<gpu_cost_function::GPUHeatmap*>* gpu_heatmaps

) {
    gpu_distance_maps_ = gpu_distance_maps;
    gpu_heatmaps_ = gpu_heatmaps;
};

/*Set Active Cost Function*/
void CostFunctionManager::setActiveCostFunction(
    jta_cost_function::CostFunctionType cf_type) {
    active_cost_function_ = cf_type;
};

/*Update Cost Function Values from Saved Session*/
bool CostFunctionManager::updateCostFunctionParameterValues(
    CostFunctionType cost_function_type,
    std::string parameter_name,
    double value) {
    for (auto i :
         available_cost_functions_[cost_function_type].getDoubleParameters()) {
        if (i.getParameterName() == parameter_name) {
            i.setParameterValue(value);
            return true;
        }
    }
    /*Unsuccessful*/
    return false;
};
bool CostFunctionManager::updateCostFunctionParameterValues(
    CostFunctionType cost_function_type,
    std::string parameter_name,
    int value) {
    /*Check Active Cost Function Name Exists*/
    for (auto i :
         available_cost_functions_[cost_function_type].getIntParameters()) {
        if (i.getParameterName() == parameter_name) {
            i.setParameterValue(value);
            return true;
        }
    }
    /*Unsuccessful*/
    return false;
};
bool CostFunctionManager::updateCostFunctionParameterValues(
    CostFunctionType cost_function_type,
    std::string parameter_name,
    bool value) {
    /*Check Active Cost Function Name Exists*/
    for (auto i :
         available_cost_functions_[cost_function_type].getBoolParameters()) {
        if (i.getParameterName() == parameter_name) {
            i.setParameterValue(value);
            return true;
        }
    }
    /*Unsuccessful*/
    return false;
};

/*Return Available Cost Functions*/
std::map<CostFunctionType, CostFunction>
CostFunctionManager::getAvailableCostFunctions() {
    return available_cost_functions_;
};

/*Return Active Cost Function*/
CostFunctionType CostFunctionManager::getActiveCostFunction() {
    return active_cost_function_;
}

/*Return Active Cost Function Class*/
CostFunction* CostFunctionManager::getActiveCostFunctionClass() {
    return &available_cost_functions_[active_cost_function_];
};

/*Return Cost Function Class*/
CostFunction* CostFunctionManager::getCostFunctionClass(
    CostFunctionType cost_function_type) {
    return &available_cost_functions_[cost_function_type];
};

/*Set Current Frame Index*/
void CostFunctionManager::setCurrentFrameIndex(
    unsigned int current_frame_index) {
    current_frame_index_ = current_frame_index;
};

unsigned int CostFunctionManager::getCurrentFrameIndex() const {
    return current_frame_index_;
}

void CostFunctionManager::BumpUploadEpoch() {
    ++upload_epoch_;
}

std::uint64_t CostFunctionManager::getUploadEpoch() const {
    return upload_epoch_;
}

/*Call Active Cost Function*/
double CostFunctionManager::callActiveCostFunction() {
    switch (active_cost_function_) {
    case CostFunctionType::DirectDilation:
        // Virtual
        // return active_objective_instance_->evaluate(
        //     gpu_principal_model_->GetCurrentPrimaryCameraPose());
        // Non-Virtual
        // return static_cast<DirectDilationObjective*>(
        //            active_objective_instance_.get())
        //     ->evaluate(gpu_principal_model_->GetCurrentPrimaryCameraPose());
        return costFunctionDIRECT_DILATION();
    }
};
/*Call Stage Initializer for Active Cost Function*/
bool CostFunctionManager::InitializeActiveCostFunction(
    std::string& error_message) {
    switch (active_cost_function_) {
    case CostFunctionType::DirectDilation:
        // active_objective_instance_ =
        // std::make_unique<DirectDilationObjective>(
        //     std::get<DirectDilationSpec>(objective_spec),
        //     gpu_principal_model_,
        //     gpu_dilated_frames_A_->at(current_frame_index_),
        //     gpu_metrics_,
        //     biplane_mode_ ? gpu_dilated_frames_B_->at(current_frame_index_)
        //                   : nullptr);

        // return active_objective_instance_->initialize(error_message);
        return initializeDIRECT_DILATION(error_message);
    }
};
/*Call Stage Destructor for Active Cost Function*/
bool CostFunctionManager::DestructActiveCostFunction(
    std::string& error_message) {
    switch (active_cost_function_) {
    case CostFunctionType::DirectDilation:
        // active_objective_instance_.reset();
        // return true;
        return destructDIRECT_DILATION(error_message);
    }
};

/*List Cost Functions*/
void CostFunctionManager::listCostFunctions() {
    CostFunction instance_direct_dilation = CostFunction("DIRECT_DILATION");
    instance_direct_dilation.addParameter(Parameter<int>("Dilation", 6));
    available_cost_functions_[CostFunctionType::DirectDilation] =
        (instance_direct_dilation);
}

}  // namespace jta_cost_function
