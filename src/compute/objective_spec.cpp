#include "compute/objective_spec.h"

namespace jta_cost_function {

std::optional<int> getDilation(const ObjectiveSpec& spec) {
    return std::visit(
        [](const auto& objective) -> std::optional<int> {
            if constexpr (requires { objective.dilation; }) {
                return objective.dilation;
            } else {
                return std::nullopt;
            }
        },
        spec);
}
}  // namespace jta_cost_function
