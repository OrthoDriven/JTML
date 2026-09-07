// Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
// SPDX-License-Identifier: AGPL-3.0-only OR MIT

// Plan 006 U5: OptimizerRunDriver — the narrow drive seam behind
// OptimizerRunController (M12/Q8). OptimizerManager::Initialize is
// NON-VIRTUAL, so a test subclass cannot intercept it; the seam is the
// driver, not the manager. The controller depends on this interface (a
// fresh driver per run, obtained from an injected factory); a production
// adapter (optimizer_run_driver.cpp) wraps OptimizerManager UNTOUCHED.
// Tests implement the interface and emit the manager's 7+1 signals,
// recording Initialize args + bind order. This preserves the "NOT touching
// OptimizerManager internals" boundary and is the multi-stage oracle's
// future entry point.
//
// Thread ownership lives in the driver: the production adapter creates a
// fresh OptimizerManager + QThread per instance and implements the
// destructor contract (H3 — never delete a running thread; cooperative
// stop -> quit + bounded wait, warn + keep waiting on expiry).

#ifndef OPTIMIZER_RUN_DRIVER_H
#define OPTIMIZER_RUN_DRIVER_H

#include <QModelIndexList>
#include <QObject>
#include <QString>
#include <memory>

#include "coordinator/optimizer_manager.h"

namespace jta {

struct OptimizerRunLaunch {
    Calibration calibration;
    std::vector<Frame> camera_a_frames;
    std::vector<Frame> camera_b_frames;
    unsigned int current_frame_index = 0;
    std::vector<Model> models;
    QModelIndexList selected_model_indexes;
    unsigned int primary_model_index = 0;
    LocationStorage pose_matrix;
    OptimizerSettings settings;
    QString directive;
    int iter_count = 0;
};

/*The drive seam. A fresh instance per run; the controller binds the 8
 * connects (finished FIRST — M6 — then the 7, L13) against Manager() before
 * calling Initialize.*/
class OptimizerRunDriver {
public:
    virtual ~OptimizerRunDriver() = default;

    virtual QObject* Manager() = 0;

    virtual bool ThreadActive() const = 0;

    virtual bool Initialize(
        const OptimizerRunLaunch& launch,
        QString& error_message) = 0;

    virtual void Start() = 0;
    virtual void Stop() = 0;
    virtual void Wait() = 0;
};

/*Production factory: a fresh OptimizerManagerRunDriver (new manager + new
 * QThread, moveToThread applied). Shared ownership: the controller releases
 * a finished run's driver at the next start(), while a finished driver's
 * connections stay alive until then (the epoch + sender guard drops any
 * straggler relays — H1).*/
std::shared_ptr<OptimizerRunDriver> CreateOptimizerManagerRunDriver();

}  // namespace jta

#endif /* OPTIMIZER_RUN_DRIVER_H */
