//! The DIRECT driver: hyperbox-tree lifecycle, batch scoring, and the
//! optional BOBYQA refinement stage. Search geometry itself lives in
//! `crate::space`; this module is concerned with DIRECT, not camera
//! coordinates.

use std::time::{self, Duration};

use ordered_float::OrderedFloat;

use crate::{
    cost::Cost,
    direct::{
        poh::POHPoint,
        settings::{DirectSettings, Refinement},
        tree::{DirectTree, Hyperbox, UnscoredHyperbox},
    },
    pose::{DIRECTIONS, Direction, PhysicalPose, PoseRange, UnitPose},
    space::SearchSpace,
};

pub(crate) mod poh;
pub mod settings;
pub(crate) mod tree;

#[cfg(test)]
mod test;

/// Optimizer outcome: the best physical pose found and its cost.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Incumbent {
    pub pose: PhysicalPose,
    pub cost: f64,
}

pub struct DirectOptimizer {
    pub(crate) boxes: DirectTree,
    current_best: Incumbent,
    budget: u32,
    pub(crate) calls: u32,
    next_box_id: u64,
    space: SearchSpace,
    settings: DirectSettings,
}

impl DirectOptimizer {
    pub fn new(range: PoseRange, starting_point: PhysicalPose, budget: u32) -> Self {
        Self::from_settings(range, starting_point, budget, DirectSettings::default())
    }
    pub fn from_settings(
        range: PoseRange,
        starting_point: PhysicalPose,
        budget: u32,
        settings: DirectSettings,
    ) -> Self {
        // The unit↔physical mapping (starting rotation, camera basis) is
        // materialized once here. Because it is precomputed, editing
        // `settings.rotation`/`settings.translation` after construction
        // would silently diverge from the actual mapping — construct with
        // the settings you want.
        let space = SearchSpace::new(
            starting_point,
            range,
            settings.rotation,
            settings.translation,
        );

        return Self {
            boxes: DirectTree::new(),
            current_best: Incumbent {
                pose: starting_point,
                cost: f64::INFINITY,
            },
            budget,
            calls: 0,
            next_box_id: 0,
            space,
            settings,
        };
    }

    /// Read of the search-space mapping without running DIRECT. Same-crate
    /// tests (the SO(3) axis-angle suite) exercise the precomputed map
    /// through this seam.
    pub(crate) fn physical_pose(&self, unit: UnitPose) -> PhysicalPose {
        self.space.physical_pose(unit)
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

    pub fn run<T: Cost>(&mut self, cost: &T) -> Incumbent {
        // seed: box lives at the unit center; cost eval at its physical pose
        let start = time::Instant::now();

        let unit = Self::unit_center();
        let physical = self.space.physical_pose(unit);

        let seed_cost = cost.eval_one(physical);
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

        if seed_cost.is_finite() && seed_cost < self.current_best.cost {
            self.current_best = Incumbent {
                pose: physical,
                cost: seed_cost,
            };
        }

        loop {
            if self.calls >= self.budget {
                break;
            }

            let candidates = self.get_potentially_optimal_candidates();
            let poh = self.settings.poh_strategy.select(&candidates);

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

        self.refine(cost);

        if self.current_best.cost.is_finite() {
            return self.best();
        } else {
            return Incumbent {
                pose: physical,
                cost: f64::INFINITY,
            };
        }
    }
    pub fn best(&self) -> Incumbent {
        self.current_best
    }

    /// Optional local refinement after the DIRECT loop, per
    /// `settings.refinement`. Two failure channels with one degradation
    /// policy (log, keep the DIRECT incumbent):
    ///
    /// - `run_bobyqa`'s `Result` closes *our* error-as-panic channel (the
    ///   `Executor::run() -> Result` path).
    /// - The `catch_unwind` firewall is retained around basin itself: it is
    ///   third-party numerical code with verified internal panic sites no
    ///   `Result` can catch (rho-ordering in `driver.rs`, model validity in
    ///   `init.rs`, positive trust-region step in `trsbox.rs`). A side
    ///   effect accepted knowingly: a `Cost::eval` batch-contract assert
    ///   that fires during refinement is *contained* here (logged, DIRECT
    ///   result kept) rather than crashing; the batch-path assert in
    ///   `score_and_reinsert` still propagates.
    fn refine<T: Cost>(&mut self, cost: &T) {
        let Refinement::Bobyqa(bobyqa) = self.settings.refinement else {
            return;
        };

        let best_unit = self
            .boxes
            .values()
            .flat_map(|row| row.values())
            .filter(|hb| hb.cost_at_center.is_finite())
            .min_by(|a, b| a.cost_at_center.total_cmp(&b.cost_at_center))
            .map(|hb| hb.center);

        let Some(best_unit) = best_unit else {
            return;
        };

        let contained = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::basin_opt::run_bobyqa(&self.space, cost, best_unit, bobyqa)
        }));

        match contained {
            Ok(Ok(refined)) => {
                self.calls += refined.evals as u32;

                if refined.cost.is_finite() && refined.cost < self.current_best.cost {
                    self.current_best = Incumbent {
                        pose: refined.pose,
                        cost: refined.cost,
                    };
                }
            }
            Ok(Err(err)) => eprintln!("BOBYQA failed; keeping DIRECT result: {err}"),
            Err(_) => eprintln!("BOBYQA panicked; keeping DIRECT result"),
        }
    }

    fn unit_center() -> UnitPose {
        UnitPose::from([0.5; 6])
    }

    /// The axis to split, if any: the least-subdivided axis still above its
    /// minimum physical width. Returns `None` when every candidate axis has
    /// reached the `min_box_size` floor.
    fn split_axis(&self, hb: &Hyperbox) -> Option<Direction> {
        DIRECTIONS
            .iter()
            .copied()
            .filter(|dir| {
                self.settings.min_box_size.values[dir.index()]
                    .is_none_or(|min_size| {
                        self.space.width_at(*dir, hb.depths[dir.index()]) > min_size
                    })
            })
            .min_by_key(|dir| hb.depths[dir.index()])
    }

    fn trisect_and_return_unscored(&mut self, boxes: &[POHPoint]) -> Vec<UnscoredHyperbox> {
        let mut unscored: Vec<UnscoredHyperbox> = Vec::with_capacity(2 * boxes.len());
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

            let (center, shifted) = parent.trisect(axis);

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
        let centers: Vec<PhysicalPose> = unscored
            .iter()
            .map(|p| self.space.physical_pose(p.center))
            .collect();
        let evaluated_costs: Vec<f64> = cost.eval(&centers);
        assert_eq!(
            evaluated_costs.len(),
            centers.len(),
            "Cost::eval must return one cost per pose"
        );
        self.calls += evaluated_costs.len() as u32;

        for (unscored_box, cost_value) in unscored.iter().zip(evaluated_costs) {
            // Id allocation is unconditional here, as before: the (cost, id)
            // key sequence must stay identical whether or not the box scores
            // finite.
            let id = self.next_id();
            let scored_box = unscored_box.add_score(cost_value);
            if scored_box.cost_at_center.is_finite() {
                let physical = self.space.physical_pose(scored_box.center);
                if scored_box.cost_at_center < self.current_best.cost {
                    self.current_best = Incumbent {
                        pose: physical,
                        cost: scored_box.cost_at_center,
                    };
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
        self.boxes
            .iter()
            .filter_map(|(size_key, row)| {
                row.iter()
                    .find(|(_, hb)| self.split_axis(hb).is_some())
                    .map(|(key, _)| POHPoint {
                        size: size_key.into_inner(),
                        cost: key.0.into_inner(),
                    })
            })
            .collect()
    }
}
