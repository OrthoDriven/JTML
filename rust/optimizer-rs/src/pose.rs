//! 6-DoF pose representation shared by every optimizer in this crate.
//!
//! `Pose` is the raw coordinate carrier. The search pipeline assigns it
//! three distinct roles, each with its own newtype so a unit-cube center can
//! never be mistaken for a world-space pose (or a half-width vector):
//!
//! - [`PhysicalPose`] — world-space pose: [`crate::cost::Cost`] impls consume
//!   batches of these, the FFI bridge marshals `[f64; 6]` into and out of
//!   them, and each optimizer's `run` returns its incumbent as one.
//! - [`UnitPose`] — a hyperbox center inside the unit cube (every coordinate
//!   nominally in [0, 1]); the role actually mutated during trisection.
//! - [`PoseRange`] — per-axis half-widths of the search box around a starting
//!   pose; a width vector, not a location.
//!
//! The wrappers hold their `Pose` privately: construct through `From` and
//! read through `Deref`. The conversion back to a raw `Pose` is deliberate,
//! which is the point — changing a pose's role requires naming it.
//!
//! The ZXY Euler convention here matches the C++ renderer
//! (`RenderEngine::SetPose`: "R*v = RzRxRy*v"); `direct/test.rs` locks the
//! two together against a literal transcription of the C++ matrix.

use std::ops::{Deref, DerefMut};

use nalgebra::{Rotation3, Unit, Vector3};

#[derive(Copy, Clone, Default, Debug, PartialEq)]
pub struct Pose {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub xa: f64,
    pub ya: f64,
    pub za: f64,
}

impl Pose {
    pub fn shift(&mut self, dir: Direction, amount: f64) {
        match dir {
            Direction::X_DIR => self.x += amount,
            Direction::Y_DIR => self.y += amount,
            Direction::Z_DIR => self.z += amount,
            Direction::XA_DIR => self.xa += amount,
            Direction::YA_DIR => self.ya += amount,
            Direction::ZA_DIR => self.za += amount,
        }
    }
    pub fn to_array(self) -> [f64; 6] {
        self.into()
    }
}

impl From<Pose> for [f64; 6] {
    fn from(p: Pose) -> Self {
        [p.x, p.y, p.z, p.xa, p.ya, p.za]
    }
}

impl From<[f64; 6]> for Pose {
    fn from(a: [f64; 6]) -> Self {
        let [x, y, z, xa, ya, za] = a;
        Pose {
            x,
            y,
            z,
            xa,
            ya,
            za,
        }
    }
}

/// World-space pose: what `Cost` evaluates and the FFI marshals.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PhysicalPose(Pose);

impl From<Pose> for PhysicalPose {
    fn from(p: Pose) -> Self {
        Self(p)
    }
}

impl From<[f64; 6]> for PhysicalPose {
    fn from(a: [f64; 6]) -> Self {
        Self(Pose::from(a))
    }
}

impl From<PhysicalPose> for [f64; 6] {
    fn from(p: PhysicalPose) -> Self {
        (*p).into()
    }
}

impl Deref for PhysicalPose {
    type Target = Pose;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Hyperbox center in unit-cube space; the role mutated during trisection.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct UnitPose(Pose);

impl From<Pose> for UnitPose {
    fn from(p: Pose) -> Self {
        Self(p)
    }
}

impl From<[f64; 6]> for UnitPose {
    fn from(a: [f64; 6]) -> Self {
        Self(Pose::from(a))
    }
}

impl From<UnitPose> for [f64; 6] {
    fn from(p: UnitPose) -> Self {
        (*p).into()
    }
}

impl Deref for UnitPose {
    type Target = Pose;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for UnitPose {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Per-axis half-widths of the search box around a starting pose
/// (not a location; a width vector).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PoseRange(Pose);

impl PoseRange {
    /// Full physical width of an axis at a given split depth:
    /// `2 * |half_width| * 3^-depth`.
    pub fn width_at(&self, dir: Direction, depth: u32) -> f64 {
        let half = match dir {
            Direction::X_DIR => self.x,
            Direction::Y_DIR => self.y,
            Direction::Z_DIR => self.z,
            Direction::XA_DIR => self.xa,
            Direction::YA_DIR => self.ya,
            Direction::ZA_DIR => self.za,
        };
        2.0 * half.abs() * 3f64.powi(-(depth as i32))
    }
}

impl From<Pose> for PoseRange {
    fn from(p: Pose) -> Self {
        Self(p)
    }
}

impl From<[f64; 6]> for PoseRange {
    fn from(a: [f64; 6]) -> Self {
        Self(Pose::from(a))
    }
}

impl From<PoseRange> for [f64; 6] {
    fn from(r: PoseRange) -> Self {
        (*r).into()
    }
}

impl Deref for PoseRange {
    type Target = Pose;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[expect(non_camel_case_types, reason = "matching cpp style")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    X_DIR = 0,
    Y_DIR = 1,
    Z_DIR = 2,
    XA_DIR = 3,
    YA_DIR = 4,
    ZA_DIR = 5,
}

impl Direction {
    /// Canonical coordinate index for this axis (matches `Pose::to_array`
    /// order and the `depths` array).
    pub const fn index(self) -> usize {
        match self {
            Direction::X_DIR => 0,
            Direction::Y_DIR => 1,
            Direction::Z_DIR => 2,
            Direction::XA_DIR => 3,
            Direction::YA_DIR => 4,
            Direction::ZA_DIR => 5,
        }
    }
}

pub const DIRECTIONS: [Direction; 6] = [
    Direction::X_DIR,
    Direction::Y_DIR,
    Direction::Z_DIR,
    Direction::XA_DIR,
    Direction::YA_DIR,
    Direction::ZA_DIR,
];

/// An ordered Euler-axis sequence. Total: no string parsing, no panic,
/// no per-evaluation allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisSeq {
    #[expect(clippy::upper_case_acronyms, reason = "axis sequence name mirrors the C++ ZXY convention")]
    ZXY,
}

impl AxisSeq {
    pub fn axes(self) -> [Unit<Vector3<f64>>; 3] {
        match self {
            AxisSeq::ZXY => [Vector3::z_axis(), Vector3::x_axis(), Vector3::y_axis()],
        }
    }
}

pub fn from_euler_ordered(seq: AxisSeq, angles: [f64; 3], extrinsic: bool) -> Rotation3<f64> {
    let [ax0, ax1, ax2] = seq.axes();
    let [a0, a1, a2] = angles;
    let r0 = Rotation3::from_axis_angle(&ax0, a0.to_radians());
    let r1 = Rotation3::from_axis_angle(&ax1, a1.to_radians());
    let r2 = Rotation3::from_axis_angle(&ax2, a2.to_radians());

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

#[cfg(test)]
mod test;
