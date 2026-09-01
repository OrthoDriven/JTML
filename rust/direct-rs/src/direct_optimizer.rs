use crate::cost::Cost;
use crate::direct_data_storage::{DirectTree, Hyperbox, MinBoxSize, UnscoredHyperbox};
use crate::direct_data_storage::{Pose, DIRECTIONS};
use crate::ffi::{CppCost, RunOutcome};
use nalgebra::{self as na, Matrix3, Rotation3, Unit, UnitVector3, Vector3};
use ordered_float::OrderedFloat;
use std::collections::BTreeMap;
use std::iter::zip;
use std::time::{self, Duration};

#[derive(Default)]
pub enum RotationRepresentation {
    AxisAngle = 0,
    #[default]
    Euler = 1,
}

#[derive(Default)]
pub enum TranslationRepresentation {
    // This defines how we want to structure the translation component of the pose
    // optimization given the following intuition: When a human is optimizing the pose,
    // there is roughly an x/y component for in-plane translation, and a "size" component.
    // Because of perspective projection, the "size" component only maps onto the "Z" part
    // of the translation when the object is at the principal point. Thus, we're going to
    // make it possible to have the "Z" of the pose (in hyperbox space) represent a
    // translation along the ray from the camera-->object, such that adjusting this
    // parameter matches more closely to size.
    #[default]
    PureEuclidean = 0,
    CameraCentered = 1,
}

#[derive(Default)]
pub enum RefinementOptions {
    #[default]
    NoRefinement = 0,
    BOBYQA = 1,
}

#[derive(Default)]
pub struct DirectSettings {
    pub poh_selection_strategy: POHSettings,
    pub min_box_size: MinBoxSize,
    pub rotation_style: RotationRepresentation,
    pub translation_style: TranslationRepresentation,
    pub refinement: RefinementOptions,
}

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

#[derive(Default)]
pub enum POHSettings {
    #[default]
    ConvexHull = 0,
    Pareto = 1,
}

#[derive(Copy, Clone)]
pub struct POHPoint {
    pub size: f64,
    pub cost: f64,
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
        let starting_rotation = Self::from_euler_ordered(
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
            boxes: BTreeMap::new(),
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
    pub fn axes_from_str(seq: &str) -> [Unit<Vector3<f64>>; 3] {
        seq.chars()
            .map(|c| match c {
                'X' | 'x' => Vector3::x_axis(),
                'Y' | 'y' => Vector3::y_axis(),
                'Z' | 'z' => Vector3::z_axis(),
                _ => panic!("invalid axis char: {c}"),
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("seq must be exactly 3 characters")
    }
    pub fn from_euler_ordered(seq: &str, angles: [f64; 3], extrinsic: bool) -> Rotation3<f64> {
        let seq_vec = Self::axes_from_str(seq);
        let r0 = Rotation3::from_axis_angle(&seq_vec[0], angles[0].to_radians());
        let r1 = Rotation3::from_axis_angle(&seq_vec[1], angles[1].to_radians());
        let r2 = Rotation3::from_axis_angle(&seq_vec[2], angles[2].to_radians());

        if extrinsic {
            // fixed-frame axes: later rotations apply about world axes,
            // so compose left-to-right in application order
            r2 * r1 * r0
        } else {
            // intrinsic (body-frame) axes: each rotation is about the
            // already-rotated frame, so compose right-to-left
            r0 * r1 * r2
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
                    applied_rot.euler_angles_ordered(Self::axes_from_str("ZXY"), false);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::ShiftedSphere;
    use crate::cost::Cost;
    use crate::test_support::{coords, on_lattice, show, splat, zero};
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    fn iter_boxes(opt: &DirectOptimizer) -> impl Iterator<Item = &Hyperbox> {
        opt.boxes.values().flat_map(|row| row.values())
    }

    /// Jones potentially-optimal test on `(size, cost)` representatives.
    /// `j` is POH iff `K_lo <= K_hi` and `K_hi > 0`, with same-size worse
    /// points treated as dominated.
    fn jones_poh(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        for (j, &(dj, fj)) in points.iter().enumerate() {
            let dominated = points
                .iter()
                .enumerate()
                .any(|(i, &(di, fi))| i != j && (di - dj).abs() <= 1e-15 && fi < fj);
            if dominated {
                continue;
            }
            let mut k_lo = f64::NEG_INFINITY;
            let mut k_hi = f64::INFINITY;
            for (i, &(di, fi)) in points.iter().enumerate() {
                if i == j {
                    continue;
                }
                let dd = dj - di;
                if dd > 1e-15 {
                    k_lo = k_lo.max((fj - fi) / dd);
                } else if dd < -1e-15 {
                    k_hi = k_hi.min((fi - fj) / (di - dj));
                }
            }
            if k_lo <= k_hi && k_hi > 0.0 {
                out.push((dj, fj));
            }
        }
        out
    }

    fn hull_pairs(pts: &[POHPoint]) -> Vec<(f64, f64)> {
        DirectOptimizer::convex_hull(pts)
            .into_iter()
            .map(|p| (p.size, p.cost))
            .collect()
    }

    fn volume_checksum(opt: &DirectOptimizer) -> Result<(u128, u128), &'static str> {
        let sums: Vec<u32> = iter_boxes(opt).map(|b| b.depths.iter().sum()).collect();
        let Some(&s_max) = sums.iter().max() else {
            return Err("no boxes");
        };
        let rhs = 3u128.checked_pow(s_max).ok_or("pow overflow")?;
        let mut lhs = 0u128;
        for s in sums {
            let term = 3u128.checked_pow(s_max - s).ok_or("pow overflow")?;
            lhs = lhs.checked_add(term).ok_or("add overflow")?;
        }
        Ok((lhs, rhs))
    }

    #[test]
    fn jones_rejects_the_descending_left_hull() {
        // Larger box is cheaper: the small expensive vertices are NOT POH.
        let pts = [
            POHPoint {
                size: 1.0,
                cost: 10.0,
            },
            POHPoint {
                size: 2.0,
                cost: 5.0,
            },
            POHPoint {
                size: 3.0,
                cost: 0.0,
            },
        ];
        let hull = hull_pairs(&pts);
        let jones = jones_poh(&[(1.0, 10.0), (2.0, 5.0), (3.0, 0.0)]);
        assert_eq!(jones, vec![(3.0, 0.0)], "oracle sanity");
        assert_eq!(
            hull, jones,
            "convex_hull returned {hull:?}, Jones POH is {jones:?} — \
             drop vertices left of the global-min-cost hull vertex"
        );
    }

    #[test]
    fn jones_keeps_the_increasing_right_hull() {
        let pts = [
            POHPoint {
                size: 1.0,
                cost: 0.0,
            },
            POHPoint {
                size: 2.0,
                cost: 1.0,
            },
            POHPoint {
                size: 3.0,
                cost: 4.0,
            },
        ];
        let mut hull = hull_pairs(&pts);
        let mut jones = jones_poh(&[(1.0, 0.0), (2.0, 1.0), (3.0, 4.0)]);
        hull.sort_by(|a, b| a.0.total_cmp(&b.0));
        jones.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(hull, jones);
    }

    #[test]
    fn hull_of_one_and_two_points() {
        let one = [POHPoint {
            size: 1.5,
            cost: 2.0,
        }];
        assert_eq!(hull_pairs(&one).len(), 1);
        let two = [
            POHPoint {
                size: 1.0,
                cost: 1.0,
            },
            POHPoint {
                size: 2.0,
                cost: 2.5,
            },
        ];
        assert_eq!(hull_pairs(&two).len(), 2);
    }

    #[test]
    fn collinear_lower_hull_keeps_interior_vertices() {
        // Jones selects every collinear lower-hull point. `cross <= 0` pops them.
        let pts = [
            POHPoint {
                size: 1.0,
                cost: 1.0,
            },
            POHPoint {
                size: 2.0,
                cost: 2.0,
            },
            POHPoint {
                size: 3.0,
                cost: 3.0,
            },
        ];
        let hull = hull_pairs(&pts);
        assert_eq!(
            hull.len(),
            3,
            "collinear interior vertex dropped ({hull:?}); \
             Jones keeps all of them — change the comparison deliberately if this is DIRECT-l"
        );
    }

    #[test]
    fn boxes_tile_the_unit_cube_exactly() {
        let mut opt = DirectOptimizer::new(splat(5.0), zero(), 80);
        opt.run(&ShiftedSphere {
            shift: Pose {
                x: 1.7,
                y: 0.4,
                z: -2.1,
                xa: 0.8,
                ya: -1.3,
                za: 2.6,
            },
        });
        let (lhs, rhs) = volume_checksum(&opt).expect("volume checksum");
        assert_eq!(lhs, rhs, "Σ 3^{{Smax-Σd}} = {lhs} != 3^{{Smax}} = {rhs}");
    }

    #[test]
    fn every_box_center_is_on_the_trisection_lattice() {
        let mut opt = DirectOptimizer::new(splat(5.0), zero(), 80);
        opt.run(&ShiftedSphere {
            shift: Pose {
                x: 1.7,
                y: 0.4,
                z: -2.1,
                xa: 0.8,
                ya: -1.3,
                za: 2.6,
            },
        });
        for hb in iter_boxes(&opt) {
            for (c, d) in coords(&hb.center).iter().zip(hb.depths) {
                assert!(
                    on_lattice(*c, d),
                    "box center {} depth {d:?} off-lattice",
                    show(&hb.center)
                );
            }
        }
    }

    #[test]
    fn no_two_boxes_share_a_unit_center() {
        let mut opt = DirectOptimizer::new(splat(5.0), zero(), 4_000);
        opt.run(&ShiftedSphere {
            shift: Pose {
                x: 1.7,
                y: 0.4,
                z: -2.1,
                xa: 0.8,
                ya: -1.3,
                za: 2.6,
            },
        });
        let mut seen = BTreeSet::new();
        for hb in iter_boxes(&opt) {
            let key = coords(&hb.center).map(f64::to_bits);
            assert!(
                seen.insert(key),
                "duplicate unit center {}",
                show(&hb.center)
            );
        }
    }

    #[test]
    fn each_box_depths_differ_by_at_most_one() {
        let mut opt = DirectOptimizer::new(splat(5.0), zero(), 120);
        opt.run(&ShiftedSphere {
            shift: Pose {
                x: 1.7,
                y: 0.4,
                z: -2.1,
                xa: 0.8,
                ya: -1.3,
                za: 2.6,
            },
        });
        for hb in iter_boxes(&opt) {
            let min = hb.depths.iter().copied().min().unwrap_or(0);
            let max = hb.depths.iter().copied().max().unwrap_or(0);
            assert!(
                max - min <= 1,
                "depths {:?} violate min-depth split",
                hb.depths
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 4096, ..ProptestConfig::default() })]

        #[test]
        fn convex_hull_matches_jones_on_unique_sizes(
            raw in proptest::collection::vec((0.1_f64..12.0, -8.0_f64..8.0), 1..12)
        ) {
            // Dedup sizes so the input matches what determine_potentially_optimal feeds.
            let mut pts = raw;
            pts.sort_by(|a, b| a.0.total_cmp(&b.0));
            pts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9);
            prop_assume!(!pts.is_empty());
            let poh: Vec<POHPoint> = pts
                .iter()
                .map(|&(size, cost)| POHPoint { size, cost })
                .collect();
            let mut hull = hull_pairs(&poh);
            let mut jones = jones_poh(&pts);
            hull.sort_by(|a, b| a.0.total_cmp(&b.0));
            jones.sort_by(|a, b| a.0.total_cmp(&b.0));
            prop_assert_eq!(hull, jones);
        }

        #[test]
        fn volume_partition_holds_for_random_runs(
            budget in 8_u32..60,
            shift in proptest::array::uniform6(-4.0_f64..4.0),
        ) {
            let [x, y, z, xa, ya, za] = shift;
            let mut opt = DirectOptimizer::new(splat(5.0), zero(), budget);
            opt.run(&ShiftedSphere { shift: Pose { x, y, z, xa, ya, za } });
            match volume_checksum(&opt) {
                Ok((lhs, rhs)) => prop_assert_eq!(lhs, rhs),
                Err("pow overflow") => {}
                Err(e) => prop_assert!(false, "volume checksum failed: {e}"),
            }
        }
    }

    struct Sphere;
    impl Cost for Sphere {
        fn eval(&self, poses: &[Pose]) -> Vec<f64> {
            poses
                .iter()
                .map(|p| coords(p).iter().map(|v| v * v).sum())
                .collect()
        }
    }

    #[test]
    fn seed_evaluation_counts_as_one_call_when_budget_is_zero() {
        let mut opt = DirectOptimizer::new(splat(5.0), zero(), 0);
        let _ = opt.run(&Sphere);
        assert_eq!(opt.calls, 1, "seed must still be evaluated at budget 0");
    }
}

/// Test-only constructors that pin the rotation representation.
///
/// Production currently always runs `RotationRepresentation::default()`
/// (`Euler`); these seams exist so the test suite can state *which* mode a
/// metamorphic property assumes instead of inheriting it silently.
#[cfg(test)]
impl DirectOptimizer {
    pub(crate) fn new_in_mode(
        range: Pose,
        starting_point: Pose,
        budget: u32,
        rotation: RotationRepresentation,
    ) -> Self {
        let mut opt = Self::new(range, starting_point, budget);
        opt.rotation_style = rotation;
        opt
    }
}

#[cfg(test)]
mod axis_angle_tests {
    //! SO(3) property tests for `RotationRepresentation::AxisAngle`.
    //!
    //! Intended semantics: the unit-space XA/YA/ZA coordinates of a hyperbox
    //! are *not* Euler angles; they denormalize to a scaled-axis (exponential
    //! map) increment `v` (in degrees at the interface, radians internally)
    //! that is applied in the body frame of the starting orientation:
    //!
    //!     R_result = R_start * Exp(v)
    //!
    //! The final rotation is only serialized back to ZXY Euler angles because
    //! the C++/FFI `Pose` interface consumes Euler angles. Euler triples are
    //! non-unique, so every property below compares rotations (SO(3)), never
    //! angle components.
    //!
    //! The project ZXY convention is defined by the C++ renderer
    //! (`RenderEngine::SetPose`: "R*v = RzRxRy*v"):
    //!
    //!     M(xa, ya, za) = Rz(za) * Rx(xa) * Ry(ya)
    //!
    //! Generators stay away from the ZXY gimbal-lock set (|xa| ~ 90 deg) and
    //! deltas are bounded well below a principal-angle ambiguity.

    use super::*;
    use crate::test_support::{pose, show, zero};
    use nalgebra::Rotation3;
    use proptest::prelude::*;

    /// Tolerance on orientation (geodesic) errors, in radians. Comfortably
    /// above f64 round-off through Exp/Euler round-trips (~1e-15), and far
    /// below the smallest semantic mistake worth catching (degrees/radians
    /// confusion is ~57x, a sign flip ~2x the angle).
    const ANG_TOL_RAD: f64 = 1e-9;

    /// Maximum |angle| for generated starting Euler components (degrees).
    /// The middle ZXY angle stays a further 10 deg below the +-90 deg
    /// gimbal-lock set; the outer two only need to avoid the wrap seam.
    const START_LIMIT: f64 = 85.0;
    const START_LIMIT_MID: f64 = 75.0;
    /// Half-width (degrees) of the denormalized axis-angle box used by the
    /// mapping tests, matching realistic tens-of-degrees search ranges.
    const RANGE_DEG: f64 = 20.0;

    /// Reconstruct a rotation from a serialized Pose using the same ZXY
    /// convention the rest of the project uses (C++ `SetPose`:
    /// Rz(za) * Rx(xa) * Ry(ya)).
    fn recon(p: &Pose) -> Rotation3<f64> {
        DirectOptimizer::from_euler_ordered("ZXY", [p.za, p.xa, p.ya], false)
    }

    /// Serialize a rotation to a Pose using production's own inverse path
    /// (`euler_angles_ordered("ZXY", extrinsic=false)`). Translation is zero;
    /// only the rotational half is meaningful. `observable` reports whether
    /// the extraction sat away from gimbal lock.
    fn serialize(r: &Rotation3<f64>) -> (Pose, bool) {
        let (angles, observable) =
            r.euler_angles_ordered(DirectOptimizer::axes_from_str("ZXY"), false);
        (
            Pose {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                xa: angles[1].to_degrees(),
                ya: angles[2].to_degrees(),
                za: angles[0].to_degrees(),
            },
            observable,
        )
    }

    /// Geodesic orientation error between two rotations, in radians.
    ///
    /// Computed as `2 * atan2(||imag(q)||, |w(q)|)` from the quaternion of
    /// the relative rotation. Do NOT replace with `(a.inverse() * b).angle()`:
    /// that is `acos((trace-1)/2)`, which saturates near the identity — fp
    /// noise of size `eps` in the trace shows up as `sqrt(eps) ~ 2e-8` rad
    /// "error" (and NaN when the acos argument lands just above 1), exactly
    /// the regime these property tests probe.
    fn ang_err(a: &Rotation3<f64>, b: &Rotation3<f64>) -> f64 {
        let q = nalgebra::UnitQuaternion::from_rotation_matrix(&(a.inverse() * b));
        2.0 * q.vector().norm().atan2(q.scalar().abs())
    }

    /// Rotation angle of `r` in [0, pi], numerically stable near identity
    /// (same atan2 construction as `ang_err`).
    fn rot_angle(r: &Rotation3<f64>) -> f64 {
        ang_err(&Rotation3::identity(), r)
    }

    /// Optimizer configured for AxisAngle over `range`, centered on `start`
    /// (budget is irrelevant: the mapping is tested directly).
    fn axis_angle_opt(start: Pose, range: Pose) -> DirectOptimizer {
        DirectOptimizer::new_in_mode(range, start, 0, RotationRepresentation::AxisAngle)
    }

    /// Unit-space pose whose rotational coordinates denote the scaled-axis
    /// delta `v_deg` (degrees) under `range` (|v_deg_i| <= range_i).
    fn unit_for_delta(v_deg: [f64; 3], range: &Pose) -> Pose {
        pose([
            0.5,
            0.5,
            0.5,
            0.5 + v_deg[0] / (2.0 * range.xa),
            0.5 + v_deg[1] / (2.0 * range.ya),
            0.5 + v_deg[2] / (2.0 * range.za),
        ])
    }

    /// apply(start, v): production's AxisAngle mapping for a body-frame
    /// scaled-axis delta `v_deg` (degrees), exercised without running DIRECT.
    fn apply(start: Pose, v_deg: [f64; 3]) -> Pose {
        let range = pose([0.0, 0.0, 0.0, RANGE_DEG, RANGE_DEG, RANGE_DEG]);
        axis_angle_opt(start, range).physical_pose_for_eval(unit_for_delta(v_deg, &range))
    }

    fn deg(v: f64) -> f64 {
        v.to_degrees()
    }

    /// C++ `RenderEngine::SetPose` literal (R*v = RzRxRy*v), transcribed from
    /// src/compute/render_engine.cu, as an independent anchor for the ZXY
    /// convention (not derived from nalgebra).
    fn cpp_setpose(xa_deg: f64, ya_deg: f64, za_deg: f64) -> nalgebra::Matrix3<f64> {
        let (sz, cz) = za_deg.to_radians().sin_cos();
        let (sx, cx) = xa_deg.to_radians().sin_cos();
        let (sy, cy) = ya_deg.to_radians().sin_cos();
        nalgebra::Matrix3::new(
            cz * cy - sz * sx * sy,
            -1.0 * sz * cx,
            cz * sy + sz * cy * sx,
            sz * cy + cz * sx * sy,
            cz * cx,
            sz * sy - cz * cy * sx,
            -1.0 * cx * sy,
            sx,
            cx * cy,
        )
    }

    #[test]
    fn project_zxy_convention_is_rz_rx_ry() {
        // Lock: recon(...) == the literal C++ SetPose matrix. If the Rust
        // composition order ever drifts from the C++ consumer, this fails
        // even if nalgebra stays internally self-consistent.
        for [xa, ya, za] in [
            [0.0, 0.0, 0.0],
            [12.5, -40.0, 5.0],
            [-60.0, 70.0, 150.0],
            [89.0, -3.0, -120.0],
        ] {
            let got = recon(&pose([0.0, 0.0, 0.0, xa, ya, za])).into_inner();
            let want = cpp_setpose(xa, ya, za);
            let err = (got - want).abs().max();
            assert!(err < 1e-12, "ZXY convention drift at {xa},{ya},{za}: {err}");
        }
    }

    #[test]
    fn serializer_inverts_reconstruction() {
        // The AxisAngle branch serializes with euler_angles_ordered("ZXY",
        // false); the SO(3) property tests reconstruct with recon(). Those
        // must be inverses up to fp, away from gimbal lock.
        for [xa, ya, za] in [
            [5.0, 12.0, -25.0],
            [-45.0, 60.0, 130.0],
            [70.0, -75.0, 10.0],
        ] {
            let p = pose([0.0, 0.0, 0.0, xa, ya, za]);
            let (q, observable) = serialize(&recon(&p));
            assert!(observable, "gimbal lock at {xa},{ya},{za}");
            let err = ang_err(&recon(&p), &recon(&q));
            assert!(err < 1e-12, "round-trip error {err} at {xa},{ya},{za}");
        }
    }

    #[test]
    fn default_rotation_style_is_euler_and_tests_override_it() {
        // Guards against Euler-branch code masquerading as AxisAngle tests.
        let opt = DirectOptimizer::new(zero(), zero(), 0);
        assert!(matches!(opt.rotation_style, RotationRepresentation::Euler));
        let aa = axis_angle_opt(zero(), pose([0.0, 0.0, 0.0, 5.0, 5.0, 5.0]));
        assert!(matches!(
            aa.rotation_style,
            RotationRepresentation::AxisAngle
        ));
    }

    #[test]
    fn zero_delta_is_identity_at_a_nontrivial_start() {
        let start = pose([1.0, -2.0, 3.0, 30.0, -45.0, 60.0]);
        let got = apply(start, [0.0, 0.0, 0.0]);
        let err = ang_err(&recon(&start), &recon(&got));
        assert!(err < 1e-12, "zero delta moved rotation by {err} rad");
        assert!(
            (got.x - start.x).abs() < 1e-12
                && (got.y - start.y).abs() < 1e-12
                && (got.z - start.z).abs() < 1e-12,
            "zero delta moved translation: {} vs {}",
            show(&got),
            show(&start)
        );
    }

    #[test]
    fn known_local_composition() {
        // Start 90 deg about world Z; delta 90 deg about local Z:
        // R_start * Exp(v) = Rz(180), so the result must map x -> -x.
        let start = pose([0.0, 0.0, 0.0, 0.0, 0.0, 90.0]);
        let got = apply(start, [0.0, 0.0, 90.0]);
        let r = recon(&got);
        let v = r * Vector3::x();
        let err = (v - Vector3::new(-1.0, 0.0, 0.0)).norm();
        assert!(err < 1e-9, "expected x -> -x, got {v:?} (err {err})");
        // Note: for this commuting pair the world-frame semantics
        // Exp(v) * R_start agrees too; locality is pinned by the
        // equivariance proptest, not this case.
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 4096, ..ProptestConfig::default() })]

        /// PRIMARY CONTRACT: R_start^-1 * R_result ~ Exp(v).
        /// Verifies body-frame composition, multiplication order, radian
        /// handling, signs, component order, axis direction, magnitude, and
        /// the Euler serialization round-trip all at once.
        #[test]
        fn observed_local_delta_equals_requested_delta(
            sza in -START_LIMIT..START_LIMIT,
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            vx in -RANGE_DEG..RANGE_DEG,
            vy in -RANGE_DEG..RANGE_DEG,
            vz in -RANGE_DEG..RANGE_DEG,
        ) {
            let start = pose([0.0, 0.0, 0.0, sxa, sya, sza]);
            let v = [vx, vy, vz];
            let result = apply(start, v);
            let r_start = recon(&start);
            let r_result = recon(&result);
            let observed = r_start.inverse() * r_result;
            let wanted = Rotation3::from_scaled_axis(Vector3::new(
                vx.to_radians(), vy.to_radians(), vz.to_radians(),
            ));
            let err = ang_err(&observed, &wanted);
            prop_assert!(
                err < ANG_TOL_RAD,
                "local delta mismatch: start Euler (xa,ya,za) = ({},{},{}) deg; \
                 requested scaled-axis v = ({},{},{}) deg; angular error = {:.3e} deg \
                 (tol {:.1e} deg)",
                sxa, sya, sza, vx, vy, vz, deg(err), deg(ANG_TOL_RAD)
            );
        }

        /// Zero delta keeps the orientation (SO(3) only; Euler components may
        /// re-branch) and the translation stays a plain affine denorm.
        #[test]
        fn zero_delta_preserves_orientation(
            sza in -START_LIMIT..START_LIMIT,
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            tx in -50.0..50.0,
            ty in -50.0..50.0,
            tz in -50.0..50.0,
        ) {
            let start = pose([tx, ty, tz, sxa, sya, sza]);
            let got = apply(start, [0.0, 0.0, 0.0]);
            let err = ang_err(&recon(&start), &recon(&got));
            prop_assert!(err < ANG_TOL_RAD, "zero delta moved rotation by {:.3e} deg", deg(err));
            prop_assert!((got.x - tx).abs() < 1e-9);
            prop_assert!((got.y - ty).abs() < 1e-9);
            prop_assert!((got.z - tz).abs() < 1e-9);
        }

        /// apply(apply(start, v), -v) returns to the starting orientation.
        /// Validates group-inverse behavior (note: does NOT by itself
        /// distinguish local from world-frame composition; the primary
        /// contract and the equivariance property do).
        #[test]
        fn delta_then_inverse_returns_to_start(
            sza in -START_LIMIT..START_LIMIT,
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            vx in -RANGE_DEG..RANGE_DEG,
            vy in -RANGE_DEG..RANGE_DEG,
            vz in -RANGE_DEG..RANGE_DEG,
        ) {
            let start = pose([0.0, 0.0, 0.0, sxa, sya, sza]);
            let v = [vx, vy, vz];
            let p1 = apply(start, v);
            let p2 = apply(p1, [-vx, -vy, -vz]);
            let err = ang_err(&recon(&start), &recon(&p2));
            prop_assert!(
                err < ANG_TOL_RAD,
                "v then -v did not return to start: err {:.3e} deg; start ({},{},{})",
                deg(err), sxa, sya, sza
            );
        }

        /// angle(R_start^-1 * R_result) == ||v_rad||: the scaled-axis norm
        /// IS the rotation angle, so this kills degree/radian confusion in
        /// either direction (denormalization or exponential map).
        #[test]
        fn relative_angle_equals_scaled_axis_norm(
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            sza in -START_LIMIT..START_LIMIT,
            vx in -RANGE_DEG..RANGE_DEG,
            vy in -RANGE_DEG..RANGE_DEG,
            vz in -RANGE_DEG..RANGE_DEG,
        ) {
            let start = pose([0.0, 0.0, 0.0, sxa, sya, sza]);
            let v = [vx, vy, vz];
            let vr = Vector3::new(vx.to_radians(), vy.to_radians(), vz.to_radians());
            let result = apply(start, v);
            let relative = recon(&start).inverse() * recon(&result);
            // ||v|| <= 20*sqrt(3) deg < pi, so the principal angle is unambiguous.
            prop_assert!(vr.norm() < std::f64::consts::PI);
            let err = (rot_angle(&relative) - vr.norm()).abs();
            prop_assert!(
                err < ANG_TOL_RAD,
                "relative angle {} rad != scaled-axis norm {} rad (diff {:.3e})",
                rot_angle(&relative), vr.norm(), err
            );
        }

        /// Two increments about the SAME axis add: applying a*n then b*n
        /// sequentially equals applying (a+b)*n once.
        #[test]
        fn same_axis_increments_are_additive(
            axis_idx in 0usize..3,
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            sza in -START_LIMIT..START_LIMIT,
            a in -15.0f64..15.0,
            b in -15.0f64..15.0,
        ) {
            let mut n = [0.0f64; 3];
            n[axis_idx] = 1.0;
            let start = pose([0.0, 0.0, 0.0, sxa, sya, sza]);
            let seq = apply(
                apply(start, [a * n[0], a * n[1], a * n[2]]),
                [b * n[0], b * n[1], b * n[2]],
            );
            let sum = apply(start, [(a + b) * n[0], (a + b) * n[1], (a + b) * n[2]]);
            let err = ang_err(&recon(&seq), &recon(&sum));
            prop_assert!(
                err < ANG_TOL_RAD,
                "same-axis additivity broken (axis {axis_idx}, a {a}, b {b}): err {:.3e} deg",
                deg(err)
            );
        }

        /// LOCAL-frame equivariance: for any world rotation Q,
        /// apply(Q * R, v) == Q * apply(R, v) — the scaled-axis perturbation
        /// stays attached to the object frame and is natural in the world
        /// frame. Fails for the world-frame semantics Exp(v) * R whenever Q
        /// does not commute with the rotation, so this is the metamorphic
        /// guard against silently switching multiplication sides.
        #[test]
        fn delta_is_local_under_world_frame_change(
            q1 in -160.0f64..160.0,
            q2 in -60.0f64..60.0,
            q3 in -160.0f64..160.0,
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            sza in -START_LIMIT..START_LIMIT,
            vx in -RANGE_DEG..RANGE_DEG,
            vy in -RANGE_DEG..RANGE_DEG,
            vz in -RANGE_DEG..RANGE_DEG,
        ) {
            let q = DirectOptimizer::from_euler_ordered("ZXY", [q1, q2, q3], false);
            let start = pose([0.0, 0.0, 0.0, sxa, sya, sza]);
            let r_start = recon(&start);
            let v = [vx, vy, vz];

            // Rotate the object's world frame by Q, then perturb locally.
            let (moved_start, observable) = serialize(&(q * r_start));
            prop_assume!(observable);
            let lhs = recon(&apply(moved_start, v));

            // Perturb in the original world frame, then rotate the outcome.
            let rhs = q * recon(&apply(start, v));

            let err = ang_err(&lhs, &rhs);
            prop_assert!(
                err < ANG_TOL_RAD,
                "local-frame equivariance broken: err {:.3e} deg (start ({},{},{}) v ({},{},{}) Q Euler ({},{},{}))",
                deg(err), sxa, sya, sza, vx, vy, vz, q1, q2, q3
            );
        }

        /// Pure-axis sanity: v = (theta,0,0) must be a rotation by theta
        /// about the LOCAL X of the start frame (likewise Y, Z). Overlaps
        /// with the primary contract but isolates component-order bugs with
        /// far more readable failure output.
        #[test]
        fn pure_axis_deltas_rotate_about_local_axis(
            axis_idx in 0usize..3,
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            sza in -START_LIMIT..START_LIMIT,
            theta in -20.0f64..20.0,
        ) {
            let mut v = [0.0f64; 3];
            v[axis_idx] = theta;
            let start = pose([0.0, 0.0, 0.0, sxa, sya, sza]);
            let result = apply(start, v);
            let relative = recon(&start).inverse() * recon(&result);
            let wanted = match axis_idx {
                0 => Rotation3::from_axis_angle(&Vector3::x_axis(), theta.to_radians()),
                1 => Rotation3::from_axis_angle(&Vector3::y_axis(), theta.to_radians()),
                _ => Rotation3::from_axis_angle(&Vector3::z_axis(), theta.to_radians()),
            };
            let err = ang_err(&relative, &wanted);
            prop_assert!(
                err < ANG_TOL_RAD,
                "pure axis {axis_idx} at theta {theta} deg from start ({},{},{}): err {:.3e} deg",
                sxa, sya, sza, deg(err)
            );
        }
    }
}
