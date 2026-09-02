/*
 * Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
 * SPDX-License-Identifier: AGPL-3.0-only OR MIT
 */

/*
 * Cost-function registry mapping (plan 006 U1 / R8): the raw
 * CostFunctionSettings entries builder shared by the widgets app (MainScreen)
 * and the QML app (SettingsBridge). Relocated verbatim from
 * MainScreen::BuildCostFunctionRegistryEntries (mainscreen.cpp:4895, plan
 * 004 U3) — the body below is byte-identical to the pre-extraction mapping
 * (R13); the golden 51-entry fixture (test/unit/cost_function_registry_test
 * .cpp + test/unit/experimental_settings_test.cpp) pins it. The module is a
 * compute-linked sibling of the QtCore-only SettingsService: it reads the
 * GPU-linked CostFunctionManager (jtml_services links jtml_compute PRIVATE).
 */

#include "services/cost_function_registry.h"

#include "compute/CostFunctionManager.h"

namespace jta {

std::vector<RegistryEntry> BuildCostFunctionRegistryEntries(
    jta_cost_function::CostFunctionManager& trunk_manager,
    jta_cost_function::CostFunctionManager& branch_manager,
    jta_cost_function::CostFunctionManager& leaf_manager) {
    std::vector<RegistryEntry> entries;

    auto append_manager_entries =
        [&entries](
            QString prefix, jta_cost_function::CostFunctionManager& manager) {
            entries.push_back(
                RegistryEntry{
                    prefix + QStringLiteral("@ACTIVE_CF"),
                    QString::fromStdString(
                        std::string(
                            to_string(manager.getActiveCostFunction())))});

            for (auto& cost_function_entry :
                 manager.getAvailableCostFunctions()) {
                auto& cost_function = cost_function_entry.second;

                auto append_parameters = [&entries, &prefix, &cost_function](
                                             auto& parameters) {
                    for (auto& parameter : parameters) {
                        entries.push_back(
                            RegistryEntry{
                                prefix + QStringLiteral("@") +
                                    QString::fromStdString(
                                        cost_function.getCostFunctionName()) +
                                    QStringLiteral("@") +
                                    QString::fromStdString(
                                        parameter.getParameterName()) +
                                    QStringLiteral("@") +
                                    QString::fromStdString(
                                        parameter.getParameterType()),
                                parameter.getParameterValue()});
                    }
                };

                auto double_parameters = cost_function.getDoubleParameters();
                auto int_parameters = cost_function.getIntParameters();
                auto bool_parameters = cost_function.getBoolParameters();

                append_parameters(double_parameters);
                append_parameters(int_parameters);
                append_parameters(bool_parameters);
            }
        };

    append_manager_entries(QStringLiteral("TRUNK"), trunk_manager);
    append_manager_entries(QStringLiteral("BRANCH"), branch_manager);
    append_manager_entries(QStringLiteral("LEAF"), leaf_manager);

    return entries;
}

} /* namespace jta */
