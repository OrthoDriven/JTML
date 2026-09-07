/**
 * @file mainscreen.h
 * @author Andrew Jensen (andrewjensen321@gmail.com)
 * @brief This is the MainScreen object class that controls the GUI.
 * @version 0.1
 * @date 2022-10-01
 *
 * @copyright Copyright (c) 2022
 *
 */
#ifndef MAINSCREEN_H
#define MAINSCREEN_H

/*Relevant QT Includes*/
#include <memory.h>
#include <qactiongroup.h>

#include <QtWidgets/QMainWindow>

#include "ui_mainscreen.h"
/*Font*/
#include <qfont.h>

/*Key Event*/
#include <QKeyEvent>
#include <numbers>

/*Direct Data Structures*/
#include "compute/curvature_utilities.h"
#include "domain/data_structures_6D.h"
/*Custom Calibration Struct (Used in CUDA GPU METRICS)*/
#include "compute/objective_spec.h"
#include "services/calibration.h"
#include "view/settings_impl.h"

/*VTK*/
#include <vtkActor.h>
#include <vtkAutoInit.h>  // Added post migration to Banks' lab computer
#include <vtkCamera.h>
#include <vtkDataSetMapper.h>
#include <vtkImageData.h>
#include <vtkInteractorStyleTrackballActor.h>
#include <vtkInteractorStyleTrackballCamera.h> /*Alternate Camera*/
#include <vtkPolyDataMapper.h>
#include <vtkProperty.h>
#include <vtkRenderWindow.h>
#include <vtkRenderWindowInteractor.h>
#include <vtkRenderer.h>
#include <vtkSTLReader.h>
#include <vtkSmartPointer.h>
#include <vtkTextActor.h>
#include <vtkTextProperty.h>
#include <vtkVersion.h>

/*Frame and Model and Location Storage*/
#include "compute/frame.h"
#include "services/location_storage.h"
#include "services/model.h"

/*Optimizer Settings*/
#include "coordinator/optimizer_run_controller.h"
#include "coordinator/session_state_controller.h"
#include "services/optimizer_settings.h"
#include "services/session_controller.h"
#include "services/settings_service.h"
#include "services/study_load_controller.h"

/*Optimizer Settings Control Window*/

#include "compute/camera_calibration.h"
#include "domain/session_state.h"

/*machine_learning_tools*/
#include "../../src/view/qml/qml_settings_dialog.h"
#include "compute/machine_learning_tools.h"
#include "services/ml_orchestrator.h"
#include "services/segmentation_controller.h"
#include "view/frame_list_model.h"
#include "view/model_list_model.h"
#include "view/viewer.h"

class MainScreen : public QMainWindow {
    Q_OBJECT

public:
    MainScreen(QWidget* parent = 0);

    ~MainScreen() override;

    /*Escape Signal from VTK to stop optimizer*/
    void VTKEscapeSignal();

    /*Make Selected Actor Principal from VTK*/
    void VTKMakePrincipalSignal(vtkActor* new_principal_actor);

    /*Bool to see if currently optimizing*/

    bool currently_optimizing_;

Q_SIGNALS:
    /*Update Whether To Write TO Text Display*/
    void UpdateDisplayText(bool);

private:
    double pi = std::numbers::pi;

    Ui::MainScreenClass ui;

    double UF_BLUE[3] = {0, 72, 204};
    double UF_ORANGE[3] = {255, 77, 0};

    int curr_frame();

    float start_time;

    /*GUI FUNCTIONS*/
    /*Arrange Layout (Do this in code so scales across different DPI monitors
     * and handles weird fonts)*/
    void ArrangeMainScreenLayout(QFont application_font);

    /*Private Variables*/
    /*Original Sizes After Construction for Main Screen List Widgets, their
     * Group Boxes and QVTK Widget*/
    int image_list_widget_starting_height_;
    int image_selection_box_starting_height_;
    int model_list_widget_starting_height_;
    int model_selection_box_starting_height_;
    int qvtk_widget_starting_height_;
    int qvtk_widget_starting_width_;

    /*Monoplane and Biplane Calibration Viewport Files*/
    Calibration calibration_file_; /*Used in monoplane and biplane*/

    /*Variables Indicating Calibration Status for Mono and Biplane*/
    bool calibrated_for_monoplane_viewport_;
    bool calibrated_for_biplane_viewport_;

    /*VTK Variables for main Viewer*/
    std::vector<vtkSmartPointer<vtkActor>> model_actor_list;
    std::vector<vtkSmartPointer<vtkPolyDataMapper>> model_mapper_list;
    vtkSmartPointer<vtkRenderer> renderer;
    vtkSmartPointer<vtkImageData> current_background;
    vtkSmartPointer<vtkSTLReader> stl_reader;
    vtkSmartPointer<vtkDataSetMapper> image_mapper;
    vtkSmartPointer<vtkActor> actor_image;
    vtkSmartPointer<vtkTextActor> actor_text;
    vtkSmartPointer<vtkInteractorStyleTrackballCamera> camera_style_interactor;

    /* VTK Variables for Coronal Plane Viewer*/
    vtkSmartPointer<vtkRenderer> coronal_renderer;

    // Main viewer
    std::shared_ptr<Viewer> vw = std::make_shared<Viewer>();

    // Coronal Plane Viewer
    std::shared_ptr<Viewer> coronal_vw = std::make_shared<Viewer>();

    /*View Menu Radio Button Container*/
    QActionGroup *alignmentGroup, *alignmentGroupSegment;

    /*Frame/Model Containers*/
    std::vector<Frame> loaded_frames;
    std::vector<Frame> loaded_frames_B; /*If Biplane mode, need second group of
                                           loaded frames for camera B*/
    std::vector<Model> loaded_models;
    /*Location Storage Class*/
    LocationStorage model_locations_;

    FrameListModel frame_list_model_;
    ModelListModel model_list_model_;

    jta::SessionState session_state_;

    SessionStateController session_state_controller_;

    jta::SessionController session_controller_;

    jta::StudyLoadController study_load_controller_;

    jta::SegmentationController segmentation_controller_;

    jta::MlOrchestrator ml_orchestrator_;

    /*Pull the current widget state into session_state_. Called wherever the
     * model/frame lists or their selection/current rows change.*/
    void SyncSessionState();

    /*Save the Pose From The Last Selected Frame*/
    void SaveLastPose();

    jta::SettingsService settings_service_;

    /*Optimizer Settings That Must Be Set in Constructor and Changed on
     * OSettings Update */
    OptimizerSettings optimizer_settings_;

    /*Copy of the Above Only Used While Optimizing to Display Output*/
    OptimizerSettings display_optimizer_settings_;

    void UpdateDilationFrames();

    /*Optimization Function: Packages Off The Optimization process in
    a new thread*/

    /*Launch Optimizer*/

    void LaunchOptimizer(OptimizerRunController::Directive directive);

    OptimizerRunController optimizer_run_controller_;

    /*Disable and Enable MainScreen During and After Optimization*/
    void DisableAll();

    void EnableAll();

    /*Calculate Viewing Angle (Accounts for Offsets)*/
    double CalculateViewingAngle(int width, int height, bool CameraA);

    /*Helper Function To Segment And Update Frames According to Model File*/
    void segmentHelperFunction(
        std::string pt_model_location,
        unsigned int input_width,
        unsigned int input_height);
    bool sym_trap_running;

    void update_image_list_widget(); /*Updates ui.image_list_widget*/

public Q_SLOTS:
    /*Load Buttons*/
    void on_load_calibration_button_clicked(); /*Load Calibration Clicked*/
    void on_load_image_button_clicked();       /*Load Images*/
    void on_load_model_button_clicked();       /*Load Models*/

    /*Biplane View Button (Monoplane is Biplane A, Biplans is Biplane B*/

    void on_camera_A_radio_button_clicked();

    void on_single_model_radio_button_clicked();
    void on_multiple_model_radio_button_clicked();

    void on_camera_B_radio_button_clicked();

    /*List Widgets*/
    void
    on_image_list_widget_itemSelectionChanged(); /*Image List Widget Changed*/
    void
    on_model_list_widget_itemSelectionChanged(); /*Model List Widget Changed*/

    void on_original_image_radio_button_clicked();
    void on_inverted_image_radio_button_clicked();
    void on_edges_image_radio_button_clicked();
    void on_dilation_image_radio_button_clicked();

    /*Model Radio Buttons*/
    void on_original_model_radio_button_clicked();

    void on_solid_model_radio_button_clicked();

    void on_transparent_model_radio_button_clicked();

    void on_wireframe_model_radio_button_clicked();

    /*Edge Buttons*/
    void on_aperture_spin_box_valueChanged();

    void on_low_threshold_slider_valueChanged();

    void on_high_threshold_slider_valueChanged();

    void on_apply_all_edge_button_clicked();

    void on_reset_edge_button_clicked();

    /*MenuBar*/
    void on_actionSave_Pose_triggered();

    void on_actionSave_Kinematics_triggered();

    void on_actionLoad_Pose_triggered();

    void on_actionLoad_Kinematics_triggered();

    void on_actionAbout_JointTrack_Auto_triggered();

    void on_actionControls_triggered();

    void on_actionStop_Optimizer_triggered();

    void on_actionReset_View_triggered();

    void on_actionReset_Normal_Up_triggered();

    void on_actionModel_Interaction_Mode_triggered();

    void on_actionCamera_Interaction_Mode_triggered();

    void on_actionSegment_FemHR_triggered();

    void on_actionSegment_TibHR_triggered();

    void on_actionReset_Remove_All_Segmentation_triggered();

    void on_actionEstimate_Femoral_Implant_s_triggered();

    void on_actionEstimate_Tibial_Implant_s_triggered();

    void on_actionCopy_Next_Pose_triggered();

    void on_actionCopy_Previous_Pose_triggered();

    void on_actionAmbiguous_Pose_Processing_triggered();

    /*Optimization Buttons*/
    void on_optimize_button_clicked();

    void on_optimize_all_button_clicked();

    void on_optimize_each_button_clicked();

    void on_optimize_from_button_clicked();

    void on_actionOptimize_Backward_triggered();

    /*OPTIMIZATION SLOTS*/
    /*Update Blue Current Optimum*/
    void onUpdateOptimum(
        double,
        double,
        double,
        double,
        double,
        double,
        unsigned int);

    void onOptimizedFrame(
        double,
        double,
        double,
        double,
        double,
        double,
        bool,
        unsigned int,
        bool,
        QString,
        bool);

    void onControllerMessage(
        const QString& title,
        const QString& message,
        OptimizerRunController::Severity severity);

    /*Update Display with Speed, Cost Function Calls, Current Minimum*/
    void onUpdateDisplay(double, int, double, unsigned int);

    /*Update Dilation Background if Radio Button is on Dilation and Moving
     * Betweeen Trunks and Branches*/
    void onUpdateDilationBackground();

    void
    updateOrientationSymTrap_MS(double, double, double, double, double, double);

    /*On Optimizer Control Windows Save Setting*/

protected:
    void resizeEvent(QResizeEvent* event) override;

    void keyPressEvent(QKeyEvent* event) override;
};

#endif /* MAINSCREEN_H */
