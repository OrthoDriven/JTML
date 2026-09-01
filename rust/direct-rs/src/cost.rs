use crate::direct::geometry::Pose;

pub trait Cost {
    fn eval(&self, poses: &[Pose]) -> Vec<f64>;
}
