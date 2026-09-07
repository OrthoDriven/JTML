# Copyright 2023 Gary J. Miller Orthopaedic Biomechanics Lab
# SPDX-License-Identifier: AGPL-3.0-only OR MIT

set(CMAKE_AUTOMOC ON)

file(GLOB HEADER_FILES CONFIGURE_DEPENDS
    ${PROJECT_SOURCE_DIR}/include/coordinator/*.h
)

add_library(jtml_coordinator STATIC
    optimizer_manager.cpp
    optimizer_run_controller_core.cpp
    optimizer_run_driver.cpp
    optimizer_stage_script.cpp
    optimizer_run_controller.cpp
    session_state_controller.cpp
    ${HEADER_FILES}
)
option(USING_CPP_COST "Use CppCost instead of CostFunction std::function" 0)

target_compile_definitions(jtml_domain PRIVATE
    $<$<BOOL:${USING_CPP_COST}>:USING_CPP_COST>
)


target_compile_definitions(jtml_coordinator PRIVATE
    $<$<BOOL:${USING_CPP_COST}>:USING_CPP_COST>
)

# Downward deps: services (-> domain) + the single SHARED compute lib
# (optimizer_manager uses gpu_*.cuh + CostFunctionManager.h => jtml_compute, merged U4).
target_link_libraries(jtml_coordinator PUBLIC jtml_services ${JTA_LIBS})
# Qt + VTK (via optimizer_manager/services), Torch + OpenCV + CUDA (compute).
target_link_libraries(jtml_coordinator PRIVATE
    ${REQUIRED_LIBS_QUALIFIED}
    ${VTK_LIBRARIES}
    ${TORCH_LIBRARIES}
    ${OpenCV_LIBRARIES}
    ${CUDA_LIBRARIES}
)

# Adding Include Directories. include/compute resolves the bare/resolver gpu/cost
# headers (gpu_*.cuh, pose_matrix.h, camera_calibration.h) in optimizer_manager + the
# cost managers; CUDA includes for the compute path.
target_include_directories(jtml_coordinator PUBLIC
    ${PROJECT_SOURCE_DIR}/include
    ${PROJECT_SOURCE_DIR}/include/compute
    ${CUDA_INCLUDE_DIRS}
)

# The compute + GPU/CUDA surface now lives in the jtml_compute SHARED lib, linked
# through ${JTA_LIBS}; no per-target CUDA flags or separable compilation are needed
# here (that is jtml_compute only).

target_link_options(jtml_coordinator PRIVATE
    "-Wl,--no-undefined"
)
