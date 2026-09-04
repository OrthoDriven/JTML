#ifndef OBJECTIVE_SPEC_H_
#define OBJECTIVE_SPEC_H_

#include <optional>
#include <variant>

namespace jta_cost_function {

struct DirectDilationSpec {
    int dilation = 6;
};

struct SymmetryTrapSpec {
    int dilation = 3;
    double pole_weight = 75.0;
    double vv_weight = 500.0;
};

struct DirectDilationNewPoleConstraintSpec {
    int dilation = 3;
    double pole_weight = 75.0;
    bool x_trans = false;
    bool y_trans = false;
    bool z_trans = false;
};

struct DirectDilationOldPoleConstraintSpec {
    int dilation = 6;
    double pole_weight = 75.0;
};

struct DirectDilationConstrainZSpec {
    int dilation = 6;
    double z_weight = 1.0;
};

struct DirectDilationOldT1Spec {
    int dilation = 6;
};

struct DirectDilationMahfouzVariantSpec {
    bool black_silhouette = true;
};

using ObjectiveSpec = std::variant<
    DirectDilationSpec,
    SymmetryTrapSpec,
    DirectDilationNewPoleConstraintSpec,
    DirectDilationOldPoleConstraintSpec,
    DirectDilationConstrainZSpec,
    DirectDilationOldT1Spec,
    DirectDilationMahfouzVariantSpec>;

// objective_spec.h
std::optional<int> getDilation(const ObjectiveSpec& spec);
}  // namespace jta_cost_function

#endif  // OBJECTIVE_SPEC_H_
