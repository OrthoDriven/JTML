/*
 * Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
 * SPDX-License-Identifier: AGPL-3.0-only OR MIT
 */

#pragma once

/*Cost Function Parameters*/
#include "Parameter.h"
#include "domain/preprocessor-defs.h"

/*Standard Library*/
#include <optional>
#include <string>
#include <vector>

namespace jta_cost_function {

#include <string_view>

#define COST_FUNCTION_TYPE_LIST(X)     \
    X(DirectDilation)                  \
    X(SymmetryTrap)                    \
    X(DirectDilationNewPoleConstraint) \
    X(DirectDilationOldPoleConstraint) \
    X(DirectDilationConstrainZ)        \
    X(DirectDilationOldT1)             \
    X(DirectDilationMahfouzVariant)

enum class CostFunctionType : unsigned char {
#define X(name) name,
    COST_FUNCTION_TYPE_LIST(X)
#undef X
};

constexpr std::string_view to_string(CostFunctionType type) {
    switch (type) {
#define X(name)                  \
    case CostFunctionType::name: \
        return #name;
        COST_FUNCTION_TYPE_LIST(X)
#undef X
    }
    return "Unknown";
}

constexpr std::optional<CostFunctionType> cost_function_type_from_string(
    std::string_view name) {
#define X(enum_name)                        \
    if (name == #enum_name) {               \
        return CostFunctionType::enum_name; \
    }
    COST_FUNCTION_TYPE_LIST(X)
#undef X
    return std::nullopt;
}

#undef COST_FUNCTION_TYPE_LIST

class CostFunction {
public:
    /*Constructor*/
    JTML_DLL CostFunction();
    JTML_DLL CostFunction(std::string cost_function_name);
    JTML_DLL ~CostFunction();

    /*Add Parameter (w/ Default Value)*/
    JTML_DLL void addParameter(Parameter<double> new_parameter);
    JTML_DLL void addParameter(Parameter<int> new_parameter);
    JTML_DLL void addParameter(Parameter<bool> new_parameter);

    /*Set Parameter Values (Bool for Success)*/
    JTML_DLL bool setDoubleParameterValue(
        std::string parameter_name,
        double value);
    JTML_DLL bool setIntParameterValue(std::string parameter_name, int value);
    JTML_DLL bool setBoolParameterValue(std::string parameter_name, bool value);

    /*Get Parameter Values (Bool for Success)*/
    JTML_DLL bool getDoubleParameterValue(
        std::string parameter_name,
        double& value);
    JTML_DLL bool getIntParameterValue(std::string parameter_name, int& value);
    JTML_DLL bool getBoolParameterValue(
        std::string parameter_name,
        bool& value);

    /*Get Parameters by Type Groups*/
    JTML_DLL std::vector<Parameter<double>> getDoubleParameters();
    JTML_DLL std::vector<Parameter<int>> getIntParameters();
    JTML_DLL std::vector<Parameter<bool>> getBoolParameters();

    /*Get/Set Cost Function Name*/
    JTML_DLL std::string getCostFunctionName();
    JTML_DLL void setCostFunctionName(std::string cost_function_name);

private:
    /*Containers for Parameters*/
    std::vector<Parameter<double>> double_parameters_;
    std::vector<Parameter<int>> int_parameters_;
    std::vector<Parameter<bool>> bool_parameters_;

    /*Cost Function Name*/
    std::string cost_function_name_;
};
}  // namespace jta_cost_function
