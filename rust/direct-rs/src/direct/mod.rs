use std::time::{self, Duration};

use nalgebra::{self as na, Rotation3, Unit, Vector3};
use ordered_float::OrderedFloat;

use crate::{
    cost::Cost,
    direct::{
        geometry::{POHPoint, Pose, DIRECTIONS},
        settings::{
            DirectSettings, POHSettings, RefinementOptions, RotationRepresentation,
            TranslationRepresentation,
        },
        tree::{DirectTree, Hyperbox, UnscoredHyperbox},
    },
};

pub(crate) mod geometry;
pub(crate) mod poh;
pub mod settings;
pub(crate) mod tree;

#[cfg(test)]
mod problems;

#[cfg(test)]
mod fixtures;

pub struct DirectOptimizer {
    pub boxes: DirectTree,
    current_best: (Pose, f64),
    budget: u32,
    range: Pose,
    starting_point: Pose,
    call_offset: u32,
    calls: u32,
    next_box_id: u64,
    starting_rotation: Rotation3<f64>,
    translation_basis: na::Matrix3<f64>,
    settings: DirectSettings,
}

impl DirectOptimizer {
    pub fn new(range: Pose, starting_point: Pose, budget: u32) -> Self {
        Self::from_settings(range, starting_point, budget, DirectSettings::default())
    }
    pub fn from_settings(
        range: Pose,
        starting_point: Pose,
        budget: u32,
        settings: DirectSettings,
    ) -> Self {
        let starting_rotation = geometry::from_euler_ordered(
            "ZXY",
            [starting_point.za, starting_point.xa, starting_point.ya],
            false,
        );
        let ray_vec = Unit::new_normalize(Vector3::new(
            starting_point.x,
            starting_point.y,
            starting_point.z,
        ));

        let translation_basis =
            na::Matrix3::from_columns(&[Vector3::x(), Vector3::y(), ray_vec.into_inner()]);

        return Self {
            boxes: DirectTree::new(),
            current_best: (starting_point, f64::INFINITY),
            budget,
            range,
            starting_point,
            call_offset: 0,
            calls: 0,
            next_box_id: 0,
            starting_rotation,
            translation_basis,
            settings,
        };
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_box_id;
        self.next_box_id += 1;
        id
    }

    fn sort_cost(cost: f64) -> f64 {
        if cost.is_finite() {
            cost
        } else {
            f64::INFINITY
        }
    }

    pub fn run<T: Cost>(&mut self, cost: &T) -> (Pose, f64) {
        // seed: box lives at the unit center; cost eval at its physical pose
        let start = time::Instant::now();

        let unit = Self::unit_center();
        // println!("{:?}", self.starting_rotation);
        let physical = self.physical_pose_for_eval(unit);
        // println!(
        //     "{:?}",
        //     Self::from_euler_ordered("ZXY", [physical.za, physical.xa, physical.ya], false)
        // );

        let seed_cost = *cost
            .eval(&[physical])
            .first()
            .expect("Cost must be returned");
        self.calls += 1;
        let seed = Hyperbox {
            cost_at_center: seed_cost,
            center: unit,
            depths: [0; 6],
        };
        let id = self.next_id();
        self.boxes
            .entry(OrderedFloat(seed.size()))
            .or_default()
            .insert((OrderedFloat(Self::sort_cost(seed_cost)), id), seed);

        if seed_cost.is_finite() && seed_cost < self.current_best.1 {
            self.current_best = (physical, seed_cost);
        }

        loop {
            if self.calls + self.call_offset >= self.budget {
                break;
            }

            let candidates = self.get_potentially_optimal_candidates();
            let poh =
                self.select_potentially_optimal(&candidates, &self.settings.poh_selection_strategy);

            if poh.is_empty() {
                break;
            }
            let unscored = self.trisect_and_return_unscored(&poh);
            if unscored.is_empty() {
                break;
            }
            self.score_and_reinsert(cost, &unscored);
        }
        let elapsed = start.elapsed();
        let avg_per_call: Duration = elapsed / self.calls;
        let it_per_sec = self.calls as f64 / elapsed.as_secs_f64();
        println!("{:?} per iteration", avg_per_call);
        println!(
            "{:?} iterations/second for {:?} iterations",
            it_per_sec, self.calls
        );

        match self.settings.refinement {
            RefinementOptions::NoRefinement => {}
            RefinementOptions::BOBYQA => {
                let best_unit = self
                    .boxes
                    .values()
                    .flat_map(|row| row.values())
                    .filter(|hb| hb.cost_at_center.is_finite())
                    .min_by(|a, b| a.cost_at_center.total_cmp(&b.cost_at_center))
                    .map(|hb| hb.center);

                if let Some(best_unit) = best_unit {
                    let bobyqa_result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            crate::basin_opt::run_bobyqa(self, cost, best_unit, 500)
                        }));

                    match bobyqa_result {
                        Ok((pose, cost_value, evals)) => {
                            self.calls += evals as u32;

                            if cost_value.is_finite() && cost_value < self.current_best.1 {
                                self.current_best = (pose, cost_value);
                            }
                        }
                        Err(_) => {
                            eprintln!("BOBYQA panicked; keeping DIRECT result");
                        }
                    }
                }
            }
        }

        if (self.current_best.1.is_finite()) && (!self.current_best.1.is_nan()) {
            return self.best();
        } else {
            return (physical, f64::INFINITY);
        }
    }

    pub fn run_rust_opt(&mut self, cost: &CppCost) -> RunOutcome {
        self.run(cost);
        let (best_pose, best_cost) = self.best();
        return RunOutcome {
            num_iter: self.calls,
            optimal_value: best_cost,
            optimal_location: best_pose.to_array(),
        };
    }

    pub fn best(&self) -> (Pose, f64) {
        self.current_best
    }

    fn physical_width(&self, hb: &Hyperbox, axis: usize) -> f64 {
        let ranges = self.range.to_array();

        2.0 * ranges[axis].abs() * 3f64.powi(-(hb.depths[axis] as i32))
    }
    fn print_resolution_summary(&self) {
        let names = ["X", "Y", "Z", "XA", "YA", "ZA"];

        println!("--- DIRECT resolution summary ---");

        for axis in 0..6 {
            let Some(min_size) = self.settings.min_box_size.values[axis] else {
                continue;
            };

            let smallest = self
                .boxes
                .values()
                .flat_map(|row| row.values())
                .map(|hb| self.physical_width(hb, axis))
                .fold(f64::INFINITY, f64::min);

            println!(
                "{:>2}: smallest={:.6}, target={:.6}, ratio={:.2}x",
                names[axis],
                smallest,
                min_size,
                smallest / min_size,
            );
        }
    }
    fn split_axis(&self, hb: &Hyperbox) -> Option<usize> {
        hb.depths
            .iter()
            .enumerate()
            .filter(|(axis, _)| {
                self.settings.min_box_size.values[*axis]
                    .is_none_or(|min_size| self.physical_width(hb, *axis) > min_size)
            })
            .min_by_key(|(_, depth)| **depth)
            .map(|(axis, _)| axis)
    }

    /// Map a unit-space pose (each axis in [0,1]) to physical space.
    /// physical[i] = start[i] + (unit[i] - 0.5) * 2 * range[i]
    pub(crate) fn physical_pose_for_eval(&self, unit: Pose) -> Pose {
        let (xa, ya, za) = match self.settings.rotation_style {
            // Euler rotations are a basic denormalization
            RotationRepresentation::Euler => (
                self.starting_point.xa + (unit.xa - 0.5) * 2.0 * self.range.xa,
                self.starting_point.ya + (unit.ya - 0.5) * 2.0 * self.range.ya,
                self.starting_point.za + (unit.za - 0.5) * 2.0 * self.range.za,
            ),

            // In axis angle, we're taking hyperbox location as a further pose applied to
            // the original point. Denormalization alone doesn't get you physical
            // rotation, you must further apply rotations
            RotationRepresentation::AxisAngle => {
                // println!("{:?}", unit);
                let denormed_xa = (unit.xa - 0.5) * 2.0 * self.range.xa;
                let denormed_ya = (unit.ya - 0.5) * 2.0 * self.range.ya;
                let denormed_za = (unit.za - 0.5) * 2.0 * self.range.za;

                let applied_rot_t = na::Rotation3::from_scaled_axis(Vector3::new(
                    denormed_xa.to_radians(),
                    denormed_ya.to_radians(),
                    denormed_za.to_radians(),
                ));

                let applied_rot = self.starting_rotation * applied_rot_t;
                let (final_angles, _observable) =
                    applied_rot.euler_angles_ordered(geometry::axes_from_str("ZXY"), false);
                let [za, xa, ya] = final_angles;

                (xa.to_degrees(), ya.to_degrees(), za.to_degrees())
            }
        };

        let (x, y, z) = match self.settings.translation_style {
            TranslationRepresentation::PureEuclidean => (
                self.starting_point.x + (unit.x - 0.5) * 2.0 * self.range.x,
                self.starting_point.y + (unit.y - 0.5) * 2.0 * self.range.y,
                self.starting_point.z + (unit.z - 0.5) * 2.0 * self.range.z,
            ),
            TranslationRepresentation::CameraCentered => {
                let local_translation = Vector3::new(
                    (unit.x - 0.5) * 2.0 * self.range.x,
                    (unit.y - 0.5) * 2.0 * self.range.y,
                    (unit.z - 0.5) * 2.0 * self.range.z,
                );
                let world_translation = self.translation_basis * local_translation;
                (
                    self.starting_point.x + world_translation.x,
                    self.starting_point.y + world_translation.y,
                    self.starting_point.z + world_translation.z,
                )
            }
        };

        return Pose {
            x,
            y,
            z,
            xa,
            ya,
            za,
        };
    }

    fn unit_center() -> Pose {
        Pose {
            x: 0.5,
            y: 0.5,
            z: 0.5,
            xa: 0.5,
            ya: 0.5,
            za: 0.5,
        }
    }
    fn trisect_and_return_unscored(&mut self, boxes: &[POHPoint]) -> Vec<UnscoredHyperbox> {
        let mut unscored: Vec<UnscoredHyperbox> = Vec::new();
        for poh in boxes {
            let size_key = OrderedFloat(poh.size);

            let parent_key = self.boxes.get(&size_key).and_then(|row| {
                row.iter()
                    .find(|(_, hb)| self.split_axis(hb).is_some())
                    .map(|(key, _)| *key)
            });

            let Some(parent_key) = parent_key else {
                continue;
            };

            let parent = self
                .boxes
                .get_mut(&size_key)
                .and_then(|row| row.remove(&parent_key))
                .expect("selected refinable box must still exist");
            let axis = self
                .split_axis(&parent)
                .expect("selected parent must be refinable");

            let (center, shifted) = {
                let mut this = parent;
                this.depths[axis] += 1;
                let shift = 3f64.powi(-(this.depths[axis] as i32));
                let mut posc = this.center;
                let mut negc = this.center;
                posc.shift(&DIRECTIONS[axis], shift);
                negc.shift(&DIRECTIONS[axis], -shift);
                let pos_shift = UnscoredHyperbox {
                    center: posc,
                    depths: this.depths,
                };
                let neg_shift = UnscoredHyperbox {
                    center: negc,
                    depths: this.depths,
                };
                (this, [pos_shift, neg_shift])
            };

            let id = self.next_id();
            self.boxes
                .entry(OrderedFloat(center.size()))
                .or_default()
                .insert(
                    (OrderedFloat(Self::sort_cost(center.cost_at_center)), id),
                    center,
                );
            unscored.extend(shifted);
        }
        return unscored;
    }

    fn score_and_reinsert<T: Cost>(&mut self, cost: &T, unscored: &[UnscoredHyperbox]) {
        let centers: Vec<Pose> = unscored
            .iter()
            .map(|p| self.physical_pose_for_eval(p.center))
            .collect();
        let evaluated_costs: Vec<f64> = cost.eval(&centers);
        self.calls += evaluated_costs.len() as u32;

        for scored_box in zip(unscored, evaluated_costs).map(|v| v.0.add_score(v.1)) {
            let id = self.next_id();
            if scored_box.cost_at_center.is_finite() {
                let physical = self.physical_pose_for_eval(scored_box.center);
                if scored_box.cost_at_center < self.current_best.1 {
                    self.current_best = (physical, scored_box.cost_at_center);
                }
                self.boxes
                    .entry(OrderedFloat(scored_box.size()))
                    .or_default()
                    .insert(
                        (OrderedFloat(Self::sort_cost(scored_box.cost_at_center)), id),
                        scored_box,
                    );
            }
        }
    }

    fn get_potentially_optimal_candidates(&self) -> Vec<POHPoint> {
        let mut candidates = Vec::new();

        for (size_key, row) in &self.boxes {
            if let Some((key, _box)) = row.iter().find(|(_, hb)| self.split_axis(hb).is_some()) {
                candidates.push(POHPoint {
                    size: size_key.into_inner(),
                    cost: key.0.into_inner(),
                });
            }
        }

        return candidates;
    }

    fn select_potentially_optimal(
        &self,
        candidates: &[POHPoint],
        settings: &POHSettings,
    ) -> Vec<POHPoint> {
        let poh = match settings {
            POHSettings::ConvexHull => DirectOptimizer::convex_hull(candidates),
            POHSettings::Pareto => DirectOptimizer::pareto_front(candidates),
        };
        return poh;
    }

    fn pareto_front(candidates: &[POHPoint]) -> Vec<POHPoint> {
        let mut pts: Vec<POHPoint> = candidates.to_vec();
        // largest size first; cost ascending breaks ties so equal-size points
        // (shouldn't occur post-dedup, but be defensive) prefer the cheaper one
        pts.sort_by(|a, b| b.size.total_cmp(&a.size).then(a.cost.total_cmp(&b.cost)));

        let mut front = Vec::with_capacity(pts.len());
        let mut best_cost = f64::INFINITY;
        for p in pts {
            if p.cost.is_finite() && p.cost < best_cost {
                front.push(p);
                best_cost = p.cost;
            }
        }
        front.reverse(); // back to ascending size, matching convex_hull's convention
        return front;
    }

    pub(crate) fn convex_hull(candidates: &[POHPoint]) -> Vec<POHPoint> {
        let mut hull: Vec<POHPoint> = Vec::new();

        if candidates.len() < 2 {
            hull.extend(candidates);
            return hull;
        }
        for p in candidates.iter() {
            while (hull.len() >= 2)
                && (DirectOptimizer::cross(
                    (hull[hull.len() - 2].size, hull[hull.len() - 2].cost),
                    (hull[hull.len() - 1].size, hull[hull.len() - 1].cost),
                    (p.size, p.cost),
                ) < 0.0)
            {
                hull.pop();
            }
            hull.push(*p);
        }
        let cut = hull
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.cost.total_cmp(&b.cost).then(b.size.total_cmp(&a.size)))
            .map(|(i, _)| i)
            .unwrap_or(0);
        return hull.split_off(cut);
    }

    fn cross(o: (f64, f64), p1: (f64, f64), p2: (f64, f64)) -> f64 {
        return (p1.0 - o.0) * (p2.1 - o.1) - (p1.1 - o.1) * (p2.0 - o.0);
    }
}
