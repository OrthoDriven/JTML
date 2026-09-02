//! The DIRECT hyperbox tree: a `(size, cost, id)`-keyed forest of scored and
//! unscored boxes in unit space.
//!
//! Tree shape: the outer map is keyed by box size, the inner map by
//! `(cost, id)`. The id tie-breaker is load-bearing — without it two boxes
//! sharing both size and cost would collide and one would be silently
//! dropped, breaking the volume-partition invariant that
//! `direct/test.rs::boxes_tile_the_unit_cube_exactly` checks.

use std::collections::BTreeMap;

use ordered_float::OrderedFloat;

use crate::pose::{DIRECTIONS, Direction, UnitPose};

pub type SizeKey = OrderedFloat<f64>;
pub type CostKey = (OrderedFloat<f64>, u64);
pub type DirectTree = BTreeMap<SizeKey, BTreeMap<CostKey, Hyperbox>>;

#[cfg(test)]
mod test;

#[derive(Clone, Copy, Debug)]
pub struct Hyperbox {
    pub cost_at_center: f64,
    pub center: UnitPose,
    pub depths: [u32; 6],
}

#[derive(Clone, Copy, Debug)]
pub struct UnscoredHyperbox {
    pub center: UnitPose,
    pub depths: [u32; 6],
}

impl Hyperbox {
    /// The least-subdivided axis: splitting it keeps depth spread ≤ 1 and
    /// shrinks volume the least per split.
    pub fn longest_axis(&self) -> Direction {
        self.depths
            .iter()
            .zip(DIRECTIONS)
            .min_by_key(|(depth, _)| **depth)
            .map(|(_, dir)| dir)
            .expect("array is fixed-size and non-empty")
    }

    pub fn size(&self) -> f64 {
        return self
            .depths
            .iter()
            .map(|e| 3f64.powf(-2.0 * (*e as f64)))
            .sum::<f64>()
            .sqrt();
    }

    /// Split along `axis` into the re-scored parent (one deeper on `axis`)
    /// and its two shifted, unscored children. `Direction` makes an
    /// out-of-range axis inexpressible, so no bounds guard is needed.
    pub fn trisect(mut self, axis: Direction) -> (Hyperbox, [UnscoredHyperbox; 2]) {
        self.depths[axis.index()] += 1;

        let shift = 3f64.powi(-(self.depths[axis.index()] as i32));

        let mut posc = self.center;
        let mut negc = self.center;

        posc.shift(axis, shift);
        negc.shift(axis, -shift);

        let pos_shift = UnscoredHyperbox {
            center: posc,
            depths: self.depths,
        };

        let neg_shift = UnscoredHyperbox {
            center: negc,
            depths: self.depths,
        };

        (self, [pos_shift, neg_shift])
    }
}

impl UnscoredHyperbox {
    pub fn add_score(self, score: f64) -> Hyperbox {
        return Hyperbox {
            cost_at_center: score,
            depths: self.depths,
            center: self.center,
        };
    }
}
