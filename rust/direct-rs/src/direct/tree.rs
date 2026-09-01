use std::collections::BTreeMap;

use ordered_float::OrderedFloat;

use crate::direct::geometry::{Pose, DIRECTIONS};

pub type SizeKey = OrderedFloat<f64>;
pub type CostKey = (OrderedFloat<f64>, u64);
pub type DirectTree = BTreeMap<SizeKey, BTreeMap<CostKey, Hyperbox>>;

#[derive(Clone, Copy)]
pub struct Hyperbox {
    pub cost_at_center: f64,
    pub center: Pose,
    pub depths: [u32; 6],
}

#[derive(Clone, Copy)]
pub struct UnscoredHyperbox {
    pub center: Pose,
    pub depths: [u32; 6],
}

impl Hyperbox {
    pub fn longest_axis(&self) -> usize {
        self.depths
            .iter()
            .enumerate()
            .min_by_key(|(_, depth)| **depth)
            .map(|(axis, _)| axis)
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
    // TODO: how to force usize to be the right size at runtime?
    pub fn trisect(mut self, axis: usize) -> (Hyperbox, [UnscoredHyperbox; 2]) {
        if let Some(x) = self.depths.get_mut(axis) {
            *x += 1;
        }

        let shift = 3f64.powi(-(self.depths[axis] as i32));

        let mut posc = self.center;
        let mut negc = self.center;

        posc.shift(&DIRECTIONS[axis], shift);
        negc.shift(&DIRECTIONS[axis], -shift);

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
