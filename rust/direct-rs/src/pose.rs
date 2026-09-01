//! 6-DoF pose representation shared by every optimizer in this crate.
//!
//! `Pose` is the interface currency: [`crate::cost::Cost`] impls consume
//! batches of physical `Pose`s, the FFI bridge marshals `[f64; 6]` into
//! `Pose`s, and each optimizer (DIRECT, BOBYQA-refinement, ...) maps its own
//! internal search space into physical `Pose`s for evaluation.
//!
//! The ZXY Euler convention here matches the C++ renderer
//! (`RenderEngine::SetPose`: "R*v = RzRxRy*v"); `direct/test.rs` locks the
//! two together against a literal transcription of the C++ matrix.

use nalgebra::{Rotation3, Unit, Vector3};

#[derive(Copy, Clone, Default, Debug)]
pub struct Pose {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub xa: f64,
    pub ya: f64,
    pub za: f64,
}
impl Pose {
    pub fn shift(&mut self, dir: &Direction, amount: f64) {
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
        return [self.x, self.y, self.z, self.xa, self.ya, self.za];
    }
}

#[expect(non_camel_case_types, reason = "matching cpp style")]
#[derive(Clone, Copy)]
pub enum Direction {
    X_DIR = 0,
    Y_DIR = 1,
    Z_DIR = 2,
    XA_DIR = 3,
    YA_DIR = 4,
    ZA_DIR = 5,
}

pub const DIRECTIONS: [Direction; 6] = [
    Direction::X_DIR,
    Direction::Y_DIR,
    Direction::Z_DIR,
    Direction::XA_DIR,
    Direction::YA_DIR,
    Direction::ZA_DIR,
];

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
    let seq_vec = axes_from_str(seq);
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

/// Half-widths of the search box around a starting pose (not a pose itself).
struct Range(Pose);
