/*
 * Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
 * SPDX-License-Identifier: AGPL-3.0-only OR MIT
 */

/*Manages Optimization in a Seperate QT Thread*/

#ifndef OPTIMIZER_MANAGER_H
#define OPTIMIZER_MANAGER_H

/*Custom CUDA Headers*/

#include <memory>

#include "domain/cost.h"

namespace gpu_cost_function {
struct GPUFrame;
struct GPUDilatedFrame;
struct GPUModel;
struct GPUMetrics;
}  // namespace gpu_cost_function

#include "compute/CostFunction.h"
#include "compute/Stage.h"
#include "compute/objective_instance.hpp"
#include "compute/objective_spec.h"
#include "domain/sym_trap_functions.h"
#include "objectives/direct_dilation.hpp"
#include "services/calibration.h"
/*QT Threading*/
#include <qobject.h>
#include <qthread.h>

#include <QModelIndex>
#include <QString>

/*Frame and Model and Location Storage*/
#include "compute/frame.h"
#include "services/location_storage.h"
#include "services/model.h"

/*Direct Library*/
#include "domain/data_structures_6D.h"

/*Optimizer Settings*/
#include "coordinator/optimizer_stage_script.h"
#include "services/optimizer_settings.h"

/*std::function (the BuildGpuCostAdapter return type)*/
#include <functional>

/*Metric Types*/

/*Cost Function Library*/

#include "optimizer-rs_bridge/bridge.h"
#include "rust/cxx.h"

using namespace gpu_cost_function;

class OptimizerManager : public QObject {
    Q_OBJECT

public:
    explicit OptimizerManager(QObject* parent = 0);
    /*Sets up Everything for Optimizer and Also Handles CUDA Initialization, Can
     * Fail!*/
    bool Initialize(
        QThread& optimizer_thread,
        Calibration calibration_file,
        std::vector<Frame> camera_A_frame_list,
        std::vector<Frame> camera_B_frame_list,
        unsigned int current_frame_index,
        std::vector<Model> model_list,
        QModelIndexList selected_models,
        unsigned int primary_model_index,
        LocationStorage pose_matrix,
        const OptimizerSettings& opt_settings,
        const QString& opt_directive,
        QString& error_message);
    ~OptimizerManager();

Q_SIGNALS:
    /*Update Blue Current Optimum*/
    void
    UpdateOptimum(double, double, double, double, double, double, unsigned int);
    /*Finished*/
    void finished();
    /*Finished Optimizing Frame, Send Optimum to MainScreen, The last bool
     * indicates if should move to next frame*/
    void OptimizedFrame(
        double,
        double,
        double,
        double,
        double,
        double,
        bool,
        unsigned int,
        bool,
        QString);
    /*Uh oh There was an Error. The string is the message*/
    void OptimizerError(QString);
    /*Update Display with Speed, Cost Function Calls, Current Minimum*/
    void UpdateDisplay(double, int, double, unsigned int);
    /*Update Dilation Background*/
    void UpdateDilationBackground();

    void CostFuncAtPoint(double);
    void
    onUpdateOrientationSymTrap(double, double, double, double, double, double);
    void onProgressBarUpdate(int);
    void get_iter_count();

public Q_SLOTS:
    /*Optimizer Biplane Single Model*/
    void Optimize();

    /*Emergency Stop*/
    void onStopOptimizer();

private:
    /*Initial Variables and Objects*/
    /*Calibration File*/
    Calibration calibration_;

    /*Optimizer Settings*/
    OptimizerSettings optimizer_settings_;

    /*SYM TRAP SETTINGS*/
    bool sym_trap_call{};
    // sym_trap *sym_trap_obj;

    /*Frames*/

    std::vector<Frame> frames_A_;
    /*Camera B Frames*/
    std::vector<Frame> frames_B_;

    /*Models: All Models, Selected Non-Primary Models, and Primary Model*/
    std::vector<Model> all_models_;
    std::vector<Model> selected_non_primary_models_;
    Model primary_model_;
    /*Indices of All Selected Models*/
    QModelIndexList selected_model_list_;
    /*Index of Primary Model*/
    unsigned int primary_model_index_{};

    /*Should we progess to next frame?*/
    bool progress_next_frame_{};
    /*Should we initialize with previous frame's best guess?*/
    bool init_prev_frame_{};
    /*Index For Starting Frame in Optimization*/
    unsigned int start_frame_index_{};
    unsigned int end_frame_index_{};

    std::vector<int> img_indices_;

    QString optimization_directive_;

    static void
    create_image_indices(std::vector<int>& img_indices, int start, int end);

    /*Error Check*/
    cudaError_t cuda_status_;

    /*Correctly Initialized*/
    bool succesfull_initialization_{};

    bool trunk_dark_silhouette_val_{};
    bool branch_dark_silhouette_val_{};
    bool leaf_dark_silhouette_val_{};

    /*GPU Metrics Class*/
    std::unique_ptr<GPUMetrics> gpu_metrics_;

    std::unique_ptr<GPUModel> gpu_principal_model_;
    std::vector<std::unique_ptr<GPUModel>> gpu_non_principal_models_;

    /*Set Search Range*/
    void SetSearchRange(Point6D range);

    /*Set Search Range*/
    void SetStartingPoint(Point6D starting_point);

    /*Actual Range of Search Direction for Each Variable*/
    Point6D range_;

    /*Starting Point For Search*/
    Point6D starting_point_;

    /*Valid Search Range*/
    bool valid_range_{};

    /*Budget*/
    unsigned int budget_{};

    void ResetStageDilation(size_t frame_index, int dilation);

    void RunDirectStage(
        Point6D range,
        ObjectiveInstance& objective,
        jta::StageKind kind,
        jta::StageSpec stage_spec);

    /*Cost Function Calls*/
    unsigned int cost_function_calls_{};

    /*Lowest Min Value*/
    double current_optimum_value_{};

    /*Argument (Location) of Lowest Min Value*/
    Point6D current_optimum_location_;

    /*Error Ocurred*/
    bool error_occurrred_{};

    /*Clock for Timing Speed*/
    /*(Milliseconds)*/
    clock_t start_clock_{}, update_screen_clock_{};

    /*Store Post Matrix on Cost Functions*/
    PoseMatrix pose_storage_;

    jta::StageScript stage_script_;

    /*Flag For Being in Either Trunk, Branch, or Z*/
    Stage search_stage_flag_;
};

namespace jta {

std::function<double(const Point6D&)> BuildGPUCostAdapter(
    gpu_cost_function::GPUModel* principal_model,
    Calibration calibration,
    ObjectiveInstance& objective);

}  // namespace jta

#endif /* OPTIMIZER_MANAGER_H */
