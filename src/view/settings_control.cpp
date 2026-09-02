// Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
// SPDX-License-Identifier: AGPL-3.0-only OR MIT

/*Settings Control Window*/
#include "view/settings_control.h"

/*Message Box*/
#include <qmessagebox.h>

#include <string>

/*Settings Constant*/
#include "domain/settings_constants.h"

/*Spacing Constants*/
#include "view/settings_window_size_constants.h"

using jta_cost_function::cost_function_type_from_string;
using jta_cost_function::CostFunctionType;
using jta_cost_function::to_string;

namespace {

QString costFunctionTypeToQString(CostFunctionType type) {
    return QString::fromStdString(std::string(to_string(type)));
}

}  // namespace

SettingsControl::SettingsControl(QWidget* parent, Qt::WindowFlags flags) :
    QDialog(parent, flags) {
    ui.setupUi(this);

    connect(this, SIGNAL(Done()), this, SLOT(close()));

    /*Set Layout of All the Differnt Buttons and Boxes etc*/
    /*Resize Buttons based on Font Size In Order to be Compatible with High DPI
     * Monitors*/
    QFontMetrics font_metrics(this->font());

    /*Adjust for Title Height*/
    this->setStyleSheet(
        this->styleSheet() += "QGroupBox { margin-top: " +
            QString::number(font_metrics.height() / 2) + "px; }");
    int group_box_to_top_button_y = font_metrics.height() / 2;

    /*Make Sure Trunk is checked and by default so is stage enabled*/
    ui.trunk_radioButton->setChecked(true);
    ui.stage_enabled_checkBox->setChecked(true);

    /*Load Cost Function Names in Constructor as these cannot change during a
     * session. The manager registry is keyed by CostFunctionType.*/
    auto available_cost_functions =
        jta_cost_function::CostFunctionManager().getAvailableCostFunctions();
    for (auto& cost_function_entry : available_cost_functions) {
        ui.cost_function_listWidget->addItem(
            costFunctionTypeToQString(cost_function_entry.first));
    }

    /*Set Up Stage Select Group Box Size */
    /*Get Size of Radio Button Widths*/
    int stage_radio_button_widths = INSIDE_RADIO_BUTTON_PADDING_X +
        std::max(std::max(
                     font_metrics.horizontalAdvance("Trunk"),
                     font_metrics.horizontalAdvance("Branch")),
                 font_metrics.horizontalAdvance("Leaf"));
    /*Check if Horizontal Spacing is Big Enough for Title*/
    int safety_padding_x = 0;
    if (3 * stage_radio_button_widths + 2 * GROUP_BOX_TO_RADIO_BUTTON_X +
            2 * BUTTON_TO_BUTTON_PADDING_X >
        1.25 *
            font_metrics.horizontalAdvance(
                ui.optimization_search_stage_groupBox->title())) {
        ui.trunk_radioButton->setGeometry(QRect(
            GROUP_BOX_TO_RADIO_BUTTON_X,
            GROUP_BOX_TO_RADIO_BUTTON_PADDING_Y + (font_metrics.height() / 2),
            stage_radio_button_widths,
            font_metrics.height() + INSIDE_RADIO_BUTTON_PADDING_Y));
        ui.branch_radioButton->setGeometry(QRect(
            ui.trunk_radioButton->geometry().right() +
                BUTTON_TO_BUTTON_PADDING_X,
            GROUP_BOX_TO_RADIO_BUTTON_PADDING_Y + (font_metrics.height() / 2),
            stage_radio_button_widths,
            font_metrics.height() + INSIDE_RADIO_BUTTON_PADDING_Y));
        ui.leaf_radioButton->setGeometry(QRect(
            ui.branch_radioButton->geometry().right() +
                BUTTON_TO_BUTTON_PADDING_X,
            GROUP_BOX_TO_RADIO_BUTTON_PADDING_Y + (font_metrics.height() / 2),
            stage_radio_button_widths,
            font_metrics.height() + INSIDE_RADIO_BUTTON_PADDING_Y));
    } else {
        safety_padding_x =
            (1.25 *
                 font_metrics.horizontalAdvance(
                     ui.optimization_search_stage_groupBox->title()) -
             (3 * stage_radio_button_widths + 2 * GROUP_BOX_TO_RADIO_BUTTON_X +
              2 * BUTTON_TO_BUTTON_PADDING_X)) /
            2;
        ui.trunk_radioButton->setGeometry(QRect(
            safety_padding_x + GROUP_BOX_TO_RADIO_BUTTON_X,
            GROUP_BOX_TO_RADIO_BUTTON_PADDING_Y + (font_metrics.height() / 2),
            stage_radio_button_widths,
            font_metrics.height() + INSIDE_RADIO_BUTTON_PADDING_Y));
        ui.branch_radioButton->setGeometry(QRect(
            ui.trunk_radioButton->geometry().right() +
                BUTTON_TO_BUTTON_PADDING_X,
            GROUP_BOX_TO_RADIO_BUTTON_PADDING_Y + (font_metrics.height() / 2),
            stage_radio_button_widths,
            font_metrics.height() + INSIDE_RADIO_BUTTON_PADDING_Y));
        ui.leaf_radioButton->setGeometry(QRect(
            ui.branch_radioButton->geometry().right() +
                BUTTON_TO_BUTTON_PADDING_X,
            GROUP_BOX_TO_RADIO_BUTTON_PADDING_Y + (font_metrics.height() / 2),
            stage_radio_button_widths,
            font_metrics.height() + INSIDE_RADIO_BUTTON_PADDING_Y));
    }
    /*Set Dimensions of Box*/
    ui.optimization_search_stage_groupBox->setGeometry(QRect(
        0,
        0,
        ui.leaf_radioButton->geometry().right() + safety_padding_x +
            GROUP_BOX_TO_RADIO_BUTTON_X,
        ui.leaf_radioButton->geometry().bottom() +
            +(font_metrics.height() / 2) +
            GROUP_BOX_TO_RADIO_BUTTON_PADDING_Y));

    /*Set Up Dimensions for Range Group Box*/
    ui.range_groupBox->setGeometry(QRect(
        GROUP_BOX_TO_SMALL_GROUP_BOX_X,
        group_box_to_top_button_y + font_metrics.height() +
            2 * SMALL_GROUP_BOX_PADDING_Y + 7,
        font_metrics.horizontalAdvance(ui.x_translation_label->text()) +
            font_metrics.horizontalAdvance(ui.x_rotation_label->text()) +
            LABEL_TO_SPIN_BOX_PADDING_X * 2 + SPIN_BOX_TO_LABEL_PADDING_X +
            SMALL_GROUP_BOX_PADDING_X * 2 + INSIDE_SPIN_BOX_PADDING_X * 2 +
            2 * font_metrics.horizontalAdvance("XXX"),
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y * 2 +
            font_metrics.height() * 3 + 3 * INSIDE_SPIN_BOX_PADDING_Y +
            2 * SPIN_BOX_TO_SPIN_BOX_PADDING_Y));
    ui.x_translation_label->setGeometry(QRect(
        SMALL_GROUP_BOX_PADDING_X,
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y,
        font_metrics.horizontalAdvance(ui.x_translation_label->text()),
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.y_translation_label->setGeometry(QRect(
        SMALL_GROUP_BOX_PADDING_X,
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y +
            font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y +
            SPIN_BOX_TO_SPIN_BOX_PADDING_Y,
        font_metrics.horizontalAdvance(ui.y_translation_label->text()),
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.z_translation_label->setGeometry(QRect(
        SMALL_GROUP_BOX_PADDING_X,
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y +
            2 *
                (font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y +
                 SPIN_BOX_TO_SPIN_BOX_PADDING_Y),
        font_metrics.horizontalAdvance(ui.z_translation_label->text()),
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.x_rotation_label->setGeometry(QRect(
        font_metrics.horizontalAdvance(ui.x_translation_label->text()) +
            SMALL_GROUP_BOX_PADDING_X + INSIDE_SPIN_BOX_PADDING_X +
            font_metrics.horizontalAdvance("XXX") +
            LABEL_TO_SPIN_BOX_PADDING_X + SMALL_GROUP_BOX_PADDING_X,
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y,
        font_metrics.horizontalAdvance(ui.x_rotation_label->text()),
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.y_rotation_label->setGeometry(QRect(
        font_metrics.horizontalAdvance(ui.x_translation_label->text()) +
            SMALL_GROUP_BOX_PADDING_X + INSIDE_SPIN_BOX_PADDING_X +
            font_metrics.horizontalAdvance("XXX") +
            LABEL_TO_SPIN_BOX_PADDING_X + SMALL_GROUP_BOX_PADDING_X,
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y +
            font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y +
            SPIN_BOX_TO_SPIN_BOX_PADDING_Y,
        font_metrics.horizontalAdvance(ui.y_rotation_label->text()),
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.z_rotation_label->setGeometry(QRect(
        font_metrics.horizontalAdvance(ui.x_translation_label->text()) +
            SMALL_GROUP_BOX_PADDING_X + INSIDE_SPIN_BOX_PADDING_X +
            font_metrics.horizontalAdvance("XXX") +
            LABEL_TO_SPIN_BOX_PADDING_X + SMALL_GROUP_BOX_PADDING_X,
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y +
            2 *
                (font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y +
                 SPIN_BOX_TO_SPIN_BOX_PADDING_Y),
        font_metrics.horizontalAdvance(ui.z_rotation_label->text()),
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.x_translation_spinBox->setGeometry(QRect(
        font_metrics.horizontalAdvance(ui.x_translation_label->text()) +
            LABEL_TO_SPIN_BOX_PADDING_X + SMALL_GROUP_BOX_PADDING_X,
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y,
        font_metrics.horizontalAdvance("XXX") + INSIDE_SPIN_BOX_PADDING_X,
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.y_translation_spinBox->setGeometry(QRect(
        font_metrics.horizontalAdvance(ui.x_translation_label->text()) +
            LABEL_TO_SPIN_BOX_PADDING_X + SMALL_GROUP_BOX_PADDING_X,
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y +
            font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y +
            SPIN_BOX_TO_SPIN_BOX_PADDING_Y,
        font_metrics.horizontalAdvance("XXX") + INSIDE_SPIN_BOX_PADDING_X,
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.z_translation_spinBox->setGeometry(QRect(
        font_metrics.horizontalAdvance(ui.x_translation_label->text()) +
            LABEL_TO_SPIN_BOX_PADDING_X + SMALL_GROUP_BOX_PADDING_X,
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y +
            2 *
                (font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y +
                 SPIN_BOX_TO_SPIN_BOX_PADDING_Y),
        font_metrics.horizontalAdvance("XXX") + INSIDE_SPIN_BOX_PADDING_X,
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.x_rotation_spinBox->setGeometry(QRect(
        font_metrics.horizontalAdvance(ui.x_translation_label->text()) +
            LABEL_TO_SPIN_BOX_PADDING_X +
            font_metrics.horizontalAdvance(ui.x_rotation_label->text()) +
            SMALL_GROUP_BOX_PADDING_X + INSIDE_SPIN_BOX_PADDING_X +
            font_metrics.horizontalAdvance("XXX") +
            SPIN_BOX_TO_LABEL_PADDING_X + LABEL_TO_SPIN_BOX_PADDING_X,
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y,
        font_metrics.horizontalAdvance("XXX") + INSIDE_SPIN_BOX_PADDING_X,
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.y_rotation_spinBox->setGeometry(QRect(
        font_metrics.horizontalAdvance(ui.x_translation_label->text()) +
            LABEL_TO_SPIN_BOX_PADDING_X +
            font_metrics.horizontalAdvance(ui.x_rotation_label->text()) +
            SMALL_GROUP_BOX_PADDING_X + INSIDE_SPIN_BOX_PADDING_X +
            font_metrics.horizontalAdvance("XXX") +
            SPIN_BOX_TO_LABEL_PADDING_X + LABEL_TO_SPIN_BOX_PADDING_X,
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y +
            font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y +
            SPIN_BOX_TO_SPIN_BOX_PADDING_Y,
        font_metrics.horizontalAdvance("XXX") + INSIDE_SPIN_BOX_PADDING_X,
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.z_rotation_spinBox->setGeometry(QRect(
        font_metrics.horizontalAdvance(ui.x_translation_label->text()) +
            LABEL_TO_SPIN_BOX_PADDING_X +
            font_metrics.horizontalAdvance(ui.x_rotation_label->text()) +
            SMALL_GROUP_BOX_PADDING_X + INSIDE_SPIN_BOX_PADDING_X +
            font_metrics.horizontalAdvance("XXX") +
            SPIN_BOX_TO_LABEL_PADDING_X + LABEL_TO_SPIN_BOX_PADDING_X,
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y +
            2 *
                (font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y +
                 SPIN_BOX_TO_SPIN_BOX_PADDING_Y),
        font_metrics.horizontalAdvance("XXX") + INSIDE_SPIN_BOX_PADDING_X,
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));

    /*Set the Y Positioning and Size of Everything in the General Options Left
     * Column*/
    ui.stage_enabled_checkBox->setGeometry(QRect(
        0,
        GROUP_BOX_TO_RADIO_BUTTON_PADDING_Y + (font_metrics.height() / 2),
        INSIDE_RADIO_BUTTON_PADDING_X +
            font_metrics.horizontalAdvance(ui.stage_enabled_checkBox->text()),
        font_metrics.height() + INSIDE_RADIO_BUTTON_PADDING_Y));
    ui.stage_budget_label->setGeometry(QRect(
        0,
        ui.stage_enabled_checkBox->geometry().bottom() + CHECKBOX_TO_LABEL_Y,
        font_metrics.horizontalAdvance(ui.stage_budget_label->text()),
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.budget_spinBox->setGeometry(QRect(
        font_metrics.horizontalAdvance(ui.stage_budget_label->text()) +
            LABEL_TO_SPIN_BOX_PADDING_X + SMALL_GROUP_BOX_PADDING_X,
        ui.stage_budget_label->geometry().top(),
        font_metrics.horizontalAdvance("XXXXXXX") + INSIDE_SPIN_BOX_PADDING_X,
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.range_groupBox->setGeometry(QRect(
        GROUP_BOX_TO_SMALL_GROUP_BOX_X,
        ui.stage_budget_label->geometry().bottom() +
            GROUP_BOX_TO_LABEL_PADDING_Y,
        ui.range_groupBox->geometry().width(),
        ui.range_groupBox->geometry().height()));
    ui.branch_total_count_label->setGeometry(QRect(
        (ui.range_groupBox->geometry().width() -
         (font_metrics.horizontalAdvance(ui.branch_total_count_label->text()) +
          LABEL_TO_SPIN_BOX_PADDING_X + font_metrics.horizontalAdvance("XXX") +
          INSIDE_SPIN_BOX_PADDING_X)) /
            2,
        group_box_to_top_button_y + SMALL_GROUP_BOX_PADDING_Y,
        font_metrics.horizontalAdvance(ui.branch_total_count_label->text()),
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.branch_count_spinBox->setGeometry(QRect(
        ui.branch_total_count_label->geometry().right() +
            LABEL_TO_SPIN_BOX_PADDING_X,
        ui.branch_total_count_label->geometry().top(),
        font_metrics.horizontalAdvance("XXX") + INSIDE_SPIN_BOX_PADDING_X,
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.stage_specific_groupBox->setGeometry(QRect(
        GROUP_BOX_TO_SMALL_GROUP_BOX_X,
        ui.range_groupBox->geometry().bottom() + GROUP_BOX_TO_GROUP_BOX_Y,
        ui.range_groupBox->geometry().width(),
        ui.branch_total_count_label->geometry().bottom() +
            (ui.range_groupBox->geometry().height() -
             ui.z_translation_label->geometry().bottom())));

    /*Shift the rest of the Left Column to the correct horizontal position*/
    ui.stage_enabled_checkBox->move(QPoint(
        ui.range_groupBox->geometry().left() +
            ((ui.range_groupBox->geometry().width() -
              ui.stage_enabled_checkBox->geometry().width()) /
             2),
        ui.stage_enabled_checkBox->geometry().top()));
    ui.stage_budget_label->move(QPoint(
        ui.range_groupBox->geometry().left() +
            ((ui.range_groupBox->geometry().width() -
              (ui.stage_budget_label->geometry().width() +
               LABEL_TO_SPIN_BOX_PADDING_X +
               ui.budget_spinBox->geometry().width())) /
             2),
        ui.stage_budget_label->geometry().top()));
    ui.budget_spinBox->move(QPoint(
        LABEL_TO_SPIN_BOX_PADDING_X + ui.stage_budget_label->geometry().right(),
        ui.budget_spinBox->geometry().top()));

    /*Central Column*/
    ui.cost_function_groupBox->setGeometry(QRect(
        ui.range_groupBox->geometry().right() + GROUP_BOX_TO_GROUP_BOX_X,
        ui.stage_enabled_checkBox->geometry().top() + group_box_to_top_button_y,
        ui.range_groupBox->geometry().width(),
        ui.stage_specific_groupBox->geometry().bottom() -
            (ui.stage_enabled_checkBox->geometry().top() +
             group_box_to_top_button_y)));
    ui.cost_function_listWidget->setGeometry(QRect(
        GROUP_BOX_TO_SMALL_GROUP_BOX_X,
        group_box_to_top_button_y + GROUP_BOX_TO_SMALL_GROUP_BOX_Y,
        ui.cost_function_groupBox->geometry().width() -
            2 * GROUP_BOX_TO_SMALL_GROUP_BOX_X,
        ui.cost_function_groupBox->geometry().height() -
            (group_box_to_top_button_y + 2 * GROUP_BOX_TO_SMALL_GROUP_BOX_Y)));

    /*Right Column*/
    ui.double_parameter_spinBox->setGeometry(QRect(
        (ui.cost_function_groupBox->geometry().width() -
         (font_metrics.horizontalAdvance("XXXXXXXXXX") +
          INSIDE_SPIN_BOX_PADDING_X)) /
            2,
        group_box_to_top_button_y + GROUP_BOX_TO_LABEL_PADDING_Y,
        font_metrics.horizontalAdvance("XXXXXXXXXX") +
            INSIDE_SPIN_BOX_PADDING_X,
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    ui.int_parameter_spinBox->setGeometry(QRect(
        (ui.cost_function_groupBox->geometry().width() -
         (font_metrics.horizontalAdvance("XXXXXXXXXX") +
          INSIDE_SPIN_BOX_PADDING_X)) /
            2,
        group_box_to_top_button_y + GROUP_BOX_TO_LABEL_PADDING_Y,
        font_metrics.horizontalAdvance("XXXXXXXXXX") +
            INSIDE_SPIN_BOX_PADDING_X,
        font_metrics.height() + INSIDE_SPIN_BOX_PADDING_Y));
    /*T/F Radio Button width*/
    int true_false_radiobutton_width = INSIDE_RADIO_BUTTON_PADDING_X +
        std::max(font_metrics.horizontalAdvance(
                     ui.bool_parameter_true_radioButton->text()),
                 font_metrics.horizontalAdvance(
                     ui.bool_parameter_false_radioButton->text()));
    ui.bool_parameter_true_radioButton->setGeometry(QRect(
        (ui.cost_function_groupBox->geometry().width() -
         (2 * true_false_radiobutton_width + BUTTON_TO_BUTTON_PADDING_X)) /
            2,
        2 + ui.int_parameter_spinBox->geometry().top(),
        true_false_radiobutton_width,
        font_metrics.height() + INSIDE_RADIO_BUTTON_PADDING_Y));
    ui.bool_parameter_false_radioButton->setGeometry(QRect(
        ui.bool_parameter_true_radioButton->geometry().right() +
            BUTTON_TO_BUTTON_PADDING_X,
        2 + ui.int_parameter_spinBox->geometry().top(),
        true_false_radiobutton_width,
        font_metrics.height() + INSIDE_RADIO_BUTTON_PADDING_Y));
    ui.parameter_value_groupBox->setGeometry(QRect(
        ui.cost_function_groupBox->geometry().right() +
            GROUP_BOX_TO_GROUP_BOX_X,
        ui.stage_specific_groupBox->geometry().top(),
        ui.cost_function_groupBox->geometry().width(),
        ui.stage_specific_groupBox->geometry().height()));
    ui.cost_function_parameters_groupBox->setGeometry(QRect(
        ui.cost_function_groupBox->geometry().right() +
            GROUP_BOX_TO_GROUP_BOX_X,
        ui.cost_function_groupBox->geometry().top(),
        ui.cost_function_groupBox->geometry().width(),
        ui.range_groupBox->geometry().bottom() -
            ui.cost_function_groupBox->geometry().top() + 1));
    ui.cost_function_parameters_listWidget->setGeometry(QRect(
        GROUP_BOX_TO_SMALL_GROUP_BOX_X,
        group_box_to_top_button_y + GROUP_BOX_TO_SMALL_GROUP_BOX_Y,
        ui.cost_function_parameters_groupBox->geometry().width() -
            2 * GROUP_BOX_TO_SMALL_GROUP_BOX_X,
        ui.cost_function_parameters_groupBox->geometry().height() -
            (group_box_to_top_button_y + 2 * GROUP_BOX_TO_SMALL_GROUP_BOX_Y)));

    /*Set Geometry of General Options*/
    ui.general_options_groupBox->setGeometry(QRect(
        APPLICATION_BORDER_TO_GROUP_BOX_PADDING_X,
        100,
        ui.parameter_value_groupBox->geometry().right() +
            GROUP_BOX_TO_SMALL_GROUP_BOX_X,
        ui.parameter_value_groupBox->geometry().bottom() +
            GROUP_BOX_TO_GROUP_BOX_Y));

    /*Move Search Stage Box in Both Direction*/
    ui.optimization_search_stage_groupBox->move(QPoint(
        ((ui.general_options_groupBox->geometry().width() -
          ui.optimization_search_stage_groupBox->geometry().width()) /
         2) +
            ui.general_options_groupBox->geometry().left(),
        APPLICATION_BORDER_TO_GROUP_BOX_PADDING_Y + group_box_to_top_button_y));

    /*Move General Options Vertically*/
    ui.general_options_groupBox->move(QPoint(
        ui.general_options_groupBox->geometry().left(),
        ui.optimization_search_stage_groupBox->geometry().bottom() +
            GROUP_BOX_TO_GROUP_BOX_Y));

    /*Set Geometry of Bottom Three Buttons*/
    int opt_settings_button_width = INSIDE_BUTTON_PADDING_X +
        std::max(std::max(
                     font_metrics.horizontalAdvance(ui.save_button->text()),
                     font_metrics.horizontalAdvance(ui.reset_button->text())),
                 font_metrics.horizontalAdvance(ui.cancel_button->text()));
    ui.save_button->setGeometry(QRect(
        1 +
            ((ui.general_options_groupBox->geometry().right() +
              APPLICATION_BORDER_TO_GROUP_BOX_PADDING_X) -
             (3 * opt_settings_button_width + 2 * BUTTON_TO_BUTTON_PADDING_X)) /
                2,
        ui.general_options_groupBox->geometry().bottom() +
            GROUP_BOX_TO_GROUP_BOX_Y,
        opt_settings_button_width,
        font_metrics.height() + INSIDE_BUTTON_PADDING_Y));
    ui.reset_button->setGeometry(QRect(
        ui.save_button->geometry().right() + BUTTON_TO_BUTTON_PADDING_X,
        ui.general_options_groupBox->geometry().bottom() +
            GROUP_BOX_TO_GROUP_BOX_Y,
        opt_settings_button_width,
        font_metrics.height() + INSIDE_BUTTON_PADDING_Y));
    ui.cancel_button->setGeometry(QRect(
        ui.reset_button->geometry().right() + BUTTON_TO_BUTTON_PADDING_X,
        ui.general_options_groupBox->geometry().bottom() +
            GROUP_BOX_TO_GROUP_BOX_Y,
        opt_settings_button_width,
        font_metrics.height() + INSIDE_BUTTON_PADDING_Y));

    /*Set Width and Height of Window*/
    setFixedSize(
        ui.general_options_groupBox->geometry().right() +
            APPLICATION_BORDER_TO_GROUP_BOX_PADDING_X,
        ui.reset_button->geometry().bottom() + GROUP_BOX_TO_GROUP_BOX_Y);
}

SettingsControl::~SettingsControl() {}

/*Load Optimizer Settings from Main Window*/
void SettingsControl::LoadSettings(
    jta_cost_function::CostFunctionManager sc_trunk_manager,
    jta_cost_function::CostFunctionManager sc_branch_manager,
    jta_cost_function::CostFunctionManager sc_leaf_manager,
    OptimizerSettings opt_settings) {
    /*Load CFMs locally*/
    sc_trunk_manager_ = sc_trunk_manager;
    sc_branch_manager_ = sc_branch_manager;
    sc_leaf_manager_ = sc_leaf_manager;

    /*Load Optimizer Settings locally*/
    opt_settings_ = opt_settings;

    /*Get Rid of Parameters Index and Hide the Value View*/
    ui.cost_function_parameters_listWidget->clearSelection();
    ui.double_parameter_spinBox->setEnabled(false);
    ui.double_parameter_spinBox->setVisible(false);
    ui.int_parameter_spinBox->setEnabled(false);
    ui.int_parameter_spinBox->setVisible(false);
    ui.bool_parameter_true_radioButton->setEnabled(false);
    ui.bool_parameter_true_radioButton->setVisible(false);
    ui.bool_parameter_false_radioButton->setEnabled(false);
    ui.bool_parameter_false_radioButton->setVisible(false);

    ui.trunk_radioButton->setChecked(true);  // Always load to trunk

    /*Load Optimizer Settings (non-cost function)*/
    ui.stage_enabled_checkBox->setChecked(true);  // ALWAYS TRUE FOR TRUNK
    ui.budget_spinBox->setValue(opt_settings_.trunk_budget);
    ui.x_translation_spinBox->setValue(opt_settings_.trunk_range.x);
    ui.y_translation_spinBox->setValue(opt_settings_.trunk_range.y);
    ui.z_translation_spinBox->setValue(opt_settings_.trunk_range.z);
    ui.x_rotation_spinBox->setValue(opt_settings_.trunk_range.xa);
    ui.y_rotation_spinBox->setValue(opt_settings_.trunk_range.ya);
    ui.z_rotation_spinBox->setValue(opt_settings_.trunk_range.za);

    /*Disable Branch options*/
    ui.branch_total_count_label->setVisible(false);
    ui.branch_count_spinBox->setVisible(false);
    ui.branch_count_spinBox->setEnabled(false);
    ui.branch_count_spinBox->setValue(opt_settings_.number_branches);

    /*Select Active Cost Function*/
    const QString active =
        costFunctionTypeToQString(sc_trunk_manager_.getActiveCostFunction());
    const auto matching_items =
        ui.cost_function_listWidget->findItems(active, Qt::MatchExactly);
    if (matching_items.empty()) {
        QMessageBox::critical(
            this, "Error!", "Active cost function not found!", QMessageBox::Ok);
    } else {
        ui.cost_function_listWidget->setCurrentItem(matching_items.front());
    }

    /*Select the first Parameter if There are any Parameters*/
    if (ui.cost_function_parameters_listWidget->count() > 0) {
        ui.cost_function_parameters_listWidget->setCurrentRow(0);
    }
}

/*On List Widgets Changed*/
/*Cost Function Selection*/
void SettingsControl::on_cost_function_listWidget_itemSelectionChanged() {
    /*Clear Parameter List*/
    ui.cost_function_parameters_listWidget->clear();
    ui.cost_function_parameters_listWidget->clearSelection();

    /*Get Rid of Parameters Index and Hide the Value View*/
    ui.double_parameter_spinBox->setEnabled(false);
    ui.double_parameter_spinBox->setVisible(false);
    ui.int_parameter_spinBox->setEnabled(false);
    ui.int_parameter_spinBox->setVisible(false);
    ui.bool_parameter_true_radioButton->setEnabled(false);
    ui.bool_parameter_true_radioButton->setVisible(false);
    ui.bool_parameter_false_radioButton->setEnabled(false);
    ui.bool_parameter_false_radioButton->setVisible(false);

    auto* selected_item = ui.cost_function_listWidget->currentItem();
    if (!selected_item) {
        return;
    }

    jta_cost_function::CostFunctionManager* manager = nullptr;
    if (ui.trunk_radioButton->isChecked()) {
        manager = &sc_trunk_manager_;
    } else if (ui.branch_radioButton->isChecked()) {
        manager = &sc_branch_manager_;
    } else {
        manager = &sc_leaf_manager_;
    }

    const auto type =
        cost_function_type_from_string(selected_item->text().toStdString());
    if (!type) {
        QMessageBox::critical(
            this, "Error!", "Unknown cost function!", QMessageBox::Ok);
        return;
    }

    manager->setActiveCostFunction(*type);
    auto* cost_function = manager->getActiveCostFunctionClass();

    const auto append_parameters = [this](auto& parameters) {
        for (auto& parameter : parameters) {
            ui.cost_function_parameters_listWidget->addItem(
                QString::fromStdString(parameter.getParameterName()));
        }
    };

    auto double_parameters = cost_function->getDoubleParameters();
    auto int_parameters = cost_function->getIntParameters();
    auto bool_parameters = cost_function->getBoolParameters();

    append_parameters(double_parameters);
    append_parameters(int_parameters);
    append_parameters(bool_parameters);

    /*Select the first Parameter if There are any Parameters*/
    if (ui.cost_function_parameters_listWidget->count() > 0) {
        ui.cost_function_parameters_listWidget->setCurrentRow(0);
    }
}

void SettingsControl::
    on_cost_function_parameters_listWidget_itemSelectionChanged() {
    /*Get Rid of Parameters Index and Hide the Value View*/
    ui.double_parameter_spinBox->setEnabled(false);
    ui.double_parameter_spinBox->setVisible(false);
    ui.int_parameter_spinBox->setEnabled(false);
    ui.int_parameter_spinBox->setVisible(false);
    ui.bool_parameter_true_radioButton->setEnabled(false);
    ui.bool_parameter_true_radioButton->setVisible(false);
    ui.bool_parameter_false_radioButton->setEnabled(false);
    ui.bool_parameter_false_radioButton->setVisible(false);

    auto* selected_item = ui.cost_function_parameters_listWidget->currentItem();
    if (!selected_item) {
        return;
    }

    jta_cost_function::CostFunctionManager* manager = nullptr;
    if (ui.trunk_radioButton->isChecked()) {
        manager = &sc_trunk_manager_;
    } else if (ui.branch_radioButton->isChecked()) {
        manager = &sc_branch_manager_;
    } else {
        manager = &sc_leaf_manager_;
    }

    const QString parameter_name = selected_item->text();
    auto* cost_function = manager->getActiveCostFunctionClass();

    auto double_parameters = cost_function->getDoubleParameters();
    for (auto& parameter : double_parameters) {
        if (QString::fromStdString(parameter.getParameterName()) ==
            parameter_name) {
            ui.double_parameter_spinBox->setVisible(true);
            ui.double_parameter_spinBox->setEnabled(true);
            ui.double_parameter_spinBox->setValue(
                parameter.getParameterValue());
            return;
        }
    }

    auto int_parameters = cost_function->getIntParameters();
    for (auto& parameter : int_parameters) {
        if (QString::fromStdString(parameter.getParameterName()) ==
            parameter_name) {
            ui.int_parameter_spinBox->setVisible(true);
            ui.int_parameter_spinBox->setEnabled(true);
            ui.int_parameter_spinBox->setValue(parameter.getParameterValue());
            return;
        }
    }

    auto bool_parameters = cost_function->getBoolParameters();
    for (auto& parameter : bool_parameters) {
        if (QString::fromStdString(parameter.getParameterName()) ==
            parameter_name) {
            ui.bool_parameter_true_radioButton->setEnabled(true);
            ui.bool_parameter_true_radioButton->setVisible(true);
            ui.bool_parameter_false_radioButton->setEnabled(true);
            ui.bool_parameter_false_radioButton->setVisible(true);
            ui.bool_parameter_true_radioButton->setChecked(
                parameter.getParameterValue());
            ui.bool_parameter_false_radioButton->setChecked(
                !parameter.getParameterValue());
            return;
        }
    }
}

/*Radio buttons for stage*/
void SettingsControl::on_trunk_radioButton_clicked() {
    /*Get Rid of Parameters Index and Hide the Value View*/
    ui.cost_function_parameters_listWidget->clearSelection();
    ui.double_parameter_spinBox->setEnabled(false);
    ui.double_parameter_spinBox->setVisible(false);
    ui.int_parameter_spinBox->setEnabled(false);
    ui.int_parameter_spinBox->setVisible(false);
    ui.bool_parameter_true_radioButton->setEnabled(false);
    ui.bool_parameter_true_radioButton->setVisible(false);
    ui.bool_parameter_false_radioButton->setEnabled(false);
    ui.bool_parameter_false_radioButton->setVisible(false);

    /*Load Optimizer Settings (non-cost function)*/
    ui.stage_enabled_checkBox->setChecked(true);  // ALWAYS TRUE FOR TRUNK
    ui.budget_spinBox->setValue(opt_settings_.trunk_budget);
    ui.x_translation_spinBox->setValue(opt_settings_.trunk_range.x);
    ui.y_translation_spinBox->setValue(opt_settings_.trunk_range.y);
    ui.z_translation_spinBox->setValue(opt_settings_.trunk_range.z);
    ui.x_rotation_spinBox->setValue(opt_settings_.trunk_range.xa);
    ui.y_rotation_spinBox->setValue(opt_settings_.trunk_range.ya);
    ui.z_rotation_spinBox->setValue(opt_settings_.trunk_range.za);

    /*Disable Branch options*/
    ui.branch_total_count_label->setVisible(false);
    ui.branch_count_spinBox->setVisible(false);
    ui.branch_count_spinBox->setEnabled(false);

    /*Select Active Cost Function*/
    const QString active =
        costFunctionTypeToQString(sc_trunk_manager_.getActiveCostFunction());
    const auto matching_items =
        ui.cost_function_listWidget->findItems(active, Qt::MatchExactly);
    if (matching_items.empty()) {
        QMessageBox::critical(
            this, "Error!", "Active cost function not found!", QMessageBox::Ok);
    } else {
        ui.cost_function_listWidget->setCurrentItem(matching_items.front());
    }

    if (ui.cost_function_parameters_listWidget->count() > 0) {
        ui.cost_function_parameters_listWidget->setCurrentRow(0);
    }
}

void SettingsControl::on_branch_radioButton_clicked() {
    /*Get Rid of Parameters Index and Hide the Value View*/
    ui.cost_function_parameters_listWidget->clearSelection();
    ui.double_parameter_spinBox->setEnabled(false);
    ui.double_parameter_spinBox->setVisible(false);
    ui.int_parameter_spinBox->setEnabled(false);
    ui.int_parameter_spinBox->setVisible(false);
    ui.bool_parameter_true_radioButton->setEnabled(false);
    ui.bool_parameter_true_radioButton->setVisible(false);
    ui.bool_parameter_false_radioButton->setEnabled(false);
    ui.bool_parameter_false_radioButton->setVisible(false);

    /*Load Optimizer Settings (non-cost function)*/
    ui.stage_enabled_checkBox->setChecked(opt_settings_.enable_branch_);
    ui.budget_spinBox->setValue(opt_settings_.branch_budget);
    ui.x_translation_spinBox->setValue(opt_settings_.branch_range.x);
    ui.y_translation_spinBox->setValue(opt_settings_.branch_range.y);
    ui.z_translation_spinBox->setValue(opt_settings_.branch_range.z);
    ui.x_rotation_spinBox->setValue(opt_settings_.branch_range.xa);
    ui.y_rotation_spinBox->setValue(opt_settings_.branch_range.ya);
    ui.z_rotation_spinBox->setValue(opt_settings_.branch_range.za);

    /*Enable Branch options*/
    ui.branch_total_count_label->setVisible(true);
    ui.branch_count_spinBox->setVisible(true);
    ui.branch_count_spinBox->setEnabled(true);

    /*Select Active Cost Function*/
    const QString active =
        costFunctionTypeToQString(sc_branch_manager_.getActiveCostFunction());
    const auto matching_items =
        ui.cost_function_listWidget->findItems(active, Qt::MatchExactly);
    if (matching_items.empty()) {
        QMessageBox::critical(
            this, "Error!", "Active cost function not found!", QMessageBox::Ok);
    } else {
        ui.cost_function_listWidget->setCurrentItem(matching_items.front());
    }

    if (ui.cost_function_parameters_listWidget->count() > 0) {
        ui.cost_function_parameters_listWidget->setCurrentRow(0);
    }
}

void SettingsControl::on_leaf_radioButton_clicked() {
    /*Get Rid of Parameters Index and Hide the Value View*/
    ui.cost_function_parameters_listWidget->clearSelection();
    ui.double_parameter_spinBox->setEnabled(false);
    ui.double_parameter_spinBox->setVisible(false);
    ui.int_parameter_spinBox->setEnabled(false);
    ui.int_parameter_spinBox->setVisible(false);
    ui.bool_parameter_true_radioButton->setEnabled(false);
    ui.bool_parameter_true_radioButton->setVisible(false);
    ui.bool_parameter_false_radioButton->setEnabled(false);
    ui.bool_parameter_false_radioButton->setVisible(false);

    /*Load Optimizer Settings (non-cost function)*/
    ui.stage_enabled_checkBox->setChecked(opt_settings_.enable_leaf_);
    ui.budget_spinBox->setValue(opt_settings_.leaf_budget);
    ui.x_translation_spinBox->setValue(opt_settings_.leaf_range.x);
    ui.y_translation_spinBox->setValue(opt_settings_.leaf_range.y);
    ui.z_translation_spinBox->setValue(opt_settings_.leaf_range.z);
    ui.x_rotation_spinBox->setValue(opt_settings_.leaf_range.xa);
    ui.y_rotation_spinBox->setValue(opt_settings_.leaf_range.ya);
    ui.z_rotation_spinBox->setValue(opt_settings_.leaf_range.za);

    /*Disable Branch options*/
    ui.branch_total_count_label->setVisible(false);
    ui.branch_count_spinBox->setVisible(false);
    ui.branch_count_spinBox->setEnabled(false);

    /*Select Active Cost Function*/
    const QString active =
        costFunctionTypeToQString(sc_leaf_manager_.getActiveCostFunction());
    const auto matching_items =
        ui.cost_function_listWidget->findItems(active, Qt::MatchExactly);
    if (matching_items.empty()) {
        QMessageBox::critical(
            this, "Error!", "Active cost function not found!", QMessageBox::Ok);
    } else {
        ui.cost_function_listWidget->setCurrentItem(matching_items.front());
    }

    if (ui.cost_function_parameters_listWidget->count() > 0) {
        ui.cost_function_parameters_listWidget->setCurrentRow(0);
    }
}

/*Optimizer Settings Buttons Toggled*/
void SettingsControl::on_stage_enabled_checkBox_clicked() {
    if (ui.trunk_radioButton->isChecked()) {
        ui.stage_enabled_checkBox->setChecked(true);
        QMessageBox::critical(
            this,
            "Not allowed!",
            "Optimizer must always have a trunk stage!",
            QMessageBox::Ok);
    } else if (ui.branch_radioButton->isChecked()) {
        opt_settings_.enable_branch_ = ui.stage_enabled_checkBox->isChecked();
    } else {
        opt_settings_.enable_leaf_ = ui.stage_enabled_checkBox->isChecked();
    }
};

void SettingsControl::on_budget_spinBox_valueChanged() {
    if (ui.trunk_radioButton->isChecked()) {
        opt_settings_.trunk_budget = ui.budget_spinBox->value();
    } else if (ui.branch_radioButton->isChecked()) {
        opt_settings_.branch_budget = ui.budget_spinBox->value();
    } else {
        opt_settings_.leaf_budget = ui.budget_spinBox->value();
    }
};

void SettingsControl::on_x_translation_spinBox_valueChanged() {
    if (ui.trunk_radioButton->isChecked()) {
        opt_settings_.trunk_range.x = ui.x_translation_spinBox->value();
    } else if (ui.branch_radioButton->isChecked()) {
        opt_settings_.branch_range.x = ui.x_translation_spinBox->value();
    } else {
        opt_settings_.leaf_range.x = ui.x_translation_spinBox->value();
    }
};

void SettingsControl::on_y_translation_spinBox_valueChanged() {
    if (ui.trunk_radioButton->isChecked()) {
        opt_settings_.trunk_range.y = ui.y_translation_spinBox->value();
    } else if (ui.branch_radioButton->isChecked()) {
        opt_settings_.branch_range.y = ui.y_translation_spinBox->value();
    } else {
        opt_settings_.leaf_range.y = ui.y_translation_spinBox->value();
    }
};

void SettingsControl::on_z_translation_spinBox_valueChanged() {
    if (ui.trunk_radioButton->isChecked()) {
        opt_settings_.trunk_range.z = ui.z_translation_spinBox->value();
    } else if (ui.branch_radioButton->isChecked()) {
        opt_settings_.branch_range.z = ui.z_translation_spinBox->value();
    } else {
        opt_settings_.leaf_range.z = ui.z_translation_spinBox->value();
    }
};

void SettingsControl::on_x_rotation_spinBox_valueChanged() {
    if (ui.trunk_radioButton->isChecked()) {
        opt_settings_.trunk_range.xa = ui.x_rotation_spinBox->value();
    } else if (ui.branch_radioButton->isChecked()) {
        opt_settings_.branch_range.xa = ui.x_rotation_spinBox->value();
    } else {
        opt_settings_.leaf_range.xa = ui.x_rotation_spinBox->value();
    }
};

void SettingsControl::on_y_rotation_spinBox_valueChanged() {
    if (ui.trunk_radioButton->isChecked()) {
        opt_settings_.trunk_range.ya = ui.y_rotation_spinBox->value();
    } else if (ui.branch_radioButton->isChecked()) {
        opt_settings_.branch_range.ya = ui.y_rotation_spinBox->value();
    } else {
        opt_settings_.leaf_range.ya = ui.y_rotation_spinBox->value();
    }
};

void SettingsControl::on_z_rotation_spinBox_valueChanged() {
    if (ui.trunk_radioButton->isChecked()) {
        opt_settings_.trunk_range.za = ui.z_rotation_spinBox->value();
    } else if (ui.branch_radioButton->isChecked()) {
        opt_settings_.branch_range.za = ui.z_rotation_spinBox->value();
    } else {
        opt_settings_.leaf_range.za = ui.z_rotation_spinBox->value();
    }
};

void SettingsControl::on_branch_count_spinBox_valueChanged() {
    if (ui.trunk_radioButton->isChecked()) {
    } else if (ui.branch_radioButton->isChecked()) {
        opt_settings_.number_branches = ui.branch_count_spinBox->value();
    } else {
    }
};

void SettingsControl::on_double_parameter_spinBox_valueChanged() {
    auto* selected_item = ui.cost_function_parameters_listWidget->currentItem();
    if (!selected_item) {
        return;
    }

    jta_cost_function::CostFunctionManager* manager = nullptr;
    if (ui.trunk_radioButton->isChecked()) {
        manager = &sc_trunk_manager_;
    } else if (ui.branch_radioButton->isChecked()) {
        manager = &sc_branch_manager_;
    } else {
        manager = &sc_leaf_manager_;
    }

    if (!manager->getActiveCostFunctionClass()->setDoubleParameterValue(
            selected_item->text().toStdString(),
            ui.double_parameter_spinBox->value())) {
        QMessageBox::critical(
            this,
            "Error!",
            "Could not update parameter value!",
            QMessageBox::Ok);
    }
}

void SettingsControl::on_int_parameter_spinBox_valueChanged() {
    auto* selected_item = ui.cost_function_parameters_listWidget->currentItem();
    if (!selected_item) {
        return;
    }

    jta_cost_function::CostFunctionManager* manager = nullptr;
    if (ui.trunk_radioButton->isChecked()) {
        manager = &sc_trunk_manager_;
    } else if (ui.branch_radioButton->isChecked()) {
        manager = &sc_branch_manager_;
    } else {
        manager = &sc_leaf_manager_;
    }

    if (!manager->getActiveCostFunctionClass()->setIntParameterValue(
            selected_item->text().toStdString(),
            ui.int_parameter_spinBox->value())) {
        QMessageBox::critical(
            this,
            "Error!",
            "Could not update parameter value!",
            QMessageBox::Ok);
    }
}

void SettingsControl::on_bool_parameter_true_radioButton_clicked() {
    auto* selected_item = ui.cost_function_parameters_listWidget->currentItem();
    if (!selected_item) {
        return;
    }

    jta_cost_function::CostFunctionManager* manager = nullptr;
    if (ui.trunk_radioButton->isChecked()) {
        manager = &sc_trunk_manager_;
    } else if (ui.branch_radioButton->isChecked()) {
        manager = &sc_branch_manager_;
    } else {
        manager = &sc_leaf_manager_;
    }

    if (!manager->getActiveCostFunctionClass()->setBoolParameterValue(
            selected_item->text().toStdString(),
            ui.bool_parameter_true_radioButton->isChecked())) {
        QMessageBox::critical(
            this,
            "Error!",
            "Could not update parameter value!",
            QMessageBox::Ok);
    }
}

void SettingsControl::on_bool_parameter_false_radioButton_clicked() {
    auto* selected_item = ui.cost_function_parameters_listWidget->currentItem();
    if (!selected_item) {
        return;
    }

    jta_cost_function::CostFunctionManager* manager = nullptr;
    if (ui.trunk_radioButton->isChecked()) {
        manager = &sc_trunk_manager_;
    } else if (ui.branch_radioButton->isChecked()) {
        manager = &sc_branch_manager_;
    } else {
        manager = &sc_leaf_manager_;
    }

    if (!manager->getActiveCostFunctionClass()->setBoolParameterValue(
            selected_item->text().toStdString(),
            ui.bool_parameter_true_radioButton->isChecked())) {
        QMessageBox::critical(
            this,
            "Error!",
            "Could not update parameter value!",
            QMessageBox::Ok);
    }
}

/*Save Button*/
void SettingsControl::on_save_button_clicked() {
    /*Emit Save Settings*/
    emit SaveSettings(
        opt_settings_, sc_trunk_manager_, sc_branch_manager_, sc_leaf_manager_);

    /*Close Window*/
    emit Done();
}

/*Reset Button*/
void SettingsControl::on_reset_button_clicked() {
    /*Reinitialize Optimizer Settings and 3 Cost Function Managers*/
    opt_settings_ = OptimizerSettings();
    sc_trunk_manager_ = jta_cost_function::CostFunctionManager(Stage::Trunk);
    sc_branch_manager_ = jta_cost_function::CostFunctionManager(Stage::Branch);
    sc_leaf_manager_ = jta_cost_function::CostFunctionManager(Stage::Leaf);

    /*Change the Default Settings of Dilation for branch and leaf to 4 and 1
     * respectively*/
    sc_branch_manager_.getCostFunctionClass(CostFunctionType::DirectDilation)
        ->setIntParameterValue("Dilation", 4);
    sc_leaf_manager_.getCostFunctionClass(CostFunctionType::DirectDilation)
        ->setIntParameterValue("Dilation", 1);

    /*Refresh Settings GUI*/
    ui.cost_function_parameters_listWidget->clear();
    ui.cost_function_parameters_listWidget->clearSelection();
    ui.double_parameter_spinBox->setEnabled(false);
    ui.double_parameter_spinBox->setVisible(false);
    ui.int_parameter_spinBox->setEnabled(false);
    ui.int_parameter_spinBox->setVisible(false);
    ui.bool_parameter_true_radioButton->setEnabled(false);
    ui.bool_parameter_true_radioButton->setVisible(false);
    ui.bool_parameter_false_radioButton->setEnabled(false);
    ui.bool_parameter_false_radioButton->setVisible(false);

    ui.trunk_radioButton->setChecked(true);  // Always load to trunk

    /*Load Optimizer Settings (non-cost function)*/
    ui.stage_enabled_checkBox->setChecked(true);  // ALWAYS TRUE FOR TRUNK
    ui.budget_spinBox->setValue(opt_settings_.trunk_budget);
    ui.x_translation_spinBox->setValue(opt_settings_.trunk_range.x);
    ui.y_translation_spinBox->setValue(opt_settings_.trunk_range.y);
    ui.z_translation_spinBox->setValue(opt_settings_.trunk_range.z);
    ui.x_rotation_spinBox->setValue(opt_settings_.trunk_range.xa);
    ui.y_rotation_spinBox->setValue(opt_settings_.trunk_range.ya);
    ui.z_rotation_spinBox->setValue(opt_settings_.trunk_range.za);

    /*Disable Branch options*/
    ui.branch_total_count_label->setVisible(false);
    ui.branch_count_spinBox->setVisible(false);
    ui.branch_count_spinBox->setEnabled(false);
    ui.branch_count_spinBox->setValue(opt_settings_.number_branches);

    /*Select Active Cost Function*/
    const QString active =
        costFunctionTypeToQString(sc_trunk_manager_.getActiveCostFunction());
    const auto matching_items =
        ui.cost_function_listWidget->findItems(active, Qt::MatchExactly);
    if (!matching_items.empty()) {
        ui.cost_function_listWidget->setCurrentItem(matching_items.front());
    }

    if (ui.cost_function_parameters_listWidget->count() > 0) {
        ui.cost_function_parameters_listWidget->setCurrentRow(0);
    }
}

/*Cancel Button*/
void SettingsControl::on_cancel_button_clicked() {
    /*Close Window*/
    emit Done();
}
