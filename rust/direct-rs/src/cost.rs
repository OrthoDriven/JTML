use crate::pose::Pose;

pub trait Cost {
    fn eval(&self, poses: &[Pose]) -> Vec<f64>;
}
