//! Cost evaluation contract shared by every search stage.

use crate::pose::PhysicalPose;

pub trait Cost {
    /// Returns exactly one cost per supplied physical pose, order preserved
    /// (result `i` belongs to `poses[i]`). A `NaN` entry signals an
    /// invalid/unmeasurable pose: the optimizer never stores non-finite
    /// box costs.
    fn eval(&self, poses: &[PhysicalPose]) -> Vec<f64>;

    /// Single-pose evaluation. The centralized enforcement point of the
    /// batch contract for every consumer: the 1:1 length guarantee is
    /// asserted here rather than trusted.
    fn eval_one(&self, pose: PhysicalPose) -> f64 {
        let mut values = self.eval(std::slice::from_ref(&pose));
        assert_eq!(values.len(), 1, "Cost::eval must return one cost per pose");
        values.pop().expect("length asserted above")
    }
}
