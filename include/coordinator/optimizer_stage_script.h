/*
 * Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
 * SPDX-License-Identifier: AGPL-3.0-only OR MIT
 */

#ifndef OPTIMIZER_STAGE_SCRIPT_H
#define OPTIMIZER_STAGE_SCRIPT_H

#include <string>
#include <vector>

#include "CostFunction.h"
#include "compute/Parameter.h"
#include "compute/objective_spec.h"
#include "domain/data_structures_6D.h"
#include "domain/settings_constants.h"
#include "services/optimizer_settings.h"

namespace jta {

/*Stage kind — the lineage paper's stage taxonomy is exactly Trunk/Branch/Leaf
 * (Flood & Banks 2018, IEEE TMI 37(1):326-335; angle 04 R3-1); a future polish
 * kind is a schema note only (R5 — the registry's reserved stubs).*/
enum class StageKind : unsigned char { Trunk = 0, Branch = 1, Leaf = 2 };

struct StageSpec {
    StageKind kind = StageKind::Trunk;
    Point6D range;
    unsigned int budget = 0;
    unsigned int repeat = 0;
    unsigned int cfm_index = 0;  // TODO: Legacy, remove eventually
    jta_cost_function::ObjectiveSpec obj_spec =
        jta_cost_function::DirectDilationSpec{.dilation = 6};
};

using StageScript = std::vector<StageSpec>;

struct StageCostParams {
    int dilation = 0;
    bool dark_silhouette = false;
};

StageCostParams DeriveStageCostParams(
    const jta_cost_function::CostFunctionType cost_function_type,
    std::vector<jta_cost_function::Parameter<int>> int_params,
    std::vector<jta_cost_function::Parameter<bool>> bool_params);

StageScript BuildStageScript(
    const OptimizerSettings& settings,
    const std::string& directive);

const StageScript jtml_production = {
    StageSpec{
        .kind = StageKind::Trunk,
        .range = Point6D(50, 50, 50, 50, 50, 50),
        .budget = 10000,
        .repeat = 0,
        .cfm_index = 0,
        .obj_spec = jta_cost_function::DirectDilationSpec{.dilation = 6}},
    StageSpec{
        .kind = StageKind::Branch,
        .range = Point6D(25, 25, 25, 25, 25, 25),
        .budget = 5000,
        .repeat = 0,
        .cfm_index = 1,
        .obj_spec = jta_cost_function::DirectDilationSpec{.dilation = 3}},
    StageSpec{
        .kind = StageKind::Leaf,
        .range = Point6D(5, 5, 100, 5, 5, 5),
        .budget = 5000,
        .repeat = 0,
        .cfm_index = 2,
        .obj_spec = jta_cost_function::DirectDilationSpec{.dilation = 1}},
};
/*The cumulative budget caps the run lands on, one entry per search run
 * (RunDirectStage invocation), transcribing budget_ = trunk_budget at the
 * trunk and budget_ += stage budget per repeat thereafter. This is the U6
 * stage-bookkeeping gate (costCalls on 20/25/30/35k for jtml-production).
 * repeat=0 (the Sym_Trap no-search leaf) contributes nothing — the caps are
 * empty and costCalls stays 0 (the U6 sym-trap pin).*/
std::vector<unsigned int> CumulativeStageCaps(const StageScript& script);

}  // namespace jta

#endif /* OPTIMIZER_STAGE_SCRIPT_H */
