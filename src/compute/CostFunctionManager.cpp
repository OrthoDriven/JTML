// Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
// SPDX-License-Identifier: AGPL-3.0-only OR MIT

/*Cost Function Manager*/
#include "CostFunctionManager.h"

#include <limits>
/******************************************************************************/
/******************************************************************************/
/******************************** BEGIN WARNING *******************************/
/******************************************************************************/
/*************************DO NOT EDIT ANYTING IN THIS FILE ********************/
/******************************************************************************/
/******************************************************************************/

namespace jta_cost_function {
/*Constructor/Destructor*/
CostFunctionManager::CostFunctionManager(Stage stage) {
    /*Load the listed cost functions to the vector of available cost functions*/
    listCostFunctions();

    /*Set Active Cost Function as the Default (DIRECT_DILATION)*/
    setActiveCostFunction(CostFunctionType::DirectDilation);

    /*Storage for Data (images, poses ,etc.) set to null*/
    /*Pointer to Vector of GPU Frame Pointers*/
    /*Camera A*/
    gpu_edge_frames_A_ = 0;
    gpu_dilated_frames_A_ = 0;
    gpu_intensity_frames_A_ = 0;
    /*Camera B*/
    gpu_edge_frames_B_ = 0;
    gpu_dilated_frames_B_ = 0;
    gpu_intensity_frames_B_ = 0;
    /*Pointer to Vector of principal GPU Model Pointer*/
    gpu_principal_model_ = 0;
    /*Pointer to Vector of non-principal GPU Model Pointers*/
    gpu_non_principal_models_ = 0;

    /*GPU Metrics Initialize*/
    gpu_metrics_ = 0;

    /*Pose Storage Initialize*/
    pose_storage_ = 0;

    /*Initialize stage*/
    stage_ = stage;
    /*Plan 008 U2 — first documented wizard-region exception: the original guard
    was a tautology (`||` — no Stage value equals all three members, so every
    manager collapsed to Trunk). `&&` forces Trunk only for invalid values.
    cfm_index (the StageScript) is the future stage source of truth; stage_ is
    constructor state kept for the getStage() accessor pin.*/
    if (stage_ != Stage::Trunk && stage_ != Stage::Branch &&
        stage_ != Stage::Leaf) {
        stage_ = Stage::Trunk;
    }

    /*Current Frame Index (0 based)*/
    current_frame_index_ = 0;

    ///*Pose Storage*/
};
CostFunctionManager::CostFunctionManager() {
    /*Load the listed cost functions to the vector of available cost functions*/
    listCostFunctions();

    /*Set Active Cost Function as the Default (DIRECT_DILATION)*/
    setActiveCostFunction(CostFunctionType::DirectDilation);

    /*Storage for Data (images, poses ,etc.) set to null*/
    /*Pointer to Vector of GPU Frame Pointers*/
    /*Camera A*/
    gpu_edge_frames_A_ = 0;
    gpu_dilated_frames_A_ = 0;
    gpu_intensity_frames_A_ = 0;
    /*Camera B*/
    gpu_edge_frames_B_ = 0;
    gpu_dilated_frames_B_ = 0;
    gpu_intensity_frames_B_ = 0;
    /*Pointer to Vector of principal GPU Model Pointer*/
    gpu_principal_model_ = 0;
    /*Pointer to Vector of non-principal GPU Model Pointers*/
    gpu_non_principal_models_ = 0;

    /*GPU Metrics Initialize*/
    gpu_metrics_ = 0;

    /*Pose Storage Initialize*/
    pose_storage_ = 0;

    /*Initialize stage*/
    stage_ = Stage::Trunk;

    /*Current Frame Index (0 based)*/
    current_frame_index_ = 0;

    /*Biplane Mode*/
    biplane_mode_ = false;
};
CostFunctionManager::~CostFunctionManager() {};

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
        return costFunctionDIRECT_DILATION();
    case CostFunctionType::SymmetryTrap:
        return costFunctionsym_trap_function();
    case CostFunctionType::DirectDilationNewPoleConstraint:
        return costFunctionDD_NEW_POLE_CONSTRAINT();
    case CostFunctionType::DirectDilationOldPoleConstraint:
        return costFunctionDIRECT_DILATION_POLE_CONSTRAINT();
    case CostFunctionType::DirectDilationConstrainZ:
        return costFunctionDIRECT_DILATION_SAME_Z();
    case CostFunctionType::DirectDilationOldT1:
        return costFunctionDIRECT_DILATION_T1();
    case CostFunctionType::DirectDilationMahfouzVariant:
        return costFunctionDIRECT_MAHFOUZ();
    }
};
/*Call Stage Initializer for Active Cost Function*/
bool CostFunctionManager::InitializeActiveCostFunction(
    std::string& error_message) {
    switch (active_cost_function_) {
    case CostFunctionType::DirectDilation:
        return initializeDIRECT_DILATION(error_message);
    case CostFunctionType::SymmetryTrap:
        return initializesym_trap_function(error_message);
    case CostFunctionType::DirectDilationNewPoleConstraint:
        return initializeDD_NEW_POLE_CONSTRAINT(error_message);
    case CostFunctionType::DirectDilationOldPoleConstraint:
        return initializeDIRECT_DILATION_POLE_CONSTRAINT(error_message);
    case CostFunctionType::DirectDilationConstrainZ:
        return initializeDIRECT_DILATION_SAME_Z(error_message);
    case CostFunctionType::DirectDilationOldT1:
        return initializeDIRECT_DILATION_T1(error_message);
    case CostFunctionType::DirectDilationMahfouzVariant:
        return initializeDIRECT_MAHFOUZ(error_message);
    }
};
/*Call Stage Destructor for Active Cost Function*/
bool CostFunctionManager::DestructActiveCostFunction(
    std::string& error_message) {
    switch (active_cost_function_) {
    case CostFunctionType::DirectDilation:
        return destructDIRECT_DILATION(error_message);
    case CostFunctionType::SymmetryTrap:
        return destructsym_trap_function(error_message);
    case CostFunctionType::DirectDilationNewPoleConstraint:
        return destructDD_NEW_POLE_CONSTRAINT(error_message);
    case CostFunctionType::DirectDilationOldPoleConstraint:
        return destructDIRECT_DILATION_POLE_CONSTRAINT(error_message);
    case CostFunctionType::DirectDilationConstrainZ:
        return destructDIRECT_DILATION_SAME_Z(error_message);
    case CostFunctionType::DirectDilationOldT1:
        return destructDIRECT_DILATION_T1(error_message);
    case CostFunctionType::DirectDilationMahfouzVariant:
        return destructDIRECT_MAHFOUZ(error_message);
    }
};

/*List Cost Functions*/
void CostFunctionManager::listCostFunctions() {
    /*DEFAULT COST FUNCTION*/
    /*Begin Cost Function Listing*/
    /*Cost Function Name: sym_trap_function*/
    /*Parameters: */
    CostFunction instance_sym_trap_function = CostFunction("sym_trap_function");
    instance_sym_trap_function.addParameter(Parameter<int>("Dilation", 3));
    instance_sym_trap_function.addParameter(
        Parameter<double>("PoleWeight", 75));
    instance_sym_trap_function.addParameter(Parameter<double>("VVWeight", 500));

    available_cost_functions_[CostFunctionType::SymmetryTrap] =
        instance_sym_trap_function;
    /*End Cost Function Listing*/

    /*Begin Cost Function Listing*/
    /*Cost Function Name: DD_NEW_POLE_CONSTRAINT*/
    /*Parameters: */
    CostFunction instance_DD_NEW_POLE_CONSTRAINT =
        CostFunction("DD_NEW_POLE_CONSTRAINT");
    instance_DD_NEW_POLE_CONSTRAINT.addParameter(Parameter<int>("Dilation", 3));
    instance_DD_NEW_POLE_CONSTRAINT.addParameter(
        Parameter<double>("PoleWeight", 75));
    instance_DD_NEW_POLE_CONSTRAINT.addParameter(
        Parameter<bool>("X_TRANS", false));
    instance_DD_NEW_POLE_CONSTRAINT.addParameter(
        Parameter<bool>("Y_TRANS", false));
    instance_DD_NEW_POLE_CONSTRAINT.addParameter(
        Parameter<bool>("Z_TRANS", false));
    available_cost_functions_
        [CostFunctionType::DirectDilationNewPoleConstraint] =
            (instance_DD_NEW_POLE_CONSTRAINT);
    /*End Cost Function Listing*/

    /*Begin Cost Function Listing*/
    /*Cost Function Name: DIRECT_DILATION_POLE_CONSTRAINT*/
    /*Parameters: */
    CostFunction instance_DIRECT_DILATION_POLE_CONSTRAINT =
        CostFunction("DIRECT_DILATION_POLE_CONSTRAINT");
    instance_DIRECT_DILATION_POLE_CONSTRAINT.addParameter(
        Parameter<double>("PoleWeight", 1));
    instance_DIRECT_DILATION_POLE_CONSTRAINT.addParameter(
        Parameter<double>("Pole_Weight", 1));
    instance_DIRECT_DILATION_POLE_CONSTRAINT.addParameter(
        Parameter<int>("Dilation", 6));
    available_cost_functions_
        [CostFunctionType::DirectDilationOldPoleConstraint] =
            (instance_DIRECT_DILATION_POLE_CONSTRAINT);
    /*End Cost Function Listing*/

    /*Begin Cost Function Listing*/
    /*Cost Function Name: DIRECT_DILATION_SAME_Z*/
    /*Parameters: */
    CostFunction instance_DIRECT_DILATION_SAME_Z =
        CostFunction("DIRECT_DILATION_SAME_Z");
    instance_DIRECT_DILATION_SAME_Z.addParameter(
        Parameter<double>("Z_Weight", 1));
    instance_DIRECT_DILATION_SAME_Z.addParameter(Parameter<int>("Dilation", 6));
    available_cost_functions_[CostFunctionType::DirectDilationConstrainZ] =
        (instance_DIRECT_DILATION_SAME_Z);
    /*End Cost Function Listing*/

    /*Begin Cost Function Listing*/
    /*Cost Function Name: DIRECT_DILATION_T1*/
    /*Parameters: */
    CostFunction instance_DIRECT_DILATION_T1 =
        CostFunction("DIRECT_DILATION_T1");
    instance_DIRECT_DILATION_T1.addParameter(Parameter<int>("Dilation", 6));
    available_cost_functions_[CostFunctionType::DirectDilationOldT1] =
        (instance_DIRECT_DILATION_T1);
    /*End Cost Function Listing*/

    /*Begin Cost Function Listing*/
    /*Cost Function Name: DIRECT_DILATION*/
    /*Parameters: */
    CostFunction instance_direct_dilation = CostFunction("DIRECT_DILATION");
    instance_direct_dilation.addParameter(Parameter<int>("Dilation", 6));
    available_cost_functions_[CostFunctionType::DirectDilation] =
        (instance_direct_dilation);
    /*End Cost Function Listing*/

    /*Begin Cost Function Listing*/
    /*Cost Function Name: DIRECT_MAHFOUZ*/
    /*Parameters: */
    CostFunction instance_direct_mahfouz = CostFunction("DIRECT_MAHFOUZ");
    instance_direct_mahfouz.addParameter(
        Parameter<bool>("Black_Silhouette", true));
    available_cost_functions_[CostFunctionType::DirectDilationMahfouzVariant] =
        (instance_direct_mahfouz);
    /*End Cost Function Listing*/
}
/*END FUNCTIONS THAT INTERACT WITH WIZARD*/
/******************************** END WARNING *********************************/
/******************************************************************************/
/*************************DO NOT EDIT FUNCTIONS ABOVE *************************/
/******************************************************************************/
}  // namespace jta_cost_function

/******************************************************************************/
/******************************************************************************/
/******************************** END WARNING
 * *********************************/
/******************************************************************************/
/*************************DO NOT EDIT ANYTING IN THIS FILE
 * ********************/
/******************************************************************************/
/******************************************************************************/
