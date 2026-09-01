use super::*;
use crate::fixtures::{denorm, pose, splat};
use crate::pose::{Pose, UnitPose, DIRECTIONS};

#[test]
fn camera_centered_zero_start_is_defined_plus_z_not_nan() {
    let space = SearchSpace::new(
        Pose::default().into(),
        splat(1.0).into(),
        RotationRepresentation::Euler,
        TranslationRepresentation::CameraCentered,
    );
    let unit = UnitPose::from([0.5, 0.5, 0.0, 0.5, 0.5, 0.5]);
    let physical = space.physical_pose(unit);

    // The R12 declared exception: the mapping is all-finite even with a
    // zero starting translation (the +Z fallback ray).
    assert!(
        physical.to_array().iter().all(|v| v.is_finite()),
        "physical pose must be all-finite, got {:?}",
        physical.to_array()
    );

    // Pin the fallback ray: unit z = 0.0 means local dz = -1 along +Z basis.
    assert!(
        (physical.z - (-1.0)).abs() < 1e-12,
        "z should be -1.0 (unit z=0 ⇒ local dz=-1 along +Z basis), got {}",
        physical.z,
    );
    assert!(
        physical.x.abs() < 1e-12,
        "x should be 0.0, got {}",
        physical.x,
    );
    assert!(
        physical.y.abs() < 1e-12,
        "y should be 0.0, got {}",
        physical.y,
    );
}

#[test]
fn euler_euclidean_matches_fixture_denorm() {
    let start = pose([1.0, -2.0, 3.0, 30.0, -45.0, 60.0]);
    let range = pose([5.0, 4.0, 3.0, 2.0, 1.0, 0.5]);
    let space = SearchSpace::new(
        start.into(),
        range.into(),
        RotationRepresentation::Euler,
        TranslationRepresentation::PureEuclidean,
    );

    let unit_poses = [
        UnitPose::from([0.5, 0.5, 0.5, 0.5, 0.5, 0.5]),
        UnitPose::from([0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
        UnitPose::from([1.0, 1.0, 1.0, 1.0, 1.0, 1.0]),
        UnitPose::from([0.25, 0.75, 0.5, 0.0, 1.0, 0.5]),
    ];

    for unit in unit_poses {
        let space_result = space.physical_pose(unit);
        let fixture_result: PhysicalPose = denorm(start, range, *unit).into();
        // Bit-identical arithmetic; abs diff < 1e-12 to be safe.
        let space_arr = space_result.to_array();
        let fixture_arr = fixture_result.to_array();
        for (i, (s, f)) in space_arr.iter().zip(fixture_arr.iter()).enumerate() {
            assert!(
                (s - f).abs() < 1e-12,
                "axis {i}: space={s} fixture={f}, diff={}",
                (s - f).abs(),
            );
        }
    }
}

#[test]
fn width_at_delegates_to_pose_range() {
    let range: PoseRange = Pose {
        x: 2.0,
        y: -1.5,
        z: 0.25,
        xa: 10.0,
        ya: 0.0,
        za: 5.0,
    }
    .into();
    let space = SearchSpace::new(
        Pose::default().into(),
        range,
        RotationRepresentation::Euler,
        TranslationRepresentation::PureEuclidean,
    );
    for dir in DIRECTIONS {
        for depth in [0, 2, 5] {
            let space_w = space.width_at(dir, depth);
            let range_w = range.width_at(dir, depth);
            assert_eq!(
                space_w.to_bits(),
                range_w.to_bits(),
                "width_at({dir:?}, {depth}) mismatch: space={space_w}, range={range_w}",
            );
        }
    }
}

#[test]
#[expect(clippy::panic, reason = "else branch panic is the assertion under test")]
fn maps_are_materialized_per_mode() {
    // Default: Euler + PureEuclidean
    let space = SearchSpace::new(
        Pose::default().into(),
        splat(1.0).into(),
        RotationRepresentation::Euler,
        TranslationRepresentation::PureEuclidean,
    );
    assert!(matches!(space.rotation, RotationMap::Euler));
    assert!(matches!(space.translation, TranslationMap::Euclidean));

    // AxisAngle + CameraCentered with nonzero start
    let start = pose([1.0, 2.0, 3.0, 10.0, 20.0, 30.0]);
    let space2 = SearchSpace::new(
        start.into(),
        splat(1.0).into(),
        RotationRepresentation::AxisAngle,
        TranslationRepresentation::CameraCentered,
    );
    assert!(matches!(
        space2.rotation,
        RotationMap::AxisAngle { .. }
    ));
    assert!(matches!(
        space2.translation,
        TranslationMap::CameraCentered { .. }
    ));

    // Pin the AxisAngle start rotation: must equal from_euler_ordered(ZXY, [start.za, start.xa, start.ya], false)
    let expected_rotation =
        from_euler_ordered(AxisSeq::ZXY, [start.za, start.xa, start.ya], false);
    if let RotationMap::AxisAngle { start: actual_rotation } = &space2.rotation {
        let expected_arr = expected_rotation.to_homogeneous();
        let actual_arr = actual_rotation.to_homogeneous();
        for (i, (e, a)) in expected_arr.iter().zip(actual_arr.iter()).enumerate() {
            assert!(
                (e - a).abs() < 1e-12,
                "rotation element {i}: expected {e}, got {a}",
            );
        }
    } else {
        panic!("expected RotationMap::AxisAngle, got {:?}", space2.rotation);
    }
}
