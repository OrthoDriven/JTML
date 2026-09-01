//! Unit-cube ↔ physical mapping: the precomputed search geometry shared by
//! this crate's optimizers.
//!
//! A `SearchSpace` is built once from plain construction arguments (a
//! starting pose, per-axis half-widths, and the two representation modes)
//! and owns everything the mapping needs: the starting rotation (only for
//! `AxisAngle`) and the camera ray basis (only for `CameraCentered`).
//! Optimizers ask it to translate unit-space hyperbox centers into physical
//! poses to evaluate.
//!
//! This module deliberately imports nothing from `direct::`: the dependency
//! arrow runs one way, `DirectSettings → DirectOptimizer → SearchSpace`, so
//! `space` knows about camera geometry but nothing about DIRECT.
//!
//! `RotationRepresentation` / `TranslationRepresentation` are the mode
//! selectors declared here and carried by `DirectSettings`; they live here
//! because the maps they select are materialized by this module.

use nalgebra::{self as na, Rotation3, Unit, Vector3};

use crate::pose::{AxisSeq, Direction, PhysicalPose, Pose, PoseRange, UnitPose, from_euler_ordered};

/// How unit-space XA/YA/ZA coordinates become a physical orientation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RotationRepresentation {
    AxisAngle,
    #[default]
    Euler,
}

/// How unit-space X/Y/Z coordinates become a physical translation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
    PureEuclidean,
    CameraCentered,
}

/// The rotation half of the mapping, materialized at construction.
enum RotationMap {
    /// Euler angles denormalize directly: physical angle = start + delta.
    Euler,
    /// Unit XA/YA/ZA are a body-frame scaled-axis delta applied *after* the
    /// starting orientation, then serialized back to ZXY Euler angles.
    AxisAngle { start: Rotation3<f64> },
}

/// The translation half of the mapping, materialized at construction.
enum TranslationMap {
    /// Axis-aligned affine denormalization.
    Euclidean,
    /// Local "z" slides along the camera→object ray.
    CameraCentered { basis: na::Matrix3<f64> },
}

/// The unit↔physical mapping and its precomputed state.
///
/// Fields stay private to this module; consumers work through
/// [`SearchSpace::physical_pose`] and [`SearchSpace::width_at`].
pub(crate) struct SearchSpace {
    start: PhysicalPose,
    range: PoseRange,
    rotation: RotationMap,
    translation: TranslationMap,
}

impl SearchSpace {
    /// Build the mapping from plain values. Mode-dependent state is
    /// computed exactly once, and only for the modes that consult it:
    /// the starting rotation for `AxisAngle`, the camera basis for
    /// `CameraCentered`.
    pub(crate) fn new(
        start: PhysicalPose,
        range: PoseRange,
        rotation: RotationRepresentation,
        translation: TranslationRepresentation,
    ) -> Self {
        let rotation_map = match rotation {
            RotationRepresentation::Euler => RotationMap::Euler,
            RotationRepresentation::AxisAngle => RotationMap::AxisAngle {
                start: from_euler_ordered(
                    AxisSeq::ZXY,
                    [start.za, start.xa, start.ya],
                    false,
                ),
            },
        };

        let translation_map = match translation {
            TranslationRepresentation::PureEuclidean => TranslationMap::Euclidean,
            TranslationRepresentation::CameraCentered => {
                // Ray from the camera (origin) through the starting position.
                // Declared semantic exception (plan 017 R12): a zero-length
                // start position previously produced NaN-poisoned non-finite
                // state via `Unit::new_normalize`; it is now *defined* as the
                // +Z ray. No production path depends on the old NaN behavior
                // (every existing zero-start test runs `PureEuclidean`, which
                // never consults the basis).
                let ray = Vector3::new(start.x, start.y, start.z);
                let ray = Unit::try_new(ray, 0.0).unwrap_or_else(Vector3::z_axis);
                TranslationMap::CameraCentered {
                    basis: na::Matrix3::from_columns(&[Vector3::x(), Vector3::y(), ray.into_inner()]),
                }
            }
        };

        Self {
            start,
            range,
            rotation: rotation_map,
            translation: translation_map,
        }
    }

    /// Map a unit-space pose (each axis nominally in [0,1]) to physical space.
    ///
    /// Euler/translation path, axis by axis:
    /// physical[i] = start[i] + (unit[i] - 0.5) * 2 * range[i]
    pub(crate) fn physical_pose(&self, unit: UnitPose) -> PhysicalPose {
        let (xa, ya, za) = match &self.rotation {
            // Euler rotations are a basic denormalization
            RotationMap::Euler => (
                self.start.xa + (unit.xa - 0.5) * 2.0 * self.range.xa,
                self.start.ya + (unit.ya - 0.5) * 2.0 * self.range.ya,
                self.start.za + (unit.za - 0.5) * 2.0 * self.range.za,
            ),

            // In axis angle, we're taking hyperbox location as a further pose applied to
            // the original point. Denormalization alone doesn't get you physical
            // rotation, you must further apply rotations
            RotationMap::AxisAngle { start } => {
                let denormed_xa = (unit.xa - 0.5) * 2.0 * self.range.xa;
                let denormed_ya = (unit.ya - 0.5) * 2.0 * self.range.ya;
                let denormed_za = (unit.za - 0.5) * 2.0 * self.range.za;

                let applied_rot_t = Rotation3::from_scaled_axis(Vector3::new(
                    denormed_xa.to_radians(),
                    denormed_ya.to_radians(),
                    denormed_za.to_radians(),
                ));

                let applied_rot = start * applied_rot_t;
                let (final_angles, _observable) =
                    applied_rot.euler_angles_ordered(AxisSeq::ZXY.axes(), false);
                let [za, xa, ya] = final_angles;

                (xa.to_degrees(), ya.to_degrees(), za.to_degrees())
            }
        };

        let (x, y, z) = match &self.translation {
            TranslationMap::Euclidean => (
                self.start.x + (unit.x - 0.5) * 2.0 * self.range.x,
                self.start.y + (unit.y - 0.5) * 2.0 * self.range.y,
                self.start.z + (unit.z - 0.5) * 2.0 * self.range.z,
            ),
            TranslationMap::CameraCentered { basis } => {
                let local_translation = Vector3::new(
                    (unit.x - 0.5) * 2.0 * self.range.x,
                    (unit.y - 0.5) * 2.0 * self.range.y,
                    (unit.z - 0.5) * 2.0 * self.range.z,
                );
                let world_translation = basis * local_translation;
                (
                    self.start.x + world_translation.x,
                    self.start.y + world_translation.y,
                    self.start.z + world_translation.z,
                )
            }
        };

        Pose {
            x,
            y,
            z,
            xa,
            ya,
            za,
        }
        .into()
    }

    /// Full physical width of one axis at a given split depth.
    ///
    /// Delegator to `PoseRange`: consumers never learn that a `PoseRange`
    /// lives inside the space (data hidden, operations exposed).
    pub(crate) fn width_at(&self, dir: Direction, depth: u32) -> f64 {
        self.range.width_at(dir, depth)
    }
}

#[cfg(test)]
mod test;
