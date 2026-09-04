// Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
// SPDX-License-Identifier: AGPL-3.0-only OR MIT

// Plan 006 U5: OptimizerRunControllerCore implementation — see the header
// for the contract. Qt/GPU-free: no QObject, no event loop, no CUDA/torch.

#include "coordinator/optimizer_run_controller_core.h"

#include <algorithm>
#include <cmath>

namespace jta {

/*---- Gate (H2) ----*/

OptimizeIntentController::Input OptimizerRunControllerCore::BuildGateInput(
    const GateInput& in) {
    OptimizeIntentController::Input out;
    out.selected_model_rows = in.selected_model_rows;
    /*H2: previous == current — the gate never sees the raw session mirrors
     * (they feed save-last-pose only); this is exactly what the QML bridge's
     * buildGateInput did (previous_frame_index = currentFrame).*/
    out.previous_frame_index = in.current_frame;
    out.current_frame = in.current_frame;
    out.frame_count = in.frame_count;
    out.model_current_index = in.model_current_index;
    out.model_count = in.model_count;
    out.pose_frame_count = in.pose_frame_count;
    out.pose_model_count = in.pose_model_count;
    return out;
}

OptimizerRunControllerCore::GateResult OptimizerRunControllerCore::EvaluateGate(
    const GateInput& in) {
    GateResult result;
    result.intent = OptimizeIntentController::Evaluate(BuildGateInput(in));
    result.status = result.intent.status;
    return result;
}

/*---- Progress (oracle seam, M12) ----*/

std::string OptimizerRunControllerCore::StageLabel(
    const ProgressBudgets& b,
    int calls) {
    const int branch_budget =
        b.enable_branch ? std::max(1, b.branch_budget) : 1;
    const int branch_total =
        b.enable_branch ? b.number_branches * b.branch_budget : 0;
    if (calls < b.trunk_budget) {
        return "Trunk";
    }
    if (calls < b.trunk_budget + branch_total) {
        return "Branch " +
            std::to_string((calls - b.trunk_budget) / branch_budget + 1);
    }
    if (calls <
        b.trunk_budget + branch_total + (b.enable_leaf ? b.leaf_budget : 0)) {
        return "Extra Z-Translation";
    }
    return "Finished";
}

void OptimizerRunControllerCore::refreshProgress(
    const ProgressBudgets& b,
    int calls,
    double minimum) {
    cost_calls_ = calls;
    current_minimum_ = minimum;
    const int cumulative = b.trunk_budget +
        (b.enable_branch ? b.number_branches * b.branch_budget : 0) +
        (b.enable_leaf ? b.leaf_budget : 0);
    stage_text_ = StageLabel(b, calls);
    progress_ =
        cumulative > 0 ? std::min(1.0, calls / double(cumulative)) : 0.0;
}
}  // namespace jta
