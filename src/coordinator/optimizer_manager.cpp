// Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
// SPDX-License-Identifier: AGPL-3.0-only OR MIT

/*Optimizer Manaer*/
#include "coordinator/optimizer_manager.h"

/*Pose Matrix Class*/
#include <chrono>
#include <cstdlib>
#include <stdexcept>
#include <string>
#include <thread>
#include <utility>

#include "compute/gpu_model.cuh"
#include "compute/pose_matrix.h"

OptimizerManager::OptimizerManager(QObject* parent) : QObject(parent) {
    // this->sym_trap_obj = nullptr;
}

/*Initialize*/
bool OptimizerManager::Initialize(
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
    QString& error_message) {
    /*Success?*/
    succesfull_initialization_ = true;

    /*Error Check for Optimizer*/
    error_occurrred_ = false;

    /*Set up Thread Connections*/
    /*Connect Start of Thread to Optimisation Loop and Emergency Stop*/
    connect(&optimizer_thread, SIGNAL(started()), this, SLOT(Optimize()));

    /*Destructor Connections*/
    connect(this, SIGNAL(finished()), &optimizer_thread, SLOT(quit()));
    connect(this, SIGNAL(finished()), this, SLOT(deleteLater()));
    connect(
        &optimizer_thread,
        SIGNAL(finished()),
        &optimizer_thread,
        SLOT(deleteLater()));

    /*Store Calibration File Locally*/
    calibration_ = std::move(calibration_file);
    optimization_directive_ = opt_directive;

    /*Just In Case Have to Delete*/
    gpu_principal_model_ = nullptr;
    gpu_metrics_ = nullptr;

    /*Store Camera Frame Lists Locally and Check That, if Biplane is Enabled ->
    both lists are the same size. Also Check that the current frame index is
    within the range of the frame list sizes.*/
    frames_A_ = std::move(camera_A_frame_list);
    frames_B_ = std::move(camera_B_frame_list);
    if (calibration_.biplane_calibration &&
        frames_A_.size() != frames_B_.size()) {
        error_message =
            "Biplane mode enabled, but each camera has a different "
            "number of frames!";
        succesfull_initialization_ = false;
        return succesfull_initialization_;
    }
    if (current_frame_index >= frames_A_.size() ||
        (current_frame_index >= frames_B_.size() &&
         calibration_.biplane_calibration)) {
        error_message = "Current frame index is out of scope!";
        succesfull_initialization_ = false;
        return succesfull_initialization_;
    }

    /*Store Model List, Non-Primary Selected Models (Blue) and Primary Model
    Also store indices of all models and the index of the primary model*/
    all_models_ = std::move(model_list);
    if (selected_models.empty() ||
        std::cmp_not_equal(selected_models[0].row(), primary_model_index)) {
        error_message = "Can't find primary model!";
        succesfull_initialization_ = false;
        return succesfull_initialization_;
    }
    for (int i = 1; i < selected_models.size(); i++) {
        selected_non_primary_models_.push_back(
            all_models_[selected_models[i].row()]);
    }
    primary_model_ = all_models_[primary_model_index];
    selected_model_list_ = selected_models;
    primary_model_index_ = primary_model_index;

    /*Store Optimizer Settings Locally*/
    /*Store the per-stage optimizer-variant slot (plan 008 U8) -- consumed by
     * RunDirectStage's DirectOptimizer ctor; the defaults reproduce the
     * pre-Options search bit-identically.*/

    /*Store Cost Function Managers Locally*/

    /*Store Post Matrix on Cost Functions*/
    for (int i = 0; i < selected_models.size(); i++) {
        /*Construct Vector of Poses for Each Frame for Model*/
        int index_for_model = selected_models[i].row();
        std::vector<Pose> poses_each_frame_for_given_model;
        for (int j = 0; j < pose_matrix.GetFrameCount(); j++) {
            Point6D temp_p6d = pose_matrix.GetPose(j, index_for_model);
            auto temp_pose = Pose(
                temp_p6d.x,
                temp_p6d.y,
                temp_p6d.z,
                temp_p6d.xa,
                temp_p6d.ya,
                temp_p6d.za);
            poses_each_frame_for_given_model.push_back(temp_pose);
        }
        /*If i ==0, principal model*/
        if (i == 0) {
            pose_storage_.AddModel(
                poses_each_frame_for_given_model,
                all_models_[index_for_model].model_name_,
                true);
        } else {
            pose_storage_.AddModel(
                poses_each_frame_for_given_model,
                all_models_[index_for_model].model_name_,
                false);
        }
    }

    sym_trap_call = false;

    /*Use Optimization Directive To Resolve the Following local variables*/
    if (opt_directive == "Single") {
        /*Should we progess to next frame?*/
        progress_next_frame_ = false;
        /*Should we initialize with previous frame's best guess?*/
        init_prev_frame_ = false;
        /*Index For Starting Frame in Optimization*/
        start_frame_index_ = current_frame_index;
        end_frame_index_ = current_frame_index;

    } else if (opt_directive == "All") {
        /*Should we progess to next frame?*/
        progress_next_frame_ = true;
        /*Should we initialize with previous frame's best guess?*/
        init_prev_frame_ = true;
        /*Index For Starting Frame in Optimization*/
        start_frame_index_ = 0;
        end_frame_index_ = frames_A_.size() - 1;
    } else if (opt_directive == "Each") {
        /*Should we progess to next frame?*/
        progress_next_frame_ = true;
        /*Should we initialize with previous frame's best guess?*/
        init_prev_frame_ = false;
        /*Index For Starting Frame in Optimization*/
        start_frame_index_ = 0;
        end_frame_index_ = frames_A_.size() - 1;
    } else if (opt_directive == "From") {
        /*Should we progess to next frame?*/
        progress_next_frame_ = true;
        /*Should we initialize with previous frame's best guess?*/
        init_prev_frame_ = true;
        /*Index For Starting Frame in Optimization*/
        start_frame_index_ = current_frame_index;
        end_frame_index_ = frames_A_.size() - 1;
    } else if (opt_directive == "Sym_Trap") {
        /*Should we progess to next frame?*/
        progress_next_frame_ = false;
        /*Should we initialize with previous frame's best guess?*/
        init_prev_frame_ = false;
        /*Index For Starting Frame in Optimization*/
        start_frame_index_ = current_frame_index;
        /*U6: pin the single-frame scope (mirrors the Single branch). Without
         * this, end_frame_index_ stays UNINITIALIZED and create_image_indices
         * reads garbage (descending negative range -> UB, or a many-thousand-
         * iteration sym-trap hang). The tibia-after-femur oracle (plan 008
         * U6, jtml.oracle_multistage) is the first executor of this path. */
        end_frame_index_ = current_frame_index;

        sym_trap_call = true;
    } else if (opt_directive == "Backward") {
        progress_next_frame_ = true;
        init_prev_frame_ = true;
        start_frame_index_ = current_frame_index;
        end_frame_index_ = 0;
    } else {
        error_message = "Unrecognized optimization directive: " + opt_directive;
        succesfull_initialization_ = false;
        return succesfull_initialization_;
    }

    /*Plan 008 U9 (Cut B): build the run's StageScript ONCE from the settings
     * + directive — the container's stage policy is DATA (U7's pure builder),
     * and Optimize()'s per-frame loop iterates it. The manager's own
     * directive validation above runs FIRST (its "Unrecognized optimization
     * directive" error path is unchanged), so the builder's unknown-directive
     * error is unreachable here; a NEGATIVE-budget settings corruption fails
     * fast through the manager's existing error path (error_message + failed
     * Initialize) instead of the engine's silent acceptance — the pure
     * builder's documented strictness (U7), never hit by the tested shapes.*/
    try {
        stage_script_ = jta::jtml_production;
    } catch (const std::invalid_argument& e) {
        error_message = QString::fromStdString(e.what());
        succesfull_initialization_ = false;
        return succesfull_initialization_;
    }
    /*Setting Up image indices based on the directive launched*/
    create_image_indices(img_indices_, start_frame_index_, end_frame_index_);
    /*Set Up Settings*/
    SetSearchRange(optimizer_settings_.trunk_range);
    SetStartingPoint(
        pose_matrix.GetPose(start_frame_index_, primary_model_index_));
    budget_ = optimizer_settings_.trunk_budget;
    cost_function_calls_ = 0;
    current_optimum_value_ = DBL_MAX;
    current_optimum_location_ = starting_point_;

    /*Initialize GPU CUDA Cost Function Library Tools*/
    /*Get Width and Height (Safe since did error check before launching this
     * function)*/
    int width = frames_A_[0].GetEdgeImage().cols;
    int height = frames_A_[0].GetEdgeImage().rows;

    /*Check CUDA Compatibility*/
    int cuda_device_id = 0;
    int gpu_device_count = 0;
    int device_count = 0;
    struct cudaDeviceProp properties{};
    cudaError_t cudaResultCode = cudaGetDeviceCount(&device_count);
    if (cudaResultCode != cudaSuccess) {
        device_count = 0;
    }
    /* Machines with no GPUs can still report one emulation device */
    for (int device = 0; device < device_count; ++device) {
        cudaGetDeviceProperties(&properties, device);
        if (properties.major != 9999 &&
            properties.major >= 5) { /* 9999 means emulation only */
            ++gpu_device_count;
        }
    }
    /*If no Cuda Compatitble Devices with Compute Capability Greater Than 5,
     * Exit*/
    if (gpu_device_count == 0) {
        error_message =
            "No Cuda Compatitble Devices with Compute Capability Greater Than "
            "5!";
        succesfull_initialization_ = false;
        return succesfull_initialization_;
    }

    /* Upload GPU Models */
    /* Monoplane Calibration */
    if (!calibration_.biplane_calibration) {
        /* Principal Model */
        gpu_principal_model_ = std::make_unique<GPUModel>(
            primary_model_.model_name_,
            true,
            width,
            height,
            cuda_device_id,
            true,
            primary_model_.triangle_vertices_.data(),
            primary_model_.triangle_normals_.data(),
            primary_model_.triangle_vertices_.size() / 9,
            calibration_.camera_A_principal_);

        if (!gpu_principal_model_->IsInitializedCorrectly()) {
            gpu_principal_model_.reset();
            error_message = "Error uploading principal model to GPU!";
            succesfull_initialization_ = false;
            return succesfull_initialization_;
        }

        /* Non-principal models */
        for (int i = 1; i < selected_model_list_.size(); i++) {
            auto gpu_non_principal_model = std::make_unique<GPUModel>(
                all_models_[selected_model_list_[i].row()].model_name_,
                true,
                width,
                height,
                cuda_device_id,
                true,
                all_models_[selected_model_list_[i].row()]
                    .triangle_vertices_.data(),
                all_models_[selected_model_list_[i].row()]
                    .triangle_normals_.data(),
                all_models_[selected_model_list_[i].row()]
                        .triangle_vertices_.size() /
                    9,
                calibration_.camera_A_principal_);

            if (gpu_non_principal_model->IsInitializedCorrectly()) {
                gpu_non_principal_models_.push_back(
                    std::move(gpu_non_principal_model));
            } else {
                error_message = "Error uploading non-principal model to GPU!";
                succesfull_initialization_ = false;
                return succesfull_initialization_;
            }
        }
    }
    /* Biplane Calibration */
    else {
        /* Principal Model */
        gpu_principal_model_ = std::make_unique<GPUModel>(
            primary_model_.model_name_,
            true,
            width,
            height,
            cuda_device_id,
            cuda_device_id,
            true,
            true,
            primary_model_.triangle_vertices_.data(),
            primary_model_.triangle_normals_.data(),
            primary_model_.triangle_vertices_.size() / 9,
            calibration_.camera_A_principal_,
            calibration_.camera_B_principal_);

        if (!gpu_principal_model_->IsInitializedCorrectly()) {
            gpu_principal_model_.reset();
            error_message = "Error uploading principal model to GPU!";
            succesfull_initialization_ = false;
            return succesfull_initialization_;
        }

        /* Non-principal models */
        for (int i = 1; i < selected_model_list_.size(); i++) {
            auto gpu_non_principal_model = std::make_unique<GPUModel>(
                all_models_[selected_model_list_[i].row()].model_name_,
                true,
                width,
                height,
                cuda_device_id,
                cuda_device_id,
                true,
                true,
                all_models_[selected_model_list_[i].row()]
                    .triangle_vertices_.data(),
                all_models_[selected_model_list_[i].row()]
                    .triangle_normals_.data(),
                all_models_[selected_model_list_[i].row()]
                        .triangle_vertices_.size() /
                    9,
                calibration_.camera_A_principal_,
                calibration_.camera_B_principal_);

            if (gpu_non_principal_model->IsInitializedCorrectly()) {
                gpu_non_principal_models_.push_back(
                    std::move(gpu_non_principal_model));
            } else {
                error_message = "Error uploading non-principal model to GPU!";
                succesfull_initialization_ = false;
                return succesfull_initialization_;
            }
        }
    }

    /*Initialize GPU Metrics*/
    gpu_metrics_ = std::make_unique<GPUMetrics>();
    if (!gpu_metrics_->IsInitializedCorrectly()) {
        error_message = "GPU metrics class not initialized correctly!";
        succesfull_initialization_ = false;
        return succesfull_initialization_;
    }

    return succesfull_initialization_;
};

void OptimizerManager::SetSearchRange(Point6D range) {
    /*Check Search Range is Not Zero*/
    if (range.x + range.y + range.z + range.xa + range.ya + range.za > 0) {
        range_ = range;
        valid_range_ = true;
    } else {
        valid_range_ = false;
    }
}

void OptimizerManager::SetStartingPoint(Point6D starting_point) {
    starting_point_ = starting_point;
}

void OptimizerManager::Optimize() {
    /*Check That Succesfull Initialization*/
    if (!succesfull_initialization_) {
        /*Restore Dilation OpenCV Images*/
        for (auto& i : frames_A_) {
            dilate(
                i.GetEdgeImage(),
                i.GetDilationImage(),
                cv::Mat(),
                cv::Point(-1, -1),
                6); /*Reset Dilation In That Image*/
        }
        /*Camera B*/
        if (calibration_.biplane_calibration) {
            for (auto& i : frames_B_) {
                dilate(
                    i.GetEdgeImage(),
                    i.GetDilationImage(),
                    cv::Mat(),
                    cv::Point(-1, -1),
                    6); /*Reset Dilation In That Image*/
            }
        }

        /*Finish And Return Dont Have to Delete the Renderer and Metric as This
         * has Been Done*/
        emit finished();
        return;
    }

    /*Container for String Message*/
    std::string error_message;

    /*Loop Over Each Frame Loaded*/
    for (int frame_index : img_indices_) {
        int width = frames_A_[0].GetEdgeImage().cols;
        int height = frames_A_[0].GetEdgeImage().rows;
        int cuda_device_id = 0;

        if (!sym_trap_call) {
            /*Set Up Search Range and Starting Point*/
            if (!init_prev_frame_ || frame_index == 0) {
                Pose starting_pose;
                pose_storage_.GetModelPose(frame_index, &starting_pose);
                SetStartingPoint(Point6D(
                    starting_pose.x_location_,
                    starting_pose.y_location_,
                    starting_pose.z_location_,
                    starting_pose.x_angle_,
                    starting_pose.y_angle_,
                    starting_pose.z_angle_));
            } else {
                SetStartingPoint(current_optimum_location_);
            }

            /*Set Current Primary (and if biplane, secondary) Poses for Non
             * Principal Models*/
            for (auto& gpu_non_principal_model : gpu_non_principal_models_) {
                Pose temp_primary_pose;
                if (pose_storage_.GetModelPose(
                        gpu_non_principal_model->GetModelName(),
                        frame_index,
                        &temp_primary_pose)) {
                    gpu_non_principal_model->SetCurrentPrimaryCameraPose(
                        temp_primary_pose);
                } else {
                    emit OptimizerError(
                        QString::fromStdString(
                            "Could not retrieve pose for non-principal model "
                            "\"" +
                            gpu_non_principal_model->GetModelName() +
                            "\" at frame " +
                            QString::number(frame_index).toStdString() + "!"));
                    error_occurrred_ = true;
                    break;
                }
                if (calibration_.biplane_calibration) {
                    auto temp_primary_point = Point6D(
                        temp_primary_pose.x_location_,
                        temp_primary_pose.y_location_,
                        temp_primary_pose.z_location_,
                        temp_primary_pose.x_angle_,
                        temp_primary_pose.y_angle_,
                        temp_primary_pose.z_angle_);
                    Point6D temp_secondary_point =
                        calibration_.convert_Pose_A_to_Pose_B(
                            temp_primary_point);
                    gpu_non_principal_model->SetCurrentSecondaryCameraPose(Pose(
                        temp_secondary_point.x,
                        temp_secondary_point.y,
                        temp_secondary_point.z,
                        temp_secondary_point.xa,
                        temp_secondary_point.ya,
                        temp_secondary_point.za));
                }
            }

            /*Reset Budget and Cost Function Calls*/
            budget_ = optimizer_settings_.trunk_budget;
            cost_function_calls_ = 0;

            /*Initialize Search Stage Flag as Trunk*/
            search_stage_flag_ = Stage::Trunk;

            /*Start Clock*/
            start_clock_ = clock();
            update_screen_clock_ = clock();
        }

        for (const jta::StageSpec& spec : stage_script_) {
            std::unique_ptr<ObjectiveInstance> obj_instance_;
            std::unique_ptr<GPUDilatedFrame> frame_a;
            match(
                spec.obj_spec,
                [&](const jta_cost_function::DirectDilationSpec& s) {
                    std::unique_ptr<GPUDilatedFrame> frame_b = nullptr;

                    dilate(
                        frames_A_[frame_index].GetEdgeImage(),
                        frames_A_[frame_index].GetDilationImage(),
                        cv::Mat(),
                        cv::Point(-1, -1),
                        s.dilation);

                    frame_a = std::make_unique<GPUDilatedFrame>(
                        width,
                        height,
                        cuda_device_id,
                        frames_A_[frame_index].GetDilationImage().data,
                        s.dilation);

                    obj_instance_ = std::make_unique<DirectDilationObjective>(
                        s,
                        gpu_principal_model_.get(),
                        frame_a.get(),
                        gpu_metrics_.get());
                },
                [&](const auto&) { error_occurrred_ = true; });

            if (!obj_instance_) {
                emit OptimizerError("Failed to construct objective");
                error_occurrred_ = true;
                break;
            }

            if (!obj_instance_->initialize(error_message)) {
                emit OptimizerError(QString::fromStdString(error_message));
                error_occurrred_ = true;
                break;
            }

            RunDirectStage(spec.range, *obj_instance_, spec.kind, spec);
            SetStartingPoint(current_optimum_location_);

            if (error_occurrred_) {
                break;
            }
        }

        /*****************STAGE LOOP END *****************************/

        emit UpdateDilationBackground();

        /*Move on and Wrap Up*/
        // if (error_occurrred_ || std::cmp_equal(frame_index,
        // end_frame_index_)) {
        //     progress_next_frame_ = false;
        // }
        emit OptimizedFrame(
            current_optimum_location_.x,
            current_optimum_location_.y,
            current_optimum_location_.z,
            current_optimum_location_.xa,
            current_optimum_location_.ya,
            current_optimum_location_.za,
            progress_next_frame_,
            primary_model_index_,
            error_occurrred_,
            optimization_directive_);

        if (sym_trap_call) {
            emit finished();
            return;
        }

        emit UpdateDisplay(
            static_cast<double>(clock() - start_clock_) /
                static_cast<double>(cost_function_calls_),
            static_cast<int>(cost_function_calls_),
            current_optimum_value_,
            primary_model_index_);
        update_screen_clock_ = clock();

        /*Update Pose Storage*/
        auto current_opt_pose = Pose(
            current_optimum_location_.x,
            current_optimum_location_.y,
            current_optimum_location_.z,
            current_optimum_location_.xa,
            current_optimum_location_.ya,
            current_optimum_location_.za);
        pose_storage_.UpdatePrincipalModelPose(frame_index, current_opt_pose);

        /*If Error Occurred or Not Progressing Breank (Which Ends)*/
        if (!progress_next_frame_) {
            break;
        }
    }

    /*Finish And Return*/
    emit finished();
}

void OptimizerManager::ResetStageDilation(size_t frame_index, int dilation) {
    /*Make Sure the Dilation Image is Showing the Given Dilation Value (Reset
     * Dilation In That Image) — the dilate-A / dilate-B (if biplane) / emit
     * UpdateDilationBackground block, one place for the trunk/branch/leaf
     * stage specs (the dilation value is the only per-stage difference).*/
    dilate(
        frames_A_[frame_index].GetEdgeImage(),
        frames_A_[frame_index].GetDilationImage(),
        cv::Mat(),
        cv::Point(-1, -1),
        dilation); /*Reset Dilation In That Image*/
    if (calibration_.biplane_calibration) {
        dilate(
            frames_B_[frame_index].GetEdgeImage(),
            frames_B_[frame_index].GetDilationImage(),
            cv::Mat(),
            cv::Point(-1, -1),
            dilation); /*Reset Dilation In That Image*/
    }
}

void OptimizerManager::RunDirectStage(
    Point6D range,
    ObjectiveInstance& objective,
    jta::StageKind kind,
    jta::StageSpec stage_spec) {
    // auto serial_cost = jta::BuildGpuCostAdapter(
    //     gpu_principal_model_.get(), calibration_, stage_manager);
    auto serial_cost = jta::BuildGPUCostAdapter(
        gpu_principal_model_.get(), calibration_, objective);

#if USE_RUST_DIRECT
    CppCost cost = CppCost(serial_cost);
    bool use_bobyqa = (kind == jta::StageKind::Leaf);

    rust::Box<direct_rs::DirectOptimizer> rust_opt = direct_rs::new_rust_opt(
        range.to_array(),
        starting_point_.to_array(),
        (stage_spec.budget),
        use_bobyqa);

    RunOutcome out = rust_opt->run_rust_opt(cost);

    cost_function_calls_ += out.num_iter;
    current_optimum_location_ = Point6D(out.optimal_location);
    current_optimum_value_ = out.optimal_value;

#else
    DirectOptimizer opt(
        serial_cost, range, starting_point_, budget_, direct_options_);

    /*Cumulative budget semantics: this stage continues from the running call
     * count, so the extracted optimizer's loop guard uses call_offset_ + its
     * own count against the (already-accumulated) budget_ member.*/
    opt.SetCallOffset(cost_function_calls_);

    /*Live optimum display when the search improves (mirrors the original
     * UpdateOptimum emit inside EvaluateCostFunction).*/
    opt.SetImprovementCallback([this](const Point6D& loc, double) {
        emit UpdateOptimum(
            loc.x, loc.y, loc.z, loc.xa, loc.ya, loc.za, primary_model_index_);
    });

    /*Progress at ~30fps + cooperative stop, fired after each ConvexHull+Trisect
     * iteration (the stage's cooperative break boundary). onStopOptimizer sets
     * error_occurrred_, which is NOT polled inside DirectOptimizer::Run(), so
     * forward it to opt.Stop() here -- mirroring the original per-stage
     * `if (error_occurrred_) break;`.*/
    opt.SetIterationCallback([this, &opt]() {
        if (error_occurrred_) {
            opt.Stop();
            return;
        }
        if ((clock() - update_screen_clock_) > 33) {
            emit UpdateDisplay(
                static_cast<double>(clock() - start_clock_) /
                    static_cast<double>(opt.GetCostFunctionCalls()),
                static_cast<int>(opt.GetCostFunctionCalls()),
                opt.GetOptimumValue(),
                primary_model_index_);
            update_screen_clock_ = clock();
        }
    });

    {
        QString stageError;
        if (!jta::RunDirectStageGuarded(opt, &stageError)) {
            emit OptimizerError(
                stageError.isEmpty()
                    ? QStringLiteral("Error optimizing current frame!")
                    : stageError);
            error_occurrred_ = true;
            return;
        }
    }

    /*Write the stage result back into the running members.*/
    cost_function_calls_ = opt.GetCostFunctionCalls();
    current_optimum_location_ = opt.GetOptimumLocation();
    current_optimum_value_ = opt.GetOptimumValue();
#endif
}

void OptimizerManager::onStopOptimizer() {
    error_occurrred_ = true;
}

void OptimizerManager::create_image_indices(
    std::vector<int>& img_indices,
    int start,
    int end) {
    if (start < end) {
        for (int i = start; i <= end; i++) {
            img_indices.push_back(i);
        }
    } else if (start > end) {
        for (int i = start; i >= end; i--) {
            img_indices.push_back(i);
        }
    } else if (start == end) {
        int i = start;
        img_indices.push_back(i);
    }
}

/*Destructor*/
OptimizerManager::~OptimizerManager() {};

std::function<double(const Point6D&)> jta::BuildGPUCostAdapter(
    gpu_cost_function::GPUModel* principal_model,
    Calibration calibration,
    ObjectiveInstance& objective) {
    return [principal_model, calibration, &objective](
               const Point6D& physical) mutable -> double {
        Pose pose(
            physical.x,
            physical.y,
            physical.z,
            physical.xa,
            physical.ya,
            physical.za);
        principal_model->SetCurrentPrimaryCameraPose(pose);
        return objective.evaluate(pose);
    };
}
