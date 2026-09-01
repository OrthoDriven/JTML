mod basin_opt;
mod bridge;
pub mod cost;
pub mod direct;
mod pose;
mod space;

#[cfg(test)]
mod fixtures;

#[cfg(test)]
mod problems;

#[cfg(test)]
mod properties;

pub use cost::Cost;
pub use direct::settings::DirectSettings;
pub use direct::{DirectOptimizer, Incumbent};
pub use pose::{PhysicalPose, Pose, PoseRange, UnitPose};
pub use space::{RotationRepresentation, TranslationRepresentation};

// Names below are resolved by `bridge.rs`'s `use super::*;` inside the
// cxx bridge expansion:
use crate::bridge::ffi::CppCost;
