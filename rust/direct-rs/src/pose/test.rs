//! Pose newtype tests: role conversions, the `Direction` index map, and the
//! `PoseRange` physical-width formula extracted from the old
//! `DirectOptimizer::physical_width`.

use super::*;
use crate::fixtures::coords;

#[test]
fn direction_index_covers_six_canonical_slots() {
    let indices = DIRECTIONS.map(|d| d.index());
    assert_eq!(indices, [0, 1, 2, 3, 4, 5]);
    assert_eq!(Direction::X_DIR.index(), 0);
    assert_eq!(Direction::Y_DIR.index(), 1);
    assert_eq!(Direction::Z_DIR.index(), 2);
    assert_eq!(Direction::XA_DIR.index(), 3);
    assert_eq!(Direction::YA_DIR.index(), 4);
    assert_eq!(Direction::ZA_DIR.index(), 5);
}

#[test]
fn pose_range_width_at_mirrors_physical_width() {
    // Half-widths chosen distinct (and one negative) so axis mix-ups and
    // lost abs() both show up.
    let range = PoseRange::from(Pose {
        x: 2.0,
        y: -1.5,
        z: 0.25,
        xa: 10.0,
        ya: 0.0,
        za: 5.0,
    });
    for (dir, half) in [
        (Direction::X_DIR, 2.0),
        (Direction::Y_DIR, -1.5),
        (Direction::Z_DIR, 0.25),
        (Direction::XA_DIR, 10.0),
        (Direction::YA_DIR, 0.0),
        (Direction::ZA_DIR, 5.0),
    ] {
        for depth in [0u32, 2, 5] {
            let want = 2.0 * half.abs() / 3f64.powi(depth as i32);
            let got = range.width_at(dir, depth);
            assert!(
                (got - want).abs() < 1e-15,
                "width_at({dir:?}, {depth}) = {got}, want {want}"
            );
        }
    }
}

#[test]
fn shift_moves_exactly_one_coordinate_per_direction() {
    for dir in DIRECTIONS {
        let mut p = Pose {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            xa: 4.0,
            ya: 5.0,
            za: 6.0,
        };
        p.shift(dir, 0.5);
        let moved: Vec<usize> = coords(&p)
            .iter()
            .enumerate()
            .filter(|(i, v)| **v != [1.0, 2.0, 3.0, 4.0, 5.0, 6.0][*i])
            .map(|(i, _)| i)
            .collect();
        assert_eq!(moved, vec![dir.index()], "shift({dir:?}) moved {moved:?}");
        assert!((coords(&p)[dir.index()] - [1.5, 2.5, 3.5, 4.5, 5.5, 6.5][dir.index()]).abs() < 1e-12);
    }
}

#[test]
fn unit_pose_deref_mut_edits_the_wrapped_pose() {
    let mut u = UnitPose::from([0.5; 6]);
    u.shift(Direction::ZA_DIR, -0.25);
    assert!((u.za - 0.25).abs() < 1e-12);
}

#[test]
fn array_pose_array_roundtrip_is_bit_identical() {
    let src = [1.25, -2.5, 3.75, -4.125, 5.0, -6.875];
    let back: [f64; 6] = Pose::from(src).into();
    assert_eq!(src.map(f64::to_bits), back.map(f64::to_bits));

    // Same through each role wrapper.
    let physical: [f64; 6] = PhysicalPose::from(src).into();
    let unit: [f64; 6] = UnitPose::from(src).into();
    let range: [f64; 6] = PoseRange::from(src).into();
    assert_eq!(src.map(f64::to_bits), physical.map(f64::to_bits));
    assert_eq!(src.map(f64::to_bits), unit.map(f64::to_bits));
    assert_eq!(src.map(f64::to_bits), range.map(f64::to_bits));
}

#[test]
fn axis_seq_zxy_matches_the_deleted_string_path() {
    // The old `axes_from_str("ZXY")` yielded z, x, y unit axes in that order;
    // from_euler_ordered(ZXY, angles, extrinsic) must rotate about z, then x,
    // then y (intrinsic composition r_z * r_x * r_y for extrinsic=false ...
    // pinned here against an explicit axis-angle product).
    let angles = [10.0, 20.0, 30.0];
    let got = from_euler_ordered(AxisSeq::ZXY, angles, false);
    let rz = Rotation3::from_axis_angle(&Vector3::z_axis(), 10.0_f64.to_radians());
    let rx = Rotation3::from_axis_angle(&Vector3::x_axis(), 20.0_f64.to_radians());
    let ry = Rotation3::from_axis_angle(&Vector3::y_axis(), 30.0_f64.to_radians());
    let want = rz * rx * ry;
    assert_eq!(got, want);
}
