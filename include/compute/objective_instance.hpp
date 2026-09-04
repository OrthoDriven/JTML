#ifndef OBJECTIVE_INSTANCE_H_
#define OBJECTIVE_INSTANCE_H_

#include "render_engine.cuh"

class ObjectiveInstance {
public:
    virtual ~ObjectiveInstance() = default;
    virtual bool initialize(std::string& error_message) = 0;
    virtual double evaluate(const gpu_cost_function::Pose& pose) = 0;
};

#endif  // OBJECTIVE_INSTANCE_H_
