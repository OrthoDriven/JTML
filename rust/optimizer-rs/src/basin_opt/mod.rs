//! BOBYQA basin-refinement adapter over `basin`.
//!
//! Depends on `&SearchSpace` (the unit↔physical mapping), not on
//! `DirectOptimizer` — refinement is a peer stage at the driver level, not
//! a DIRECT subordinate. Errors travel on two distinct channels:
//!
//! - Our own channel: `Result<Refined, String>` — the
//!   `Executor::run() -> Result` leg, mapped to a `String` here. The `Err`
//!   arm is type-unreachable for the `Infallible` problem error today; it
//!   is kept for solver-honesty should basin's error surface grow.
//! - Basin's channel: internal `panic!`s (rho ordering, model validity,
//!   trust-region step). Those are contained UPSTREAM by `refine()`'s
//!   `catch_unwind` firewall — this module deliberately has no
//!   `expect`/`unwrap` of its own.

use std::convert::Infallible;

use basin::{Bobyqa, BobyqaState, BoxConstraints, CostFunction, Executor, MaxCostEvals};

use crate::{
    cost::Cost,
    direct::settings::BobyqaSettings,
    pose::{PhysicalPose, UnitPose},
    space::SearchSpace,
};

/// Successful refinement: the incumbent candidate, in physical roles.
pub(crate) struct Refined {
    pub pose: PhysicalPose,
    pub cost: f64,
    pub evals: u64,
}

struct BobyqaProblem<'a, T> {
    space: &'a SearchSpace,
    cost: &'a T,
    lower: Vec<f64>,
    upper: Vec<f64>,
}

impl<T: Cost> CostFunction for BobyqaProblem<'_, T> {
    type Param = Vec<f64>;
    type Output = f64;
    type Error = Infallible;

    fn cost(&self, x: &Self::Param) -> Result<f64, Self::Error> {
        // basin owns this length-6 parameter vector: unit-space coordinates
        // [x, y, z, xa, ya, za], each nominally in [0, 1].
        let unit = UnitPose::from([x[0], x[1], x[2], x[3], x[4], x[5]]);

        let physical = self.space.physical_pose(unit);

        // eval_one centralizes the batch 1:1 contract assert (R2). A
        // violation panics here; during refinement that panic is contained
        // by refine()'s firewall (logged, DIRECT incumbent kept).
        let value = self.cost.eval_one(physical);

        Ok(value)
    }
}

impl<T: Cost> BoxConstraints for BobyqaProblem<'_, T> {
    fn lower(&self) -> &Self::Param {
        &self.lower
    }

    fn upper(&self) -> &Self::Param {
        &self.upper
    }
}

pub(crate) fn run_bobyqa<T: Cost>(
    space: &SearchSpace,
    cost: &T,
    start: UnitPose,
    bobyqa: BobyqaSettings,
) -> Result<Refined, String> {
    let problem = BobyqaProblem {
        space,
        cost,
        lower: vec![0.0; 6],
        upper: vec![1.0; 6],
    };

    let x0 = start.to_array().to_vec();

    let solver = Bobyqa::new()
        .with_rho_beg(bobyqa.rho_beg)
        .with_rho_end(bobyqa.rho_end)
        .with_npt(bobyqa.npt);

    let result = Executor::new(problem, solver, BobyqaState::new(x0))
        .terminate_on(MaxCostEvals(bobyqa.max_evals))
        .run()
        .map_err(|e| format!("{e:?}"))?;

    let x = result.best_param();

    let unit = UnitPose::from([x[0], x[1], x[2], x[3], x[4], x[5]]);

    let physical = space.physical_pose(unit);

    Ok(Refined {
        pose: physical,
        cost: result.best_cost(),
        evals: result.cost_evals(),
    })
}
