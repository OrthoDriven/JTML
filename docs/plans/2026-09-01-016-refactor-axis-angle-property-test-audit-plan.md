---
title: Refactor: Axis-angle rotation property tests, metamorphic suite audit, and explicit MinBoxSize wiring
type: refactor
status: active
date: 2026-09-01
deepened: 2026-09-01
---

# Refactor: Axis-angle rotation property tests, metamorphic suite audit, and explicit MinBoxSize wiring

## Overview

SO(3) property tests for `RotationRepresentation::AxisAngle` in the Rust DIRECT port
(`rust/direct-rs`) have been added and pass. Three things remain to finish this
workline cleanly:

1. **Audit `rust/direct-rs/src/properties.rs`** — its metamorphic proptests assume all
   six `Pose` coordinates behave as plain affine scalar coordinates. That assumption
   holds only in Euler mode; it is geometrically invalid for the rotational half of
   the AxisAngle mapping (whose outputs are ZXY Euler *serializations* of composed
   rotations, not denormalized search coordinates).
2. **Make `MinBoxSize` explicit at construction** — the implicit
   `MinBoxSize::default()` (all axes `Some(0.1)`) silently imposes a resolution floor
   that regressed two bench.rs convergence-threshold tests. Owner directive: remove
   the implicit default; every construction site chooses its floor explicitly; the
   cxx-bridge path (`new_rust_opt`) must create a `MinBoxSize` explicitly so
   production behavior is unchanged and visible at the seam.
3. **Documentation** — record the rotation-semantics contract, the acos-saturation
   measurement gotcha, prune stale proptest regression seeds, and update
   `AGENTS.md` / the FFI-seam org docs.

## Problem Frame

The parent commit (`qxlvwryt` — "feat: using axis-angle in optimization vs Euler")
introduced a body-frame scaled-axis rotation mode. An `AxisAngle` unit-space
coordinate triple denotes a scaled-axis (exponential-map) increment relative to the
starting pose, not final Euler angles:

- `v_deg_i = (unit_i - 0.5) * 2 * range_i` (degrees at the interface)
- `R_result = R_start * Exp(v_rad)` (local/body-frame composition)
- The result is serialized back to ZXY Euler angles only because the C++/FFI `Pose`
  interface consumes Euler angles.

The project ZXY convention is anchored externally by the C++ renderer
(`src/compute/render_engine.cu`, `SetPose`: `/* R*v = RzRxRy*v */`), i.e.
`M(xa, ya, za) = Rz(za) * Rx(xa) * Ry(ya)`, and independently by the recovery
algorithm in `include/services/calibration.h`.

A real production bug was found this way and fixed in the working copy:
`DirectOptimizer::from_euler_ordered` had its intrinsic/extrinsic branches swapped,
building `starting_rotation` as `Ry*Rx*Rz` instead of the C++-matching
`Rz*Rx*Ry`. The new property tests are the regression guard.

Untreated residue: the metamorphic suite's silent Euler-mode assumptions, the
implicit `MinBoxSize` floor, and missing documentation.

## Requirements Trace

- R1. Primary contract test: `R_start⁻¹ * R_result ≈ Exp(v)` for generated starts and
  small deltas, with failure output naming start Euler pose, requested `v`, and
  angular error in degrees. *(DONE — landed in working copy)*
- R2. Zero-delta, delta-inverse round-trip, relative-angle-equals-scaled-axis-norm,
  same-axis additivity, local-frame equivariance under world rotation `Q`, and
  pure-axis sanity properties — all comparing SO(3), never Euler components.
  *(DONE — landed in working copy)*
- R3. Convention locked to the C++ `SetPose` literal independently of nalgebra;
  serializer/reconstructor inverse tested. *(DONE — landed in working copy)*
- R4. Existing metamorphic properties must state which rotation mode they assume:
  keep mode-agnostic ones, make Euler-only ones explicit (rename + construct Euler
  mode via the test seam), never silently inherit the default.
- R5. AxisAngle-mode optimizer-level invariants added as SO(3) statements
  (bounded rotational delta; zero-range pinning semantics), not Euler-component boxes.
- R6. No false properties tested: no componentwise Euler equality, no cross-axis
  `Exp` commutativity claims, no Euler-component range claims. *(constraint on R4/R5)*
- R7. The two pre-existing bench failures are resolved by removing the implicit
  `MinBoxSize` floor (per owner diagnosis), not by loosening thresholds; the
  cxx-bridge constructor creates a `MinBoxSize` explicitly.
- R8. Rotation semantics + measurement gotchas documented where future agents will
  look (`docs/solutions/`, FFI-seam org, FFI-handshake org, `AGENTS.md`).
- R9. Full Rust test suite green afterwards (modulo any separately-tracked
  exclusions); stale proptest regression seeds pruned.

## Scope Boundaries

- AxisAngle mode remains unreachable from production: no public setter, no FFI
  parameter for `RotationRepresentation`. Making it reachable is explicitly **not**
  this plan (tracked in `docs/rust-direct-rs-ffi-seam-todos.org`).
- No changes to the C++ side; the C++ path stays Euler-only.
- No reintroduction of the removed CUDA-graph stack (per `AGENTS.md`).
- Deferred to follow-up work:
  - A Rust proptest-authoring conventions doc mirroring `test/HEGEL-PBT-GUIDE.md`
    (that guide is C++/hegel-only; a future session can extract one from this
    suite's final shape).
  - Wiring `MinBoxSize` through the FFI as a tunable per-stage parameter (today the
    bridge fixes it at the historical `Some(0.1)` values).

## Context & Research

### Relevant Code and Patterns

- `rust/direct-rs/src/direct_optimizer.rs` — `physical_pose_for_eval` (the mapping
  under test), `from_euler_ordered` / `axes_from_str` (fixed branches),
  `#[cfg(test)] mod axis_angle_tests` (12 tests, the pattern to follow),
  `#[cfg(test)] impl DirectOptimizer::new_in_mode` (mode-explicit seam),
  `MinBoxSize::default()` currently applied in `new_with_strat`, and
  `split_axis`/`physical_width` (where the floor gates refinement).
- `rust/direct-rs/src/properties.rs` — `run_recorded` tape harness,
  `Recording`/`Affine`/`InUnit`/`Permuted` cost adapters, `CASES = 32`.
- `rust/direct-rs/src/test_support.rs` — `denorm`/`invert`/`permute_pose` affine
  helpers (the Euler-only assumptions), `coords`, `pose`, `splat`.
- `rust/direct-rs/src/bench.rs` — `shifted_sphere_reaches_min` (~line 348) and
  `anisotropic_weights_still_reach_min` (~line 406): convergence-threshold tests
  using `DirectOptimizer::new`, thus inheriting the default `MinBoxSize`.
- `rust/direct-rs/src/lib.rs` — `new_rust_opt` (the single cxx-bridge construction
  site; `src/coordinator/optimizer_manager.cpp:1294` is its C++ caller).
- `src/compute/render_engine.cu` ~480-500 (`SetPose`), `include/services/calibration.h`
  (~171-230, ~258+): the ZXY convention anchors.
- nalgebra 0.35: `euler_angles_ordered` convention pinned by its own doctest
  (`M = R_{seq0}(a0)*R_{seq1}(a1)*R_{seq2}(a2)` for `extrinsic=false`);
  `UnitQuaternion::from_rotation_matrix` + `vector()`/`scalar()` for the stable
  angle metric.

### Institutional Learnings

- `docs/solutions/conventions/jtml-testability-and-cmake-conventions-2026-08-07.md` —
  anti-circular-test doctrine: helpers must not re-derive the math under test.
  Mitigation already in place: the ZXY convention is pinned to the C++ literal
  (`cpp_setpose` test), and property oracles (`Exp(v)` targets) are constructed
  independently of `physical_pose_for_eval`.
- `docs/solutions/logic-errors/jtml-cost-function-update-parameter-int-noop-2026-08-12.md` —
  "a failing consistency invariant is a genuine defect somewhere; find it, don't bake
  it in." Drives R7: fix the silent default, keep the thresholds.
- `docs/solutions/logic-errors/cost-function-parameter-double-truncation-2026-08-08.md` —
  round-trip PBT invariants are the strongest silent-corruption detector; the
  serializer/reconstructor round-trip test mirrors this.
- `docs/solutions/logic-errors/jtml-cuda-graph-stub-failure-2026-08-19.md` —
  no `[x]` on trust; re-run rather than assume green.
- `docs/solutions/architecture-patterns/jtml-three-layer-optimizer-architecture.md` —
  cumulative budget model (20k→25k→30k→35k) context for the production cost path;
  `new_rust_opt` sits on this path, hence the explicit-floor requirement.

### External References

- None used. The two version-sensitive facts (nalgebra `euler_angles_ordered`
  convention; `sin_cos()` returning `(sin, cos)`) were verified against the pinned
  nalgebra 0.35.0 registry source and its doctests.

## Key Technical Decisions

- **SO(3)-only assertions for rotation semantics.** All rotational property checks use
  the geodesic angle between reconstructed rotations; never compare Euler components
  (branch/wrap makes component equality meaningless).
- **Error metric = `2 * atan2(‖imag(q)‖, |w(q)|)`** on the relative rotation's
  quaternion, not `Rotation3::angle()` (`acos((trace-1)/2)` saturates at ~√eps rad
  and NaNs just above 1 for near-identity rotations; this exact artifact produced
  phantom 1.2e-6° "failures" before the metric was changed). Tolerance `1e-9 rad`
  (≈ 5.7e-8°) is then comfortably achievable through the serializer round-trip.
- **Property tests target the mapping directly** (`physical_pose_for_eval` via
  `apply(start, v)`), not full DIRECT runs, with `cases: 4096`; optimizer-level
  properties keep the existing `CASES = 32` tape idiom because each case runs DIRECT.
- **Mode-explicit naming over `#[ignore]`.** Euler-only properties are renamed with an
  `_in_euler_mode` suffix and constructed via `new_in_mode(..., Euler)`. Renames and
  the pre-fix seeds make `rust/direct-rs/proptest-regressions/` entries stale →
  prune (R9).
- **`MinBoxSize`: no implicit defaults.** `new`/`new_with_strat` no longer inject
  `MinBoxSize::default()`. Deleting `impl Default for MinBoxSize` outright is the
  preferred route: it forces compile errors at every missed construction site
  (`new_in_mode`, `run_recorded`, internal test modules) instead of letting a
  silent floor-loss through; if it is kept instead, every such site must be
  enumerated. A `None`-floor (unlimited refinement) is the right choice
  for pure convergence benches; `new_rust_opt` constructs the historical
  all-`Some(0.1)` floor explicitly so FFI behavior is bit-identical to today.
- **AxisAngle optimizer-level invariants are ball/bound statements, not box
  statements.** `|v_deg_i| ≤ range_i` implies
  `angle(R_start⁻¹ R_result) = ‖v_rad‖ ≤ (π/180)·√(Σ range_i²)` — the correct
  geometric replacement for the Euler search-cube property.

## Open Questions

### Resolved During Planning

- Why do the two bench tests fail? Owner diagnosis: the implicit `MinBoxSize` floor
  (R7), confirmed against `split_axis` gating — not a rotation-semantics regression.
- Do the new axis_angle tests re-derive production math (circularity)? No — see
  institutional-learnings mitigation; the C++ literal test is the independent anchor.
- Where do new AxisAngle-mode optimizer properties live? In `properties.rs` (same
  harness idioms), clearly grouped in an `axis_angle_*` naming block alongside the
  renamed `*_in_euler_mode` tests.

### Deferred to Implementation

- Exact constructor signature shape for explicit `MinBoxSize` plumbing
  (extra parameter vs. builder method) — must stay a small, readable seam; decide on
  touch against `optimizer_manager.cpp:1294`'s expectations.
- Whether `Default for MinBoxSize` is deleted outright or left unused — depends on
  remaining construction sites found during the change.
- Bench thresholds after the floor is removed: confirm `1e-6`/`1e-4` are met
  deterministically at the given budgets; if not, that is a new investigation, not a
  threshold loosening (record under R9 exclusions if genuinely stochastic).

## Property Classification Matrix (drives U1/U2)

| properties.rs test | Today assumes | Decision |
|---|---|---|
| `samples_stay_inside_the_search_cube` | per-component affine denorm on all 6 coords | rename → `..._in_euler_mode`, construct Euler explicitly (R4) |
| `domain_affine_equivariance` | `denorm`/`invert` affine on all 6 coords | rename → `..._in_euler_mode` (R4) |
| `axis_permutation_preserves_best_cost` | permuting scaled-axis components preserves the map (false under Exp) | rename → `..._in_euler_mode` (R4) |
| `zero_range_axis_stays_pinned_to_start` | range_i=0 pins Euler output i (false under coupling) | rename → `..._in_euler_mode` (R4) |
| `incumbent_is_the_recorded_argmin` | tape consistency only | keep mode-agnostic; doc-note (R6) |
| `no_duplicate_samples` | bit-distinct serialized poses | keep; doc-note (R6) |
| `positive_affine_cost_is_equivariant`, `negative_affine_does_not_preserve_minimizer` | cost-side transform only | keep; doc-note (R6) |
| `evals_do_not_exceed_budget` (ignored) | budget semantics | leave ignored as-is |
| *(new)* AxisAngle translation cube + rotational ball | SO(3) delta bound | add (R5) |
| *(new)* all-zero rotation ranges pin orientation | SO(3) pinning | add (R5) |
| *(new)* per-component zero range pins scaled-axis component, not Euler output | tangent-space semantics | add (R5) |

## Implementation Units

- [ ] U1. **Make metamorphic suite mode-explicit (Euler renames + construction)**

  **Goal:** every property declares the rotation mode it is valid in; none inherits
  the default silently.

  **Requirements:** R4, R6

  **Dependencies:** none to start (`new_in_mode` seam already exists) — but U3
  changes `MinBoxSize` plumbing underneath `new`/`new_in_mode`; the
  `run_recorded` extension must be coordinated with U3 as a single harness change
  (see U3 Files), not double-touched.

  **Files:**
  - Modify: `rust/direct-rs/src/properties.rs`
  - Modify: `rust/direct-rs/proptest-regressions/properties.txt` (prune in U4)

  **Approach:**
  - Extend `run_recorded` (or add a sibling) to accept a `RotationRepresentation` and
    construct via `DirectOptimizer::new_in_mode`.
  - Rename the four Euler-only tests per the matrix; pin them to `Euler`.
  - Add header + per-test doc notes: which of translation-affine / tangent-space /
    Euler-serialization semantics each coordinate class carries (the three-way
    distinction from the audit).
  - Doc-note the mode-agnostic tests explaining *why* mode cannot matter for them.
  - Keep `CASES = 32`; these run DIRECT.

  **Patterns to follow:** existing `proptest!` block and tape-adapter idioms in
  `properties.rs`; naming convention: suffix `_in_euler_mode` for renamed
  Euler-only tests (per the matrix and Key Technical Decisions), prefix
  `axis_angle_*` for new AxisAngle tests.

  **Test scenarios:**
  - All four renamed tests pass unchanged in Euler mode (they do today).
  - Spot-check one renamed test (e.g. `domain_affine_equivariance`) with the mode
    argument flipped to `AxisAngle` *as a scratch check only* to confirm the property
    genuinely depends on the mode flag (do not commit this variant as a test).
  - Doc comments compile cleanly (`cargo doc` unaffected — plain `///`/`//!`).

  **Verification:** `pixi run cargo test --manifest-path rust/Cargo.toml --lib
  properties::` green; grep shows no remaining `DirectOptimizer::new(` call in
  `properties.rs` outside explicitly-Euler or mode-agnostic helpers.

- [ ] U2. **AxisAngle-mode optimizer-level SO(3) properties**

  **Goal:** replace the affine box intuition with geometric invariants at the level of
  full DIRECT runs (complementing, not duplicating, the mapping-level axis_angle_tests).

  **Requirements:** R5, R6

  **Dependencies:** U1 (shared harness extension)

  **Files:**
  - Modify: `rust/direct-rs/src/properties.rs` (new `axis_angle_*` proptest block)
  - Test helpers may move into `rust/direct-rs/src/test_support.rs` if shared

  **Approach:**
  - Property A (cube + ball): run AxisAngle mode with translation ranges and rotation
    ranges; every recorded sample's translation stays in `[start_i − range_i,
    start_i + range_i]`, and every sample's rotational delta satisfies
    `angle(R_start⁻¹ R_sample) ≤ (π/180)·√(Σ r_rot_i²) + tol` — the SO(3) rewrite of
    the search-cube property. Reuses the `ang_err`-style quaternion metric; move it to
    a shared test helper if U1's notes don't keep it private to `axis_angle_tests`.
  - Property B (all rotation ranges zero): every sample reconstructs to the starting
    rotation in SO(3) (translation still free), even though Euler components of
    samples may legitimately differ from the starting Euler triple.
  - Property C (single component pinned): zeroing one rotation range forces the
    corresponding scaled-axis *component* of the observed delta to 0
    (`relative.scaled_axis()[i] ≈ 0`); explicitly do NOT assert the output Euler
    component equals the starting Euler component (coupling — the audit's named trap).
  - Generators: starts keep the middle ZXY angle ≤75°; ranges small (≤ 20° per axis).

  **Patterns to follow:** U1's mode-parameterized `run_recorded`; existing
  `ShiftedSphere` cost + tape recording; `axis_angle_tests` tolerance policy
  (1e-9 rad for mapping-level; use 1e-12 *absolute* only where exact identity holds).

  **Test scenarios:**
  - Happy path: all three properties pass across 32 generated runs each.
  - Edge case: `range_rot = [20,20,20]` with start at the gimbal-adjacent corner —
    ball bound must still hold (bound is on the delta, not on Euler components).
  - Negative control (dev-time only, not committed): with `RotationRepresentation::Euler`
    substituted into Property C, the scaled-axis-component assertion is meaningless —
    confirm the test names/structures cannot be accidentally mode-agnostic.
  - Error path: NaN/hostile costs not re-tested here (already covered).

  bound is *tight* via a **runtime assertion**, not a doc comment: track the
  maximum observed `angle(R_start⁻¹ R_sample)` across cases (e.g. a Cell in the
  harness), and after the proptest block assert it exceeds ~0.3 ×
  `(π/180)·√(Σ r_rot_i²)` — so a future generator-range shrink or silent
  mode-swap fails the suite instead of quietly weakening the bound.

- [ ] U3. **Explicit MinBoxSize plumbing; restore bench convergence tests**

  **Goal:** remove the implicit resolution floor from the default path; fix
  `shifted_sphere_reaches_min` and `anisotropic_weights_still_reach_min`
  (owner-diagnosed as default-`MinBoxSize` artifacts) without touching thresholds;
  production cxx path keeps identical behavior via an explicit floor.

  **Requirements:** R7

  **Dependencies:** none (parallel-safe with U1/U2 only; U4 must land after U3 —
  see U4's dependency list; coordinate with U5's docs)

  **Files:**
  - Modify: `rust/direct-rs/src/direct_optimizer.rs` (`new_with_strat`, struct field
    assignment, `MinBoxSize` Default usage, and the `#[cfg(test)] new_in_mode`
    constructor — required: every renamed Euler-mode test from U1 routes through
    it, and it delegates to `Self::new()`, inheriting whatever floor injection
    `new_with_strat` performs)
  - Modify: `rust/direct-rs/src/lib.rs` (`new_rust_opt` constructs `MinBoxSize` explicitly)
  - Modify: `rust/direct-rs/src/bench.rs` (bench constructors choose their floor;
    `anisotropic_ranges_still_reach_min` keeps an explicit `Some(0.1)`-style floor —
    its rotation ranges are exactly 0.1 and it intentionally probes anisotropic
    resolution; verify it still passes at its 1e-1 threshold)
  - Modify: `rust/direct-rs/src/properties.rs` — `run_recorded` is a GUARANTEED
    breakage (it calls `DirectOptimizer::new`, inheriting the default floor); its
    MinBoxSize extension must be coordinated with U1's RotationRepresentation
    extension into a single harness change
  - Modify: `rust/direct-rs/src/direct_optimizer.rs` `#[cfg(test)] mod tests` —
    `volume_partition_holds_for_random_runs`, `every_box_center_is_on_the_trisection_lattice`,
    `no_two_boxes_share_a_unit_center`, `each_box_depths_differ_by_at_most_one` all
    call `DirectOptimizer::new` and must choose explicit floors
  - Possibly: `rust/direct-rs/src/test.rs`, `axis_angle_tests` helpers
    (only where they relied on the default)

  **Approach:**
  - Add an explicit `MinBoxSize` route to construction (parameter or dedicated
    constructor — smallest readable shape); stop calling `MinBoxSize::default()`
    inside `new_with_strat`.
  - Benches that assert convergence get the unrestricted floor (all `None`);
    benches that intentionally probe anisotropic resolution keep explicit values.
  - `new_rust_opt` constructs the historical all-`Some(0.1)` floor explicitly,
    documented as behavior-preserving until the FFI exposes the parameter.
  - Do not change budget, thresholds, or `split_axis` logic.

  **Execution note:** owner states these tests passed before the default floor
  landed — this is a regression restoration, treat it as characterization-first:
  confirm green at the pre-feature threshold with the floor removed before
  anything else.

  **Test scenarios:**
  - Happy path: `shifted_sphere_reaches_min` meets `1e-6` cost threshold;
    `anisotropic_weights_still_reach_min` meets `1e-4` — no budget increases.
  - Integration: `new_rust_opt` behavior unchanged vs today for identical inputs
    (best-cost bit-identity spot check via the existing
    `same_inputs_give_bit_identical_result`-style determinism test, extended to
    construct via `new_rust_opt`'s Rust-side equivalent).
  - Edge case: a run whose every axis floor is reached (tiny ranges) still
    terminates (`split_axis` returns `None` → loop exits) — covered by existing
    volume-partition tests staying green.
  - Error path: `MinBoxSize::default()` no longer exists / is unreachable from
    production constructors (grep-clean).

  **Verification:** full `--lib` suite green including the two benches; the
  convergence thresholds are untouched in the diff.

- [ ] U4. **Prune stale proptest regression seeds**

  **Goal:** committed seed files must not encode pre-fix or renamed-test history.

  **Requirements:** R9

  **Dependencies:** U1 (renames), U3 (bench changes may re-seed)

  **Files:**
  - Modify: `rust/direct-rs/proptest-regressions/direct_optimizer.txt`
  - Modify: `rust/direct-rs/proptest-regressions/properties.txt`

  **Approach:** `direct_optimizer.txt` holds 5 seed entries: four `axis_angle_*`-family
  seeds captured this session under the pre-fix branches plus the acos-metric bug
  (stale — the failures they recorded are fixed) and one older convex-hull seed.
  Before dropping each stale entry, replay it against the post-fix code under its
  current (renamed, post-U1) test name and confirm it passes; record the replay
  outcome in the change description. Remove seeds whose test names were renamed out
  of existence in U1; re-run with seeds replay enabled so surviving entries still
  pass post-U3.

  **Test scenarios:** `Test expectation: none` — data-file hygiene; exercised by the
  suites replaying remaining seeds green in U6.

  **Verification:** a `cargo test` run logs no stale-name warnings and all
  surviving seed replays pass.

- [ ] U5. **Documentation: rotation contract, measurement gotcha, seam/status updates**

  **Goal:** the next agent (or human) in this area finds the conventions without
  re-deriving them.

  **Requirements:** R8

  **Dependencies:** U1, U2, U3 (document the final shapes)

  **Files:**
  - Create: `docs/solutions/numerics/rotation-angle-metric-acos-saturation-2026-09-01.md`
    (YAML frontmatter: `module`, `tags`, `problem_type`, matching the existing
    solutions taxonomy)
  - Modify: `docs/rust-direct-rs-ffi-seam-todos.org` (rotation items: mark what the
    property suite now guards; note AxisAngle remains seam-unreachable and what that
    implies)
  - Modify: `AGENTS.md` ("Current work" — one short paragraph: axis-angle mode +
    property suite + MinBoxSize wiring landed; pointer to the solutions doc)
  - Modify (light): `docs/rust-direct-rs-ffi-handshake.org` property-tests section
    (~1885-1924) to reference the new rotation tier

  **Approach:**
  - Solutions doc covers both faces of the same trap class: (1) `acos((trace-1)/2)`
    saturation/NaN near identity and the `2·atan2` replacement, with the concrete
    phantom-failure numbers observed this session; (2) the C++ `SetPose` ZXY
    literal as convention anchor and `euler_angles_ordered`'s doctest-pinned
    convention, including the `sin_cos()` return-order footnote (the bug that
    *did* bite during development).
  - Org updates via `edit` anchors (per repo org rules), not full rewrites.

  **Test scenarios:** `Test expectation: none` — prose deliverable; accuracy is
  checked in doc review (Phase 5.3.8) and by cross-references resolving.

  **Verification:** every file path/symbol cited in the new docs exists in the tree
  after U1-U3; `AGENTS.md` "Current work" no longer omits the rust workline.

- [ ] U6. **Full-suite verification and change hygiene**

  **Goal:** prove R1-R9 together and leave history readable.

  **Requirements:** R9

  **Dependencies:** U1-U5

  **Approach:**
  - Run the full headless Rust suite (mapping-level `axis_angle_tests` at 4096
    cases, `properties::` block, `bench::`, deterministic tests) plus a CMake-built
    workspace sanity check (`pixi run build`) if the bridge signature or struct
    surface moved.
  - Land as separate jj changes per repo workflow (`describe` → `new`): test audit +
    properties (U1/U2), MinBoxSize wiring (U3), regressions prune (U4 — lands after
    both U1 and U3; its diff may fold back into the earlier changes once both have
    landed and seeds replay green), docs (U5). Scope-prefixed messages per owner
    convention.

  **Verification:** zero failures; the two previously-failing bench tests pass;
  ignore count unchanged (only the pre-existing budget-overshoot ignore).

## System-Wide Impact

- **Interaction graph:** `new_rust_opt` → `optimizer_manager.cpp:1294` (C++ DIRECT
  stage). The MinBoxSize change must be a no-op on that path; golden-tier C++
  comparisons remain valid.
- **API surface:** production Rust API gains at most a `MinBoxSize`-aware
  constructor; `RotationRepresentation` stays test-settable only. No cxx bridge
  signature changes.
- **Unchanged invariants:** Euler mode remains the only production-reachable
  representation; the AxisAngle branch stays serialized through
  `euler_angles_ordered("ZXY", false)`; budget/call-offset semantics untouched;
  `split_axis` logic untouched.
- **Integration coverage:** U3's bit-identity check is the only scenario that
  spans the FFI-adjacent construction path without C++ present — full C++ parity
  stays with the golden/oracle tiers per repo doctrine.

## Risks & Dependencies

| Risk | Mitigation |
|---|---|
| Removing the implicit floor changes another test's behavior that relied on `Some(0.1)` | grep every `MinBoxSize`/`new(` construction site (U3 approach bullet); full-suite run in U6 catches the rest |
| Renamed property tests orphan committed seeds | U4 prunes explicitly; U6 replays survivors |
| New ball-bound property is too loose to be useful | U2 verification requires the observed max-delta to actually press against the bound |
| Bench thresholds prove stochastic even without the floor | treated as a fresh finding (not a threshold edit), surfaced in U6 notes; never `#[ignore]` |
| AxisAngle someday becomes reachable and old "mode-agnostic" notes turn out mode-sensitive | every kept-mode-agnostic test now carries a one-line *why*, making the re-audit mechanical |

## Sources & References

- Owner task specification (this session): rotation semantics, required properties 1-7, audit instructions.
- Owner diagnosis (2026-09-01): bench failures attributable to `MinBoxSize` default; directive to remove implicit defaults and make the cxx path construct one explicitly.
- Working copy: `rust/direct-rs/src/direct_optimizer.rs` (`axis_angle_tests`, `new_in_mode`, fixed `from_euler_ordered` branches).
- `src/compute/render_engine.cu:480-500`; `include/services/calibration.h:171-230`.
- Learnings: `docs/solutions/conventions/jtml-testability-and-cmake-conventions-2026-08-07.md`; `docs/solutions/logic-errors/jtml-cost-function-update-parameter-int-noop-2026-08-12.md`; `docs/solutions/logic-errors/cost-function-parameter-double-truncation-2026-08-08.md`.
- Recon reports: `ce-repo-research-analyst` (run `6bcfa4f4`), `ce-learnings-researcher` (run `0c2b8acd`), `panoptes-explore` (run `c079b710`).
