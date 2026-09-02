//! cxx boundary: marshalling between the C++ host's `[f64; 6]` world and
//! this crate's typed surface. Shapes crossing the FFI stay raw; the role
//! newtypes gate everything on the Rust side.

use super::*;
use crate::bridge::ffi::RunOutcome;
use crate::cost::Cost;
use crate::direct::settings::DirectSettings;
use crate::direct::Incumbent;

#[cxx::bridge]
pub mod ffi {

    pub struct RunOutcome {
        pub num_iter: u32,
        pub optimal_value: f64,
        pub optimal_location: [f64; 6],
    }

    #[namespace = "direct_rs"]
    extern "Rust" {
        type DirectOptimizer;

        pub fn new_rust_opt(
            range: [f64; 6],
            starting_point: [f64; 6],
            budget: u32,
            use_bobyqa: bool,
        ) -> Box<DirectOptimizer>;

        pub fn run_rust_opt(self: &mut DirectOptimizer, cost: &CppCost) -> RunOutcome;

    }

    unsafe extern "C++" {
        include!("domain/cost.h");
        include!("domain/data_structures_6D.h");

        pub(crate) type CppCost;
        type Point6D;

        pub fn evaluate(self: &CppCost, point: &Point6D) -> f64;
        pub fn evaluate_batch(self: &CppCost, flat_poses: Vec<f64>) -> Vec<f64>;

        #[Self=Point6D]
        pub fn new_point(x: f64, y: f64, z: f64, xa: f64, ya: f64, za: f64) -> UniquePtr<Point6D>;
    }
}

impl Cost for CppCost {
    fn eval(&self, poses: &[PhysicalPose]) -> Vec<f64> {
        let flat_poses: Vec<f64> = poses
            .iter()
            .flat_map(|p| <[f64; 6]>::from(*p))
            .collect();
        return self.evaluate_batch(flat_poses);
    }
}

pub fn new_rust_opt(
    range: [f64; 6],
    starting_point: [f64; 6],
    budget: u32,
    use_bobyqa: bool,
) -> Box<DirectOptimizer> {
    return Box::new(DirectOptimizer::from_settings(
        range.into(),
        starting_point.into(),
        budget,
        DirectSettings::production(use_bobyqa),
    ));
}
/// FFI entry point: run DIRECT against a C++ cost, marshaling the result
/// into the C++-facing `RunOutcome`.
impl DirectOptimizer {
    pub fn run_rust_opt(&mut self, cost: &CppCost) -> RunOutcome {
        self.run(cost);
        let Incumbent {
            pose: best_pose,
            cost: best_cost,
        } = self.best();
        return RunOutcome {
            num_iter: self.calls,
            optimal_value: best_cost,
            optimal_location: best_pose.to_array(),
        };
    }
}
