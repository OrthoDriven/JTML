---
title: "refactor: Pose newtypes, SearchSpace extraction, and review-item completion in direct-rs"
type: refactor
status: active
date: 2026-09-01
revised: 2026-09-01 — review rounds 1–3 integrated (one-way space dependency, Result-vs-firewall channels, Refinement payload enum, eval_one centralization, zero-ray exception, Direction axis flow; round 3: manual BobyqaSettings Default, private newtype fields, SearchSpace behavior-not-fields, behavior-based stale-precompute guard)
---

# refactor: Pose newtypes, SearchSpace extraction, and review-item completion in direct-rs

**Target:** the `rust/direct-rs` crate (Rust workspace member, C++ host via cxx — untouched).

## Overview

The module reorganization of `direct-rs` has landed (shared `pose.rs` / `cost.rs` / `bridge.rs` above optimizer modules; `direct/{mod,settings,poh,tree}.rs`; root-level `fixtures/problems/properties` with sibling test files). It verified numerically neutral: 56 passed / 2 ignored, with the 2 failing convergence tests proven pre-existing via bit-identical costs against the pre-reorg revision.

This plan implements the remaining items from the architecture review, as decided by the owner:

1. **Full newtype separation of pose roles** (`PhysicalPose` / `UnitPose` / `PoseRange`) — the chosen intensity of review item E2.
2. **`SearchSpace` extraction** — the owner-endorsed "best architectural refactor": `RotationMap` / `TranslationMap` constructed once from settings, owning the unit→physical mapping and its precomputed state (starting rotation, camera ray basis). `DirectOptimizer` becomes concerned with DIRECT, not camera geometry — and `space.rs` knows nothing about DIRECT, receiving its inputs as plain arguments.
3. The remaining review batch: settings renames/derives/constructors (A4, E1), strategy dispatch (A6), `Incumbent` (E2), `Result` **and** retained panic containment for BOBYQA (E4), `AxisSeq` (E5), Cost batch contract centralized in `eval_one` (T2), trisect consolidation + `Direction` axis flow (A3), and the E6 idiom sweep.

Function semantics must not change — with one declared exception (R12: the zero-ray `CameraCentered` fallback becomes defined +Z instead of NaN-poisoned). Structural semantics may.

## Problem Frame

`Pose` currently plays three incompatible roles: world-space pose, unit-cube hyperbox center, and (in `DirectOptimizer::range`) per-axis half-widths that are not a pose at all. The mapping machinery (rotation/translation modes, `starting_rotation`, `translation_basis`) lives inside `DirectOptimizer`, so `basin_opt` borrows the optimizer to denormalize — an artificial coupling given that basin will become a peer optimizer at the driver level, not a DIRECT subordinate. Around this, the review found: a silently-truncating `zip` where the batch-cost contract is only enforced by tests; a `catch_unwind` standing in for a `Result` **and** a missing firewall framing (the two cover different failure channels); a panicking string-parsed axis sequence on the per-evaluation hot path; duplicated trisection arithmetic; settings with three sources of truth; and `usize` axes where a six-value `Direction` type already exists.

Decisions in this plan were fixed interactively with the owner (see Requirements Trace) and adjusted per the external plan review (see Sources); nothing here reopens them.

## Requirements Trace

- R1. Newtypes `PhysicalPose(Pose)`, `UnitPose(Pose)`, `PoseRange(Pose)` in `rust/direct-rs/src/pose.rs` — **private inner field**: construct via `From<Pose>`/`From<[f64; 6]>`, read via `Deref<Target = Pose>`; no cross-module `.0` past the role gate. `Cost::eval` consumes `&[PhysicalPose]`. `DirectOptimizer::new`/`from_settings` take `PoseRange` + `PhysicalPose`; `run`/`best` return `Incumbent { pose: PhysicalPose, cost: f64 }`; `Hyperbox.center`/`UnscoredHyperbox.center` become `UnitPose`. (owner: "full newtypes please")
- R2. `Cost` batch contract documented on the trait and enforced at **every** consumption path, centralized in a default method:
  ```rust
  pub trait Cost {
      /// Returns exactly one cost per supplied physical pose, order preserved.
      fn eval(&self, poses: &[PhysicalPose]) -> Vec<f64>;

      fn eval_one(&self, pose: PhysicalPose) -> f64 {
          let mut values = self.eval(std::slice::from_ref(&pose));
          assert_eq!(values.len(), 1, "Cost::eval must return one cost per pose");
          values.pop().expect("length asserted above")
      }
  }
  ```
  Seed evaluation uses `cost.eval_one(physical)`; the basin adapter uses `self.cost.eval_one(physical)`; batch scoring in `score_and_reinsert` keeps the matching `assert_eq!(evaluated.len(), centers.len(), ...)` (same message). No `[0]` / `.first().expect(...)` single-pose accesses remain anywhere.
- R3. `SearchSpace { start, range, rotation: RotationMap, translation: TranslationMap }` in `rust/direct-rs/src/space.rs`. Constructor signature: `SearchSpace::new(start: PhysicalPose, range: PoseRange, rotation: RotationRepresentation, translation: TranslationRepresentation)` — **plain arguments, no `&DirectSettings` parameter**: `space.rs` imports nothing from `direct::`, keeping the dependency arrow `DirectSettings → DirectOptimizer → SearchSpace` strictly one-way (`RotationMap::{Euler, AxisAngle{start}}`, `TranslationMap::{Euclidean, CameraCentered{basis}}` built once at construction; mode-dependent — no basis for `Euclidean`, no starting rotation for `Euler`). `physical_pose(&self, unit: UnitPose) -> PhysicalPose`; `PoseRange::width_at(dir: Direction, depth: u32) -> f64`. **Data hidden, operations exposed**: `SearchSpace`'s fields stay private to `space`; `direct/` reaches them only through `physical_pose(..)` and a `pub(crate) width_at(dir, depth)` delegator — consumers never learn a `PoseRange` lives inside.
- R4. `basin_opt::run_bobyqa` takes `&SearchSpace` (decoupled from `DirectOptimizer`) and returns `Result<Refined { pose: PhysicalPose, cost: f64, evals: u64 }, String>`; the `.expect("BOBYQA failed")` is deleted. **The `catch_unwind(AssertUnwindSafe(..))` firewall is retained** at the `refine()` boundary around `run_bobyqa`: `Result` closes *our* error-as-panic channel (the `Executor::run() -> Result` path), but basin is third-party numerical code with verified internal panic sites no `Result` can catch — `bobyqa/driver.rs` asserts `rho_beg > rho_end > 0` (its own comment concedes degenerate coordinate ranges "still trip" it), `bobyqa/init.rs` asserts model validity pre-`Result`, `bobyqa/trsbox.rs` asserts a positive trust-region step. Today's constants satisfy those asserts; R5 deliberately makes the knobs configurable, which makes them reachable from our own settings surface. Degradation policy on both channels: log, keep the DIRECT incumbent. (If basin later converts its assert sites to `Err`, the firewall can go.)
- R5. Settings renames + ceremony: `poh_selection_strategy`→`poh_strategy`, `rotation_style`→`rotation`, `translation_style`→`translation`, `POHSettings`→`PohStrategy`, `RefinementOptions`→`Refinement`; `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]` (et al.) on the small enums; explicit `= 0/=1` discriminants dropped; `MinBoxSize::uniform(f64)`. **`Refinement` carries its payload**: `enum Refinement { #[default] None, Bobyqa(BobyqaSettings) }` with `BobyqaSettings { max_evals: 500, rho_beg: 0.5, rho_end: 1e-3, npt: 28 }` — derives `Debug, Clone, Copy, PartialEq` plus a **manual `impl Default` returning exactly those constants**: a *derived* `Default` would zero `rho_beg`/`rho_end` (tripping basin's `driver.rs:152` rho assert on the first run) and `max_evals` (a silently no-op refinement), so `Refinement::Bobyqa(BobyqaSettings::default())` must be a working refinement. There is no standalone `bobyqa` field, so the nonsensical state "BOBYQA tuning while refinement is disabled" is inexpressible. `DirectSettings::production(use_bobyqa: bool)` owns the FFI-side hardcodes, including `refinement: if use_bobyqa { Refinement::Bobyqa(BobyqaSettings::default()) } else { Refinement::None }`.
- R6. Strategy dispatch per review A6: `impl PohStrategy { pub fn select(self, &[POHPoint]) -> Vec<POHPoint> }`; refinement dispatch inside `DirectOptimizer::refine` (inherent, not enum method — the BOBYQA arm needs optimizer state and `crate::basin_opt` access; keeps `settings.rs` dependency-free). The `Refinement` payload shape makes dispatch read: `match self.settings.refinement { Refinement::None => {}, Refinement::Bobyqa(bobyqa) => { .. } }` — tuning arrives exactly where the stage runs.
- R7. `call_offset` deleted outright (owner handles cumulative budgets C++-side via `budget`); loop guard becomes `self.calls >= self.budget`. `print_resolution_summary` and the dead `Range` experiment deleted.
- R8. `AxisSeq` enum (`ZXY` variant) with `fn axes(self)` in `pose.rs` replaces `axes_from_str`; no panic, no per-evaluation string parse/alloc; `from_euler_ordered` and the `euler_angles_ordered` call consume `AxisSeq`/`.axes()`.
- R9. A3 trisect consolidation **plus the axis-type fix**: production `trisect_and_return_unscored` calls `Hyperbox::trisect(..)`; the two divergent axis selectors become one. Axis values flow as `Direction`, not `usize`: `impl Direction { pub const fn index(self) -> usize }`, `split_axis(..) -> Option<Direction>`, `longest_axis() -> Direction`, `trisect(self, axis: Direction)`, `width_at(Direction, depth)`; array indexing only via `axis.index()`; `Pose::shift(dir: Direction)` by value (was `&Direction`, E1). `DIRECTIONS` stays as the canonical depth-index↔Direction pairing used when iterating. An out-of-range axis becomes inexpressible, which also retires the "decorative `get_mut` guard then direct index" inconsistency: the guard disappears rather than being documented. Adopted from the external review; bounded to files already in this plan's lists (worst case: drop the `width_at` leg if it cascades beyond them).
- R10. E6 idioms: method-form `.zip` with tuple destructuring (drop `std::iter::zip` import); redundant `is_finite && !is_nan` → `is_finite`; `Vec::new`+push in `get_potentially_optimal_candidates` → `filter_map().collect()`; `unscored` built with `Vec::with_capacity(2 * poh.len())`; seed `.first().expect` replaced by `eval_one` (R2).
- R11. Visibility: `DirectOptimizer::boxes` → `pub(crate)` (same-crate tests only).
- R12. **Explicitly out of scope:** budget overshoot handling (owner veto — `#[ignore]` on `evals_do_not_exceed_budget` stays, no truncation/clamp); the 2 pre-existing convergence-tolerance failures (`shifted_sphere_reaches_min`, `anisotropic_weights_still_reach_min` — must fail *identically*, same costs); the C++ side and cxx bridge include paths; renaming `direct-rs` itself. **Declared semantic exception:** constructing a `CameraCentered` space at zero starting translation is now defined as a `+Z` ray instead of producing non-finite state (previously silent NaN poisoning). No reachable path depends on the NaN behavior — all existing zero-start tests run `Euclidean`, which never consults a basis. This is the one numerical-behavior change in the plan.

## Scope Boundaries

- No algorithm changes: POH selection, trisection, scoring, refinement, and the ZXY convention are bit-for-bit preserved, excepting only R12's declared zero-ray fallback.
- The cxx bridge's *shape* is untouched (`new_rust_opt`, `run_rust_opt`, `CppCost`); only the Rust-side types they marshal flow through newtypes/`From`s.
- `direct/settings.rs` must not depend on `DirectOptimizer` (dispatch lives where the state lives) — and `space.rs` must not depend on `direct/` at all, in either direction (R3).

## Context & Research

### Relevant Code and Patterns

- `rust/direct-rs/src/pose.rs` — newtype host; `#[expect(non_camel_case_types, reason = ...)]` already models the suppression convention; `From<Pose> for [f64; 6]` / `From<[f64; 6]> for Pose` land here (E3) and `to_array` delegates.
- `rust/direct-rs/src/direct/mod.rs` — `DirectOptimizer` fields; `run` seed/loop/refinement/report blocks (refine extracted per R4/R6); `score_and_reinsert` (R2 assert); `physical_pose_for_eval` moves out (→ R3).
- `rust/direct-rs/src/basin_opt/mod.rs` — adapter shape to preserve; `Infallible` error for `CostFunction`; `physical_pose_for_eval` call sites become `space.physical_pose(..)`.
- `rust/direct-rs/src/direct/poh.rs` — `select_potentially_optimal` moves onto `PohStrategy` (R6); hull/pareto remain free `pub` fns.
- `rust/direct-rs/src/direct/test.rs` — `new_in_mode` post-mutation helper **must not survive** into the SearchSpace world (round-3 review item 5 — see U8).
- Test pattern to follow: `let Incumbent { pose: best, cost } = opt.run(&f);` destructuring preserves every existing local variable name; `Recording`/`Affine`/`InUnit`/`Permuted` wrappers in `properties.rs` take `&[PhysicalPose]`, and `Recording.log` becomes `Vec<PhysicalPose>` (deref makes `coords(&pose)` etc. unchanged).
- Verification protocol (established this session): reconstruct a baseline with `jj file show -r <rev>`, run the suite, compare failure messages to the bit; `cargo test -m ./rust/Cargo.toml`.

### Institutional Learnings

- `docs/solutions/architecture-patterns/jtml-three-layer-optimizer-architecture.md` — the C++ coordinator's domain/services/compute split is mirrored by this crate's boundary layering (`space` = domain, `direct` = algorithm, `bridge` = services seam). Decoupling basin from `DirectOptimizer` (R4) and keeping `space.rs` ignorant of `direct/` (R3) completes that pattern in Rust.
- No `docs/solutions/` entries touch `direct-rs` internals yet; the newtype/`SearchSpace` decisions may deserve one after landing.

### External References

- basin 1.7.0 (crates.io): `Executor::run` → `Result<OptimizationResult<S>, So::Error>` with `So::Error = P::Error` for `Bobyqa`. The `Err` channel is unreachable for our `Infallible` problem error — which is exactly why the *panic* channel is the firewall's real job, not the `Result`'s (R4). Verified panic-capable sites in non-test lib code: `solver/bobyqa/driver.rs:152` (rho ordering assert), `solver/bobyqa/init.rs:134-147` (model-validity asserts before the `Result`-returning initialize), `solver/bobyqa/trsbox.rs:136` (`assert!(delta > 0)` on trust-region step).

## Key Technical Decisions

- **Selective `Deref`, minimal `DerefMut`.** `PhysicalPose` and `PoseRange` implement `Deref<Target = Pose>` only; `UnitPose` additionally gets `DerefMut` — it is the role actually mutated during trisection (`posc.shift(..)`). Field reads still flow through deref (~200 `.x` sites unchanged), but the less mutable surface makes the type distinction load-bearing.
- **`From`-based wrapping; private inner `Pose`.** `impl From<Pose>` for each newtype and `From<newtype> for [f64; 6]`, so constructors stay terse (`range.into()`, `start.into()`) and the FFI flattener becomes one `flat_map`. `From<Pose>` stays the *intentional* escape from raw `Pose` into any role — but the private tuple field means changing a role requires naming a conversion, never extracting the payload and re-wrapping it.
- **`space.rs` is private; the public API surface is root re-exports.** `SearchSpace` is an internal collaborator of this crate's optimizers, not an extension point yet. `lib.rs` carries `pub use pose::{Pose, PhysicalPose, UnitPose, PoseRange}; pub use cost::Cost; pub use direct::{DirectOptimizer, Incumbent}; pub use direct::settings::DirectSettings;` and re-exports the style enums from `space`. No `private_interfaces` concern: the wrapped types are nominal-`pub`, so `pub trait Cost` over `PhysicalPose` is lint-clean (verified: current `--all-targets` shows no such warning with `mod pose` private).
- **One-way settings flow.** `DirectSettings` (in `direct/`) is consumed only by `DirectOptimizer::from_settings`, which feeds `SearchSpace::new` plain values. `space.rs` gains zero knowledge of DIRECT, POH, refinement, or budgets.
- **`Result` and the panic firewall cover different channels.** `Result` replaces our own `expect` (error-as-control-flow inside our adapter); `catch_unwind` remains in `refine()` around `run_bobyqa` because basin can `panic!` internally and third-party numerical code is the canonical defensible containment boundary. A side effect to accept knowingly: an `eval_one` contract assert that fires *during refinement* is contained by the firewall (logged, DIRECT result kept) rather than crashing — the batch-path assert in `score_and_reinsert` still propagates.
- **`Refinement` is a payload enum, not a mode flag plus orphan knobs.** See R5; the disabled-but-configured state is unrepresentable, and dispatch (R6) receives its settings by binding.
- **`refine` stays inherent.** See R6; the enum-method form would drag optimizer state and `crate::basin_opt` into `settings.rs`.
- **`Incumbent` replaces the tuple at `run`/`best`** (the review's "at minimum" was superseded by the full-newtypes answer; the tuple was the flagged smell, `self.current_best.1` appearing a dozen times).
- **Axis values are `Direction`, not `usize`.** Extends the same philosophy as the pose newtypes across the whole unit set: `Pose →` three role-specific types; `usize axis →` a six-value type with a total `index()`. Bounds decision: adopted (R9) because it lands entirely in files the plan already touches; if `width_at` or test churn starts cascading beyond that list, drop the leg and say so in the commit message rather than expanding scope.
- **Precompute means no post-construction setting mutation.** Because `SearchSpace` materializes `RotationMap`/`TranslationMap` at construction, any later `opt.settings.rotation = ..` edit is a silent divergence between declared and actual behavior. Test helpers migrate to construction-time settings (U8), and the stale-precompute guard is **behavioral** — the SO(3) suite fails instantly if settings and the precomputed map diverge — not field introspection; `space`'s visibility stays sealed (round 3).
- **One commit-able unit per existing module boundary**, ordered by the type dependency graph: pose → space → settings/poh/tree → optimizer → basin → bridge → fixtures → test migration. Each unit keeps the suite green (modulo R12's two known failures and the interim-migration note in Risks).

## Output Structure

Modifies only existing files except one new module:

```
rust/direct-rs/src/
├── pose.rs            # + PhysicalPose/UnitPose/PoseRange, AxisSeq, From impls, Direction::index; − axes_from_str, − Range
├── space.rs           # NEW, private mod: RotationMap, TranslationMap, SearchSpace; moved style enums; zero direct:: imports
├── cost.rs            # trait doc (batch contract), &[PhysicalPose], eval_one default method
├── lib.rs             # mod space + root pub-use re-exports of the public surface
├── bridge.rs          # production() + From conversions; − hardcoded settings struct
├── basin_opt/mod.rs   # &SearchSpace, Result<Refined, String>, eval_one; no expect
└── direct/
    ├── mod.rs         # Incumbent, space field, refine() (Result + firewall), call_offset gone, idioms
    ├── settings.rs    # renames, derives, Refinement::Bobyqa(payload), PohStrategy::select, production()
    ├── poh.rs         # − select_potentially_optimal; POHPoint gains Debug
    └── tree.rs        # UnitPose centers, trisect(Direction)
```

## High-Level Technical Design

> *This illustrates the intended approach and is directional guidance for review, not implementation specification. The implementing agent should treat it as context, not code to reproduce.*

```
DirectSettings (declares WHAT the space is)              SearchSpace (owns the precomputed state)
├── poh_strategy:  PohStrategy ──select──▶ poh.rs        start:   PhysicalPose
├── min_box_size                                           range:   PoseRange ──▶ width_at(Direction, depth)
├── rotation:      RotationRepresentation ────plain────▶  rotation:    RotationMap      (Euler | AxisAngle{start})
├── translation:   TranslationRepresentation ───args───▶  translation: TranslationMap   (Euclidean | CameraCentered{basis})
└── refinement:    Refinement::Bobyqa(BobyqaSettings)
        │
        ▼
DirectOptimizer::refine ── catch_unwind firewall ──▶ basin_opt::run_bobyqa(&space, &cost, UnitPose, BobyqaSettings)
                                                          │  Result<Refined, String>   (Err = our channel)
                                                          └ panic = basin's channel → contained, logged, DIRECT kept

DirectOptimizer ── space.physical_pose(UnitPose) ──▶ PhysicalPose ── Cost::eval(&[PhysicalPose]) ─▶ Vec<f64>
     │                                                    ▲                    Cost::eval_one(PhysicalPose) — asserted 1:1
     └─ Incumbent { pose: PhysicalPose, cost: f64 } ◀── run()/best() ─────────┘
```

Construction-time discipline: `SearchSpace::new` computes the starting rotation **only** for `AxisAngle` and the camera basis **only** for `CameraCentered` (try-normalize, +Z fallback — the R12 exception), and `space.rs` imports no `direct::` items. `refine()`'s containment sketch (directional):

```rust
match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    basin_opt::run_bobyqa(&self.space, cost, best_unit, bobyqa)
})) {
    Ok(Ok(refined)) => { .. accumulate calls, update incumbent .. }
    Ok(Err(err)) => eprintln!("BOBYQA failed; keeping DIRECT result: {err}"),
    Err(_) => eprintln!("BOBYQA panicked; keeping DIRECT result"),
}
```

## Implementation Units

- [x] U1. **Newtypes, axis enum, and direction type in `pose.rs`**

**Goal:** Introduce `PhysicalPose`/`UnitPose`/`PoseRange` and `AxisSeq`; give `Direction` its total `index()`; retire `axes_from_str` and the dead `Range`; add `From` conversions both ways.

**Requirements:** R1, R8, R9(partial), E3

**Dependencies:** None

**Files:**
- Modify: `rust/direct-rs/src/pose.rs`

**Approach:**
- Three `#[derive(Copy, Clone, Debug, PartialEq)] pub struct X(Pose)` wrappers — **private inner field**; construction via `impl From<Pose>` (plus `From<[f64; 6]>` passthrough), reads via `Deref<Target = Pose>`; **no `Default` on the wrappers** (an all-zero `UnitPose` is a lying center — `Pose` itself keeps its own `Default`). Deref per the table: `PhysicalPose`/`PoseRange` → `Deref` only; `UnitPose` → `Deref + DerefMut`. `From<newtype> for [f64; 6]` for marshalling.
- `impl Direction { pub const fn index(self) -> usize }` (match arms per variant); keep `DIRECTIONS` as the canonical pairing; extend the existing `Clone, Copy` derives with `Debug, PartialEq, Eq` (`Copy` matters: `shift` takes `Direction` by value).
- `PoseRange::width_at(dir: Direction, depth: u32) -> f64` = `2.0 * half_widths[dir.index()].abs() * 3^-depth` — extracted from today's `physical_width`.
- `pub enum AxisSeq { ZXY }` + `pub fn axes(self) -> [Unit<Vector3<f64>>; 3]`; `from_euler_ordered` takes `AxisSeq`; delete `axes_from_str`.
- `Pose::shift(dir: Direction)` by value (was `&Direction`).
- Delete `Range`.

**Patterns to follow:** existing `#[expect(non_camel_case_types, reason = "matching cpp style")]` on `Direction`; `to_array` delegates to the new `From`.

**Test scenarios:**
- Happy path: `PoseRange::width_at(Direction::X_DIR, d)` returns `2·|range_i|·3^{-d}` for d ∈ {0, 2, 5} (mirrors old `physical_width` semantics).
- Happy path: `Direction::index()` maps all six variants to 0..=5, and `DIRECTIONS[i].index() == i`.
- Happy path: `from_euler_ordered(AxisSeq::ZXY, angles, false)` produces the same matrix as the deleted string path for 3 sampled triples (covered transitively by `project_zxy_convention_is_rz_rx_ry` once tests compile).
- Edge case: `Pose::shift` with every `Direction` variant moves exactly one coordinate.
- Round-trip: `[f64; 6] → Pose → [f64; 6]` bit-identity.

**Verification:** compiles with `pose.rs` alone migrated (cascade errors elsewhere are expected until U5–U7 — see Risks R1 row).

---

- [x] U2. **`space.rs`: SearchSpace with mode-dependent maps, ignorant of DIRECT**

**Goal:** Own the unit↔physical mapping and its precomputed state outside any optimizer, with a strictly one-way dependency.

**Requirements:** R3, R12(exception declaration)

**Dependencies:** U1

**Files:**
- Create: `rust/direct-rs/src/space.rs`
- Modify: `rust/direct-rs/src/lib.rs` (private `mod space;` + root `pub use` re-exports per Key Technical Decisions)
- Create: `rust/direct-rs/src/space/test.rs` (zero-ray finite-pose unit, `width_at` delegator check, construction-level map asserts — the only place `SearchSpace`'s private fields are visible)
- Modify: `rust/direct-rs/src/direct/settings.rs` (remove the two style enums; re-import from `crate::space`)

**Approach:**
- `RotationMap`/`TranslationMap` per the design sketch; `SearchSpace::new(start, range, rotation: RotationRepresentation, translation: TranslationRepresentation)` — plain args, **no `&DirectSettings` parameter**, no `direct::` imports anywhere in `space.rs`. Starting rotation computed only for `AxisAngle`; camera basis only for `CameraCentered` (`Unit::try_new_normalize` + `+Z` fallback with a doc comment naming R12 as the declared exception).
- `physical_pose(&self, unit: UnitPose) -> PhysicalPose` — port of `physical_pose_for_eval`, byte-for-byte same arithmetic, reading `self.start/self.range` instead of optimizer fields.
- Style enums are *moved*, not duplicated; `direct/settings.rs` and `lib.rs` re-export for callers.

**Test scenarios:**
- Happy path: Euler+Euclidean (defaults) reproduces the old mapping — the existing `domain_affine_equivariance` property covers it once U8 migrates.
- Edge case: zero starting translation under `CameraCentered` yields an all-finite physical pose (basis = +Z fallback, no NaN) — new targeted unit test in `space.rs`'s sibling test module; this is the R12 exception asserted, not assumed.
- Happy path: `AxisAngle` starting rotation computed once at construction — the SO(3) equivariance suite in `direct/test.rs` exercises the map after migration.

**Verification:** `grep "use crate::direct" rust/direct-rs/src/space.rs` → empty. The axis-angle proptest suite passes unchanged after U5 migrates call sites (same rotation, same tolerance).

---

- [x] U3. **`settings.rs` renames, ceremony, payload refinement, strategy dispatch; `cost.rs` contract**

**Goal:** One settings vocabulary where disabled stages cannot carry tuning; `PohStrategy::select`; `production()` absorbs the FFI hardcodes; `Cost` gains its contract doc + `eval_one`.

**Requirements:** R2, R5, R6, A4, E1

**Dependencies:** U2

**Files:**
- Modify: `rust/direct-rs/src/direct/settings.rs`
- Modify: `rust/direct-rs/src/direct/poh.rs` (delete `select_potentially_optimal`; `POHPoint` derives `Debug, PartialEq`)
- Modify: `rust/direct-rs/src/cost.rs` (batch-contract doc + `&[PhysicalPose]` + `eval_one` default method)

**Approach:**
- Field/type renames per R5; derive set `Debug, Clone, Copy, PartialEq, Eq, Default`; drop discriminants; `MinBoxSize::uniform`.
- `enum Refinement { #[default] None, Bobyqa(BobyqaSettings) }` with `BobyqaSettings` deriving `Debug, Clone, Copy, PartialEq` plus a **manual `impl Default`** returning `{ max_evals: 500, rho_beg: 0.5, rho_end: 1e-3, npt: 28 }` — a derived `Default` would zero the radii (trips basin's assert) and `max_evals` (refines nothing). No standalone `bobyqa` field; `DirectSettings::production(use_bobyqa)` sets the variant per R5's snippet.
- `impl PohStrategy { pub fn select(self, &[POHPoint]) -> Vec<POHPoint> }` delegating to `poh::convex_hull`/`poh::pareto_front`.
- `cost.rs`: trait doc states the one-cost-per-pose, order-preserved contract + NaN-means-invalid semantics; `eval_one` per R2's sketch.

**Test scenarios:**
- Happy path: `DirectSettings::production(true).refinement` is `Refinement::Bobyqa` with each of the four default knobs matching today's hardcoded constants; other five fields equal the old `new_rust_opt` inline struct values.
- Edge case: `production(false)` → `Refinement::None`.
- `PohStrategy::select` on empty candidates returns empty (both arms).
- Error path: a `Cost` impl returning zero/two costs for one pose trips `eval_one`'s assert (test with a deliberately wrong mini-impl in `direct/test.rs`'s test module or a sibling).

**Verification:** grep shows zero remaining `poh_selection_strategy|rotation_style|translation_style|POHSettings|RefinementOptions` references.

---

- [x] U4. **`tree.rs`: UnitPose centers, consolidated trisect on `Direction`**

**Goal:** Tree speaks `UnitPose` and `Direction`; production and tests share one trisection.

**Requirements:** R1, R9

**Dependencies:** U1–U3

**Files:**
- Modify: `rust/direct-rs/src/direct/tree.rs`
- Modify: `rust/direct-rs/src/direct/mod.rs` (`trisect_and_return_unscored` calls `parent.trisect(dir)` with `split_axis(..) -> Option<Direction>`; `Hyperbox` construction sites; seed uses `UnitPose`; `unit_center() -> UnitPose`; `DIRECTIONS[..]` lookups disappear)
- Modify: `rust/direct-rs/src/direct/tree/test.rs` (wrappers for centers; `assert_eq!(axis, 1)` → `assert_eq!(axis, Direction::Y_DIR)`)

**Approach:**
- `trisect(self, axis: Direction)`: increment via `self.depths[axis.index()]`, shift via `posc.shift(axis, shift)` — the old `get_mut` guard disappears entirely (out-of-range is inexpressible per R9); return shape unchanged.
- `longest_axis() -> Direction`; production `split_axis(..) -> Option<Direction>` (filter axes by `settings.min_box_size` through `width_at(dir, depth)`, pick min depth) — the two selectors now share the tree's one trisection routine.
- Production inline duplicate deleted; behavior identical (verified line-by-line this session).

**Test scenarios:**
- Existing lattice tests must pass bit-identically (they now assert the *same code path* production runs — the A3 drift hazard is closed structurally).
- Happy path: `trisect` on each of the six `Direction` variants increments exactly that depth.
- Note: the former "trisect panics at axis 6" documentation scenario is retired — R9 makes that state unrepresentable.

**Verification:** `volume_partition_holds_for_random_runs` and `every_box_center_is_on_the_trisection_lattice` pass unchanged after U8 test migration.

---

- [x] U5. **`direct/mod.rs`: Incumbent, SearchSpace field, refine() with Result+firewall, deletions, idioms**

**Goal:** The driver becomes DIRECT-only; the R2/R4/R7/R10/R11 items land here.

**Requirements:** R2, R4, R6, R7, R9(consumers), R10, R11, E2

**Dependencies:** U1–U4

**Files:**
- Modify: `rust/direct-rs/src/direct/mod.rs`

**Approach:**
- Fields: `range`/`starting_point`/`starting_rotation`/`translation_basis`/`call_offset`/`print_resolution_summary`/`physical_pose_for_eval` removed; `space: SearchSpace` added (private field, constructed from the plain `settings.rotation`/`settings.translation` values); `boxes: pub(crate)`; `current_best: Incumbent`.
- `from_settings`: builds `SearchSpace::new(starting_point, range, settings.rotation, settings.translation)`; `current_best: Incumbent { pose: starting_point, cost: f64::INFINITY }`.
- `run`: seed via `cost.eval_one(physical)`; guard `self.calls >= self.budget`; `poh = self.settings.poh_strategy.select(&candidates)`; tail collapses to `current_best.cost.is_finite()`; the BOBYQA block is replaced by `self.refine(cost);`.
- `refine(&mut self, cost)` new: `match self.settings.refinement { Refinement::None => {}, Refinement::Bobyqa(bobyqa) => { best_unit scan (unchanged iterator chain); let boxed = catch_unwind(AssertUnwindSafe(|| run_bobyqa(&self.space, cost, best_unit, bobyqa))); match { Ok(Ok(r)) => accumulate r.evals + maybe update incumbent from r.pose/r.cost, Ok(Err(e)) => eprintln!(.."{e}"), Err(_) => eprintln!("BOBYQA panicked; keeping DIRECT result") } } }` — per the HLT sketch; containment is deliberate (R4).
- `score_and_reinsert`: `assert_eq!(evaluated_costs.len(), centers.len(), "Cost::eval must return one cost per pose")`; `.zip` method + tuple destructuring; `Vec::with_capacity(2 * poh.len())` on `unscored`; incumbent updates via `Incumbent` fields.
- `physical_width` → `self.space.width_at(dir, hb.depths[dir.index()])` — the `SearchSpace` delegator, not field access (R3 privacy: `direct/` never touches `space.range`); `split_axis`/`trisect` plumbing per U4.
- `get_potentially_optimal_candidates`: `filter_map().collect()`.
- `next_box_id` untouched (pure move, not a review item).

**Execution note:** This unit is where `cargo check --lib` should turn green (bridge/basin lag until U6–U7; check `--lib` after U7, not before).

**Test scenarios:**
- Happy path: `boxes_tile_the_unit_cube_exactly` (volume checksum) passes — proves the batch assert didn't change scoring.
- Edge case: `seed_evaluation_counts_as_one_call_when_budget_is_zero` passes (seed precedes the simplified guard).
- Integration: `refine` BOBYQA arm with a real cost returns an incumbent no worse than DIRECT-only (same as today; behavior parity to the pre-change `catch_unwind`-wrapped `expect` path). The `Err(_)` firewall arm is not unit-testable without inducing a basin panic — accepted containment, same posture as the current code.

**Verification:** `refine()` is the crate's **only** `catch_unwind` (grep: exactly one site, wrapping `run_bobyqa`); `basin_opt` contains zero `expect`/`unwrap`; the nested `Ok(Ok)/Ok(Err)/Err` match is present and all three arms log-or-accumulate (no silent drop).

---

- [x] U6. **`basin_opt`: SearchSpace coupling, Result, eval_one**

**Goal:** The refinement adapter depends on `&SearchSpace`, not `&DirectOptimizer`; success is a typed struct; the error channel is `Result`; single-pose evaluation goes through the trait's assert.

**Requirements:** R2, R4

**Dependencies:** U2, U5

**Files:**
- Modify: `rust/direct-rs/src/basin_opt/mod.rs`

**Approach:**
- `pub(crate) struct Refined { pub pose: PhysicalPose, pub cost: f64, pub evals: u64 }` replaces the 3-tuple.
- `run_bobyqa(space: &SearchSpace, cost: &T, start: UnitPose, bobyqa: BobyqaSettings) -> Result<Refined, String>`; all four knobs are consumed from the payload — `MaxCostEvals(bobyqa.max_evals)`, `with_rho_beg(bobyqa.rho_beg)`, `with_rho_end(bobyqa.rho_end)`, `with_npt(bobyqa.npt)` (no hardcoded 500 left at any call site) — and the `.expect("BOBYQA failed")` is gone: `Executor::run()`'s `Result` maps via `map_err(|e| format!("{e:?}"))`. The Err arm is type-unreachable today (`Infallible`); kept for solver-honesty with a comment; basin's *panics* are contained upstream by `refine()`'s firewall (R4).
- `BobyqaProblem` holds `&SearchSpace`; single-pose evaluation is `self.cost.eval_one(physical)` (the `[0]` index disappears); the unit-space `x[0..5]` reads stay (basin-owned length-6 params; note in comment).

**Test scenarios:**
- Happy path: existing bobyqa-included runs behave as before; refinement integration exercised by U5's scenario (none of the convergence tests enable `production(true)`, and the `#[ignore]`d rastrigin remains ignored).
- Edge case: no test mandated for basin param length (structural, not reachable).

**Verification:** `cargo check -m ./rust/Cargo.toml --lib` green with U7.

---

- [x] U7. **`bridge.rs`: production settings and `From` marshalling**

**Goal:** The boundary module shrinks to marshalling + constructor wiring.

**Requirements:** R5, E3, R1 (bridge side)

**Dependencies:** U1, U3, U5

**Files:**
- Modify: `rust/direct-rs/src/bridge.rs`

**Approach:**
- `new_rust_opt`: `DirectSettings::production(use_bobyqa)`; `Pose::from(range).into()` / `Pose::from(starting_point).into()` for `PoseRange`/`PhysicalPose` — replaces the two 12-line literals and the 24-line inline settings struct.
- `Cost for CppCost`: signature `&[PhysicalPose]`; flattener `poses.iter().flat_map(|p| <[f64; 6]>::from(*p)).collect()` (or `p.0` per the Deref decision — whichever the compiler prefers for the Copy copy).
- `run_rust_opt`: `let Incumbent { pose: best_pose, cost: best_cost } = self.best();`, marshalling via `From<PhysicalPose> for [f64; 6]` (or `best_pose.to_array()` through deref).

**Test scenarios:** Test expectation: none — pure marshalling; behavior proven by `cargo check` + the C++ link (out of scope here) and by U8's suite.

**Verification:** bridge function signatures unchanged; corrosion/cxx regeneration unaffected (`cargo check` lib target passes, meaning the `#[cxx::bridge]` expansion still type-checks).

---

- [x] U8. **Test-suite and fixture migration (`fixtures.rs`, `direct/test.rs`, `problems.rs`, `properties.rs`, viz)**

**Goal:** All consumers compiled against the new types; destructuring renames keep every assertion intact; **mode selection moves to construction time** (external-review item 5).

**Requirements:** R1, R2 (consumption side), R8, R9(tests), E1(`assert_eq!` over `matches!`)

**Dependencies:** U1–U7

**Files:**
- Modify: `rust/direct-rs/src/fixtures.rs`
- Modify: `rust/direct-rs/src/direct/test.rs`
- Modify: `rust/direct-rs/src/problems.rs`
- Modify: `rust/direct-rs/src/properties.rs`
- Modify (as needed): `rust/direct-rs/src/direct/poh/test.rs`, `rust/direct-rs/src/direct/tree/test.rs`

**Approach:**
- `fixtures::pose/splat/zero` keep returning `Pose` (builders for all three roles); `denorm`/`invert`/`coords` signatures take the role types where known, field access via deref; `viz::snapshot_boxes`/`plot_boxes` unchanged in spirit.
- Every `let (best, cost) = opt.run(...)` → `let Incumbent { pose: best, cost } = opt.run(...)`; `.1` accesses → `.cost`; `DirectOptimizer::new(range, start, budget)` call sites pass `.into()` conversions of `Pose`/`[f64; 6]` expressions (private tuple fields — `PoseRange(..)`/`PhysicalPose(..)` constructors are usable only inside `pose.rs`).
- **Delete `new_in_mode`** (the `opt.settings.rotation_style = rotation` post-construction mutation is exactly the stale-precompute trap R3/decisions create): `axis_angle_opt(start, range)` becomes `DirectOptimizer::from_settings(range, start, 0, DirectSettings { rotation: RotationRepresentation::AxisAngle, ..Default::default() })`.
- `axes_from_str("ZXY")` test sites → `AxisSeq::ZXY.axes()`; `matches!` style assertions → `assert_eq!`; tree-test axis asserts → `Direction::Y_DIR`.
- Sphere/Recorder test `Cost` impls adopt `&[PhysicalPose]` (and the U3 mini-impl error-path test lives here).

**Execution note:** mechanical, compiler-driven; no assertion semantics change. The two pre-existing failures and two `#[ignore]`s must remain as-is (R12).

**Test scenarios:**
- Happy path: full suite = 56 passed, 2 ignored, 2 failed-with-identical-costs before/after this unit.
- Integration: `convex_hull_matches_jones_on_unique_sizes` (4096 cases) unchanged — the POH move is pure.
- Regression (the stale-precompute guard is **behavior, not introspection**): `axis_angle_opt` constructs settings *before* the optimizer exists, and the SO(3) suite observes the mapping through `apply` — if the precomputed `RotationMap` were still `Euler` while settings claimed `AxisAngle`, `observed_local_delta_equals_requested_delta` fails on the first random case. Deliberately **no** `matches!(opt.space.rotation, ..)` from `direct/test.rs`: the field is private to `space`, and widening it for a weaker check would invert the encapsulation (round-3 review). Construction-level map asserts, if wanted at all, belong in `space/test.rs`.

**Verification:** `cargo test -m ./rust/Cargo.toml --lib` exit state matches the pre-change baseline (same failures, same messages).

---

- [x] U9. **Verification and doc currency**

**Goal:** Prove semantic preservation end-to-end and leave the crate's own docs honest.

**Requirements:** R12; `Cost` trait doc (R2); `DirectTree` invariant comment (the review's T1, cheap here: expand the existing comment to state the (size, cost, id) key rationale).

**Dependencies:** U1–U8

**Files:**
- Modify: `rust/direct-rs/src/direct/tree.rs` (comment), `rust/direct-rs/src/cost.rs` (contract doc — if not landed in U3), `rust/direct-rs/src/pose.rs`/`space.rs` (R12 exception note at the fallback site)

**Approach:** run `cargo check -m ./rust/Cargo.toml --all-targets` then the full suite; confirm `jj diff --stat` shape; the commit message names the two pre-existing failures and the R12 zero-ray exception explicitly.

**Verification:** zero compiler warnings (the reorganization's two remaining warnings cleared: `print_resolution_summary`, `Range`); clippy delta limited to the pre-existing `indexing_slicing`/cxx-macro families; suite state matches baseline; **the only intentional numerical-behavior difference is R12's zero-ray fallback** — every other failure/pass/cost matches bit-for-bit.

## System-Wide Impact

- **Interaction graph:** `Cost` consumers = `direct` loop (incl. `eval_one` seed), `basin_opt` adapter (`eval_one`), `problems`/`properties` wrappers, `bridge` FFI impl. All five change together per U-unit order; no partial-migration state compiles (see Risks).
- **Error propagation:** two channels, one policy. Our `Result` (`Ok(Err)`) and basin's internal panics (contained by `refine()`'s firewall) both degrade to "log, keep DIRECT incumbent"; neither unwinds toward the FFI frames. Accepted tradeoff: an `eval_one` contract panic during refinement is *contained, not crashed* (the batch-path assert in `score_and_reinsert` still propagates loudly).
- **State lifecycle:** `call_offset` deletion is observable *only* if the C++ side ever calls `run` on a reused box with a pre-spent budget — currently impossible via the bridge (`new_rust_opt` → one `run`), and the owner manages cumulative budget through `budget`. `SearchSpace` precomputation makes post-construction setting edits a silent divergence — guarded by U8's regression scenario.
- **Unchanged invariants:** the cxx contract (`RunOutcome`, `new_rust_opt`, `run_rust_opt`), the ZXY convention tests (bit-tolerance thresholds untouched), POH selection behavior, trisection lattice math, and every optimizer numeric outcome — excepting only R12's declared zero-ray fallback.

## Dependencies / Prerequisites

- No new crates. `nalgebra` `Unit::try_new_normalize` availability confirmed in 0.35 (used in U2 fallback; behavior declared in R12).
- The C++ build (`corrosion`) must see only internal renames — bridge function signatures are unchanged (verified by U7).

## Risks & Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Partial-migration tree doesn't compile mid-plan (U1–U7 interdependent through `Cost`) | Certain by design | Low | Accept: U-units are *logical* commits; `--all-targets` green asserted at U8/U9, not between U1–U7. Sequence strictly U1→U9. |
| A "mechanical" destructure rename silently changes an assertion (e.g. `.0`/`.1` flipped) | Med | High | Compiler prevents flips; semantic drift prevented by the bit-identical suite comparison (U8 verification) — any cost drift fails the deterministic convergence/invariant tests loudly. |
| Deref-coercion makes newtypes too transparent (accidental `UnitPose → Pose` slips) | Low | Med | `DerefMut` restricted to `UnitPose` (mutable surface ≈ mutation sites); signatures are the gate; by-value `fn(Pose)` sites wrap explicitly. |
| Post-construction setting edit diverges from precomputed maps | Med if unaddressed | High | Structurally impossible in production (no setters); U8 deletes `new_in_mode` and the guard is behavioral (SO(3) suite observes the mapping) — no `space`-field visibility leak for testing. |
| `refine` extraction perturbs BOBYQA integration | Low | Med | Move-only diff; `Result` + firewall preserved per R4 with all three arms observable (grep exactly-one `catch_unwind`, zero `expect` in basin_opt); incumbent monotonicity via U5's integration scenario. |
| `eval_one` panic during refinement gets contained, masking a contract bug | Low | Med | Deliberate policy (U5 verification); the batch-path assert in `score_and_reinsert` still propagates, and the firewall logs the payload. |
| `Direction` axis flow cascades beyond the plan's file set | Low | Low | Pre-agreed bound (R9): drop the leg, record it in the commit message, keep the rest. |
| Pre-existing failures mask a new regression | Low | High | Compare *costs bit-for-bit* in failure messages, not just pass/fail counts (baseline values recorded this session: 0.0009644532506901156 / 0.005420074853088229). |

## Documentation / Operational Notes

- `Cost` trait gains the batch-contract + NaN-meaning + `eval_one` doc (R2); `DirectTree` gains the (size, cost, id) key-rationale comment (T1); `space.rs` documents R12 at the fallback site.
- Post-landing: a `docs/solutions/architecture-patterns/` entry for "unit vs physical pose newtypes + SearchSpace + Result-vs-firewall channels" is a good compound-knowledge candidate once the dust settles (not part of this plan's units).

## Sources & References

- Review origin: this session's architecture review (A1–A7, E1–E6, T1–T12) and the owner's follow-up decisions (full newtypes; `call_offset` delete; trisect consolidation; `SearchSpace` sketch; settings renames; Cost contract; translation-mode construction; "do NOT handle budget overshoot").
- External plan review (owner-relayed, 2026-09-01). Round 1: space←/→settings dependency inversion fixed by plain-arg construction; `Result` ≠ panic containment (firewall retained around basin); `Refinement` payload enum replaces mode+orphans; contract centralized via `eval_one`; zero-ray fallback declared as the single semantic exception; `space` privatized with root re-exports; `DerefMut` limited to `UnitPose`; `Direction`-typed axes adopted with a scope bound. Round 3 (same reviewer): derived `BobyqaSettings::default()` would zero the radii — manual `Default` mandated; `self.space.range`/`opt.space.rotation` accesses violate field privacy as planned — replaced by the `width_at` delegator and a behavior-based stale-precompute guard; newtype tuple fields made private so `.0` cannot bypass the role gate.
- Baseline: working copy at `jj` revision `txxrsvps` (post-reorg, pre-this-plan); failures baseline `problems::tests::shifted_sphere_reaches_min` / `anisotropic_weights_still_reach_min` with exact costs above.
- Related plan: `docs/plans/2026-09-01-016-refactor-axis-angle-property-test-audit-plan.md` (axis-angle suite that U8 must keep passing).
