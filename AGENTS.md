# user tools
 │ C++ / CUDA: readseek mis-detects .h as C and .cu/.cuh as unknown. For .h, .cu, .cuh always pass
 │ language: "cpp" on digest/edit/grep/search/refs/def calls (.cpp/.hpp/.hh/.hxx are already correct).

# JTML — Agent Working Guide

JTML is a Qt6 (qt6-main/wayland 6.11.^) + VTK 9.7.^ built against Qt6 + CUDA 13.^ + OpenCV C++20 desktop app for 2D-3D knee-implant
registration (DIRECT global optimizer over a GPU cost function). This file captures the
conventions a coding agent needs to work here without re-deriving them.

## Build & environment (pixi)

Everything build-related is handled by pixi tasks — never invoke cmake/make/nvcc directly.

- `pixi run configure` — cmake configure (pulls deps; builds VTK once via `vtk_installer.sh`)
- `pixi run build` — build all targets (incl. the GUI + tests)
- `pixi run test` — headless suite runner (`ctest -L headless --timeout 600`). NOTE (2026-08-28): the suite is not currently built or run — see Test suite below.
- `pixi run run` — launch the GUI app
- `pixi add <pkg>` / `pixi run tidy` / `pixi run format` — deps / clang-tidy / clang-format

Add any new C++ test framework/tool to `pixi.toml` (the lockfile is `pixi.lock`).

## Version control is `jj` (Jujutsu), NOT git

- Always use `jj` commands. Never raw git.
- The repo owner's workflow is **`jj describe -m "<scope>: <msg>"` then `jj new`** for each
  logical change. `jj new` starts the next change after `describe`.
- `jj st` to see the working-copy change; `jj log --no-graph` to read history.

## Test suite (STATUS 2026-08-28: not built, not run, mostly legacy)

The `test/` tree (75 `.cpp` files under `test/unit`, `test/lifecycle`, `test/oracle`,
`test/qml`) is **not currently built or run** — the existing `.build` registers 0 tests.
Most of it characterizes the legacy state (CFM ambient execution, coordinator relays, the
removed CUDA-graph stack — e.g. `test_bank_binding_api.cpp` references deleted APIs and
will not compile). Do NOT treat it as a safety net, a gate, or a design authority; do not
"fix the tests" for legacy behavior. New tests are written per new seam as it lands (e.g.
`ObjectiveInstance` parity via targeted A/B probes).

Layout (for reference only):
- `test/unit/` — Catch2 logic tests (mixed: some pin legacy CFM behavior; pure-logic ones
  like the `StageScript`/`DeriveStageCostParams` transcription pins remain valid as
  *characterization* of legacy behavior, not as correctness).
- `test/golden/` — captured data files (`baseline.json`, `fem_golden.jts`,
  `fem_oracle_captured.jtak`, `calibration.txt`, probe artifacts). Legacy captures.
- `test/oracle/` — legacy GPU oracle tests (appearance/bit-identity). Not built or run.
- `test/lifecycle/`, `test/qml/` — QtTest/Qt-Quick-Test seams over the coordinator and QML
  bridges. Not currently run.


Conventions (for new tests):
- **QtTest for Qt/threading seams; Catch2 for pure math.** Both register via CTest.
- **Prefer hegel property-based tests for extracted pure logic.** When a piece of pure,
  CUDA/Qt-free logic has invariants worth locking down (length preservation,
  collision-freedom, monotonicity, determinism), add a hegel PBT test alongside its
  deterministic Catch2 unit test — PBT complements, never replaces, the deterministic cases.
  See `test/HEGEL-PBT-GUIDE.md` for the authoring patterns, built-in generator survey, and
  how to discover the hegel API.
- **New Qt test target gotcha:** CMake AUTOMOC does not auto-moc an included shared header,
  so add the Q_OBJECT header to the `add_executable(...)` source list (see
  `jtml_test_coordinator` in `test/CMakeLists.txt`).
- `QSignalSpy` must observe a signal on the **main/test thread**, never the worker thread
  (QTBUG-2842) — the coordinator re-emits on its own thread.

## Where to find information

`docs/` holds historical handoffs (`docs/handoff-*.md`), review records (`docs/reviews/`),
and the documented-solutions knowledge store (`docs/solutions/` — bugs, best practices,
and architecture patterns organized by category with YAML frontmatter
`module`/`tags`/`problem_type`). Search `docs/solutions/` before implementing or debugging
in a documented area; the current refactor playbook is
`docs/solutions/architecture-patterns/explicit-inputs-peeling-stateless-objectives.md`.
The plan/requirements trees (`docs/plans/`, `docs/brainstorms/`) were deleted 2026-08-28 —
do not chase references to them; they described the superseded additive-layering approach.
Fastest way over all of `docs/`: run `ctx_index` once per session, then `ctx_search` for
focused snippets.

## Current work (2026-08-28 — graph stack removed; objectives peel)

- **CUDA graphs / evaluation_context / evaluation_executor / graph_recipe / bank_state** were removed in `cleanup: removing a TON of old, useless files` (2026-08-28). Do not reintroduce them; the host-visible graph executor approach was measured as host-bound (`test/golden/probe_measurement.md`, `test/golden/graph_performance_baseline.json: reverted`).
- **Current hot-path focus:** `docs/jtml_cuda_d2h_hotpath_notes.org` — single-pose DIRECT_DILATION cost: ~16 kernel launches, ~5 D2H copies, 32 bytes payload; priority is D2H sync / launch overhead / kernel fusion before any multi-pose batching.
- **Current direction (2026-08-28):** explicit-inputs peeling — `ObjectiveInstance` / `DirectDilationObjective` (`src/objectives/`, `include/compute/objective_instance.hpp`) is the pure `evaluate(pose)` path replacing CFM's wizard-region execution; the run shape is data (`jta::StageScript` via `BuildStageScript`, consumed by the `Optimize()` stage loop). Playbook: `docs/solutions/architecture-patterns/explicit-inputs-peeling-stateless-objectives.md`.

## Architecture (current, brief)

- Layered layout: `src/domain` (pure logic, Qt/GPU-free — the Rust-interop
  surface), `src/services` (QtCore-only services; never references
  coordinator), `src/coordinator` (QObject orchestration), `src/compute`
  (GPU/CUDA, the SHARED `jtml_compute`), `src/view` (QWidgets), `src/app`
  (GUI composition roots + experimental QML bridges). The old `src/core/` dir
  is gone.
- `include/domain/direct_optimizer.h` / `src/domain/direct_optimizer.cpp` —
  pure DIRECT optimizer with an injected `std::function<double(const Point6D&)>`
  cost. Preserves the cumulative budget (effective 20k/25k/30k across
  trunk/branch/leaf). Has call-offset + iteration/improvement callbacks. Used
  by `OptimizerManager::RunDirectStage` via `jta::BuildGpuCostAdapter` (the
  single production cost path).
- Run shape is DATA: `jta::StageScript` (`BuildStageScript` /
  `DeriveStageCostParams` in `optimizer_stage_script.*`) is built once per run
  in `OptimizerManager::Initialize` and consumed by the `Optimize()` stage
  loop. The named-graph registry (`ListStageGraphs` / `StageGraphByName`) is
  test-only — not yet wired into the manager.

The 2026-08 testability/MVVM refactor introduced the domain/services/ coordinator/view
layering; its plan and requirements documents were deleted (2026-08-28) as superseded. The
seams above are what survives; history is in `jj log`.

## Repo gotchas

- Each layered lib (`src/{domain,services,coordinator}/CMakeLists.txt`) uses
  `file(GLOB ...)` for headers **and** an explicit `.cpp`/`.cu` source list. New `.cpp`
  files must be added to the explicit list (GLOB only catches headers; a header globbed
  without its impl in the target causes an AUTOMOC undefined-symbol link error). See
  `direct_optimizer.cpp`/`optimize_coordinator.cpp` entries there.
- Several files historically relied on **transitive standard includes** that used to arrive
  via `gpu/render_engine.cuh`. The data-structures decoupling (U3) removed that — always
  include what you use (`<cmath>`, `<iostream>`, `<climits>`, ...).
- Don't reify Qt-mocking wrappers (function-pointer `QFileDialog`/`QMessageBox` shims) or
  "instantiate the real `MainScreen`" characterization tests — both were rejected as low-value.
