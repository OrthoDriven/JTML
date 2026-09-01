use super::*;
use crate::{cost::Cost, direct::settings::MinBoxSize};
use crate::bridge::ffi::RunOutcome;

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
    fn eval(&self, poses: &[Pose]) -> Vec<f64> {
        let mut flat_poses: Vec<f64> = Vec::with_capacity(poses.len() * 6);
        for pose in poses {
            flat_poses.push(pose.x);
            flat_poses.push(pose.y);
            flat_poses.push(pose.z);
            flat_poses.push(pose.xa);
            flat_poses.push(pose.ya);
            flat_poses.push(pose.za);
        }
        return self.evaluate_batch(flat_poses);
    }
}

pub fn new_rust_opt(
    range: [f64; 6],
    starting_point: [f64; 6],
    budget: u32,
    use_bobyqa: bool,
) -> Box<DirectOptimizer> {
    let settings = DirectSettings {
        poh_selection_strategy: POHSettings::Pareto,
        min_box_size: MinBoxSize {
            values: [
                Some(0.5),
                Some(0.5),
                Some(0.5),
                Some(0.5),
                Some(0.5),
                Some(0.5),
            ],
        },
        rotation_style: RotationRepresentation::AxisAngle,
        translation_style: TranslationRepresentation::CameraCentered,
        refinement: match use_bobyqa {
            true => RefinementOptions::BOBYQA,
            false => RefinementOptions::NoRefinement,
        },
    };
    return Box::new(DirectOptimizer::from_settings(
        Pose {
            x: range[0],
            y: range[1],
            z: range[2],
            xa: range[3],
            ya: range[4],
            za: range[5],
        },
        Pose {
            x: starting_point[0],
            y: starting_point[1],
            z: starting_point[2],
            xa: starting_point[3],
            ya: starting_point[4],
            za: starting_point[5],
        },
        budget,
        settings,
    ));
}
/// FFI entry point: run DIRECT against a C++ cost, marshaling the result
/// into the C++-facing `RunOutcome`.
impl DirectOptimizer {
    pub fn run_rust_opt(&mut self, cost: &CppCost) -> RunOutcome {
        self.run(cost);
        let (best_pose, best_cost) = self.best();
        return RunOutcome {
            num_iter: self.calls,
            optimal_value: best_cost,
            optimal_location: best_pose.to_array(),
        };
    }
}
