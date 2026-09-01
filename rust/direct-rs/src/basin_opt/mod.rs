use std::convert::Infallible;

use basin::{Bobyqa, BobyqaState, BoxConstraints, CostFunction, Executor, MaxCostEvals};

use crate::{
    cost::Cost,
    direct::{geometry::Pose, DirectOptimizer},
};

struct BobyqaProblem<'a, T> {
    direct: &'a DirectOptimizer,
    cost: &'a T,
    lower: Vec<f64>,
    upper: Vec<f64>,
}

impl<T: Cost> CostFunction for BobyqaProblem<'_, T> {
    type Param = Vec<f64>;
    type Output = f64;
    type Error = Infallible;

    fn cost(&self, x: &Self::Param) -> Result<f64, Self::Error> {
        let unit = Pose {
            x: x[0],
            y: x[1],
            z: x[2],
            xa: x[3],
            ya: x[4],
            za: x[5],
        };

        let physical = self.direct.physical_pose_for_eval(unit);
        let value = self.cost.eval(&[physical])[0];

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
    direct: &DirectOptimizer,
    cost: &T,
    start: Pose,
    max_evals: u64,
) -> (Pose, f64, u64) {
    let problem = BobyqaProblem {
        direct,
        cost,
        lower: vec![0.0; 6],
        upper: vec![1.0; 6],
    };

    let x0 = start.to_array().to_vec();

    let solver = Bobyqa::new()
        .with_rho_beg(0.5)
        .with_rho_end(1e-3)
        .with_npt(28);

    let result = Executor::new(problem, solver, BobyqaState::new(x0))
        .terminate_on(MaxCostEvals(max_evals))
        .run()
        .expect("BOBYQA failed");

    let x = result.best_param();

    let unit = Pose {
        x: x[0],
        y: x[1],
        z: x[2],
        xa: x[3],
        ya: x[4],
        za: x[5],
    };

    let physical = direct.physical_pose_for_eval(unit);

    (physical, result.best_cost(), result.cost_evals())
}
