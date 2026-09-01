#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::ShiftedSphere;
    use crate::cost::Cost;
    use crate::test_support::{coords, on_lattice, show, splat, zero};
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    fn iter_boxes(opt: &DirectOptimizer) -> impl Iterator<Item = &Hyperbox> {
        opt.boxes.values().flat_map(|row| row.values())
    }

    /// Jones potentially-optimal test on `(size, cost)` representatives.
    /// `j` is POH iff `K_lo <= K_hi` and `K_hi > 0`, with same-size worse
    /// points treated as dominated.
    fn jones_poh(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        for (j, &(dj, fj)) in points.iter().enumerate() {
            let dominated = points
                .iter()
                .enumerate()
                .any(|(i, &(di, fi))| i != j && (di - dj).abs() <= 1e-15 && fi < fj);
            if dominated {
                continue;
            }
            let mut k_lo = f64::NEG_INFINITY;
            let mut k_hi = f64::INFINITY;
            for (i, &(di, fi)) in points.iter().enumerate() {
                if i == j {
                    continue;
                }
                let dd = dj - di;
                if dd > 1e-15 {
                    k_lo = k_lo.max((fj - fi) / dd);
                } else if dd < -1e-15 {
                    k_hi = k_hi.min((fi - fj) / (di - dj));
                }
            }
            if k_lo <= k_hi && k_hi > 0.0 {
                out.push((dj, fj));
            }
        }
        out
    }

    fn hull_pairs(pts: &[POHPoint]) -> Vec<(f64, f64)> {
        DirectOptimizer::convex_hull(pts)
            .into_iter()
            .map(|p| (p.size, p.cost))
            .collect()
    }

    fn volume_checksum(opt: &DirectOptimizer) -> Result<(u128, u128), &'static str> {
        let sums: Vec<u32> = iter_boxes(opt).map(|b| b.depths.iter().sum()).collect();
        let Some(&s_max) = sums.iter().max() else {
            return Err("no boxes");
        };
        let rhs = 3u128.checked_pow(s_max).ok_or("pow overflow")?;
        let mut lhs = 0u128;
        for s in sums {
            let term = 3u128.checked_pow(s_max - s).ok_or("pow overflow")?;
            lhs = lhs.checked_add(term).ok_or("add overflow")?;
        }
        Ok((lhs, rhs))
    }

    #[test]
    fn jones_rejects_the_descending_left_hull() {
        // Larger box is cheaper: the small expensive vertices are NOT POH.
        let pts = [
            POHPoint {
                size: 1.0,
                cost: 10.0,
            },
            POHPoint {
                size: 2.0,
                cost: 5.0,
            },
            POHPoint {
                size: 3.0,
                cost: 0.0,
            },
        ];
        let hull = hull_pairs(&pts);
        let jones = jones_poh(&[(1.0, 10.0), (2.0, 5.0), (3.0, 0.0)]);
        assert_eq!(jones, vec![(3.0, 0.0)], "oracle sanity");
        assert_eq!(
            hull, jones,
            "convex_hull returned {hull:?}, Jones POH is {jones:?} — \
             drop vertices left of the global-min-cost hull vertex"
        );
    }

    #[test]
    fn jones_keeps_the_increasing_right_hull() {
        let pts = [
            POHPoint {
                size: 1.0,
                cost: 0.0,
            },
            POHPoint {
                size: 2.0,
                cost: 1.0,
            },
            POHPoint {
                size: 3.0,
                cost: 4.0,
            },
        ];
        let mut hull = hull_pairs(&pts);
        let mut jones = jones_poh(&[(1.0, 0.0), (2.0, 1.0), (3.0, 4.0)]);
        hull.sort_by(|a, b| a.0.total_cmp(&b.0));
        jones.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(hull, jones);
    }

    #[test]
    fn hull_of_one_and_two_points() {
        let one = [POHPoint {
            size: 1.5,
            cost: 2.0,
        }];
        assert_eq!(hull_pairs(&one).len(), 1);
        let two = [
            POHPoint {
                size: 1.0,
                cost: 1.0,
            },
            POHPoint {
                size: 2.0,
                cost: 2.5,
            },
        ];
        assert_eq!(hull_pairs(&two).len(), 2);
    }

    #[test]
    fn collinear_lower_hull_keeps_interior_vertices() {
        // Jones selects every collinear lower-hull point. `cross <= 0` pops them.
        let pts = [
            POHPoint {
                size: 1.0,
                cost: 1.0,
            },
            POHPoint {
                size: 2.0,
                cost: 2.0,
            },
            POHPoint {
                size: 3.0,
                cost: 3.0,
            },
        ];
        let hull = hull_pairs(&pts);
        assert_eq!(
            hull.len(),
            3,
            "collinear interior vertex dropped ({hull:?}); \
             Jones keeps all of them — change the comparison deliberately if this is DIRECT-l"
        );
    }

    #[test]
    fn boxes_tile_the_unit_cube_exactly() {
        let mut opt = DirectOptimizer::new(splat(5.0), zero(), 80);
        opt.run(&ShiftedSphere {
            shift: Pose {
                x: 1.7,
                y: 0.4,
                z: -2.1,
                xa: 0.8,
                ya: -1.3,
                za: 2.6,
            },
        });
        let (lhs, rhs) = volume_checksum(&opt).expect("volume checksum");
        assert_eq!(lhs, rhs, "Σ 3^{{Smax-Σd}} = {lhs} != 3^{{Smax}} = {rhs}");
    }

    #[test]
    fn every_box_center_is_on_the_trisection_lattice() {
        let mut opt = DirectOptimizer::new(splat(5.0), zero(), 80);
        opt.run(&ShiftedSphere {
            shift: Pose {
                x: 1.7,
                y: 0.4,
                z: -2.1,
                xa: 0.8,
                ya: -1.3,
                za: 2.6,
            },
        });
        for hb in iter_boxes(&opt) {
            for (c, d) in coords(&hb.center).iter().zip(hb.depths) {
                assert!(
                    on_lattice(*c, d),
                    "box center {} depth {d:?} off-lattice",
                    show(&hb.center)
                );
            }
        }
    }

    #[test]
    fn no_two_boxes_share_a_unit_center() {
        let mut opt = DirectOptimizer::new(splat(5.0), zero(), 4_000);
        opt.run(&ShiftedSphere {
            shift: Pose {
                x: 1.7,
                y: 0.4,
                z: -2.1,
                xa: 0.8,
                ya: -1.3,
                za: 2.6,
            },
        });
        let mut seen = BTreeSet::new();
        for hb in iter_boxes(&opt) {
            let key = coords(&hb.center).map(f64::to_bits);
            assert!(
                seen.insert(key),
                "duplicate unit center {}",
                show(&hb.center)
            );
        }
    }

    #[test]
    fn each_box_depths_differ_by_at_most_one() {
        let mut opt = DirectOptimizer::new(splat(5.0), zero(), 120);
        opt.run(&ShiftedSphere {
            shift: Pose {
                x: 1.7,
                y: 0.4,
                z: -2.1,
                xa: 0.8,
                ya: -1.3,
                za: 2.6,
            },
        });
        for hb in iter_boxes(&opt) {
            let min = hb.depths.iter().copied().min().unwrap_or(0);
            let max = hb.depths.iter().copied().max().unwrap_or(0);
            assert!(
                max - min <= 1,
                "depths {:?} violate min-depth split",
                hb.depths
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 4096, ..ProptestConfig::default() })]

        #[test]
        fn convex_hull_matches_jones_on_unique_sizes(
            raw in proptest::collection::vec((0.1_f64..12.0, -8.0_f64..8.0), 1..12)
        ) {
            // Dedup sizes so the input matches what determine_potentially_optimal feeds.
            let mut pts = raw;
            pts.sort_by(|a, b| a.0.total_cmp(&b.0));
            pts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9);
            prop_assume!(!pts.is_empty());
            let poh: Vec<POHPoint> = pts
                .iter()
                .map(|&(size, cost)| POHPoint { size, cost })
                .collect();
            let mut hull = hull_pairs(&poh);
            let mut jones = jones_poh(&pts);
            hull.sort_by(|a, b| a.0.total_cmp(&b.0));
            jones.sort_by(|a, b| a.0.total_cmp(&b.0));
            prop_assert_eq!(hull, jones);
        }

        #[test]
        fn volume_partition_holds_for_random_runs(
            budget in 8_u32..60,
            shift in proptest::array::uniform6(-4.0_f64..4.0),
        ) {
            let [x, y, z, xa, ya, za] = shift;
            let mut opt = DirectOptimizer::new(splat(5.0), zero(), budget);
            opt.run(&ShiftedSphere { shift: Pose { x, y, z, xa, ya, za } });
            match volume_checksum(&opt) {
                Ok((lhs, rhs)) => prop_assert_eq!(lhs, rhs),
                Err("pow overflow") => {}
                Err(e) => prop_assert!(false, "volume checksum failed: {e}"),
            }
        }
    }

    struct Sphere;
    impl Cost for Sphere {
        fn eval(&self, poses: &[Pose]) -> Vec<f64> {
            poses
                .iter()
                .map(|p| coords(p).iter().map(|v| v * v).sum())
                .collect()
        }
    }

    #[test]
    fn seed_evaluation_counts_as_one_call_when_budget_is_zero() {
        let mut opt = DirectOptimizer::new(splat(5.0), zero(), 0);
        let _ = opt.run(&Sphere);
        assert_eq!(opt.calls, 1, "seed must still be evaluated at budget 0");
    }
}

/// Test-only constructors that pin the rotation representation.
///
/// Production currently always runs `RotationRepresentation::default()`
/// (`Euler`); these seams exist so the test suite can state *which* mode a
/// metamorphic property assumes instead of inheriting it silently.
#[cfg(test)]
impl DirectOptimizer {
    pub(crate) fn new_in_mode(
        range: Pose,
        starting_point: Pose,
        budget: u32,
        rotation: RotationRepresentation,
    ) -> Self {
        let mut opt = Self::new(range, starting_point, budget);
        opt.settings.rotation_style = rotation;
        opt
    }
}

#[cfg(test)]
mod axis_angle_tests {
    //! SO(3) property tests for `RotationRepresentation::AxisAngle`.
    //!
    //! Intended semantics: the unit-space XA/YA/ZA coordinates of a hyperbox
    //! are *not* Euler angles; they denormalize to a scaled-axis (exponential
    //! map) increment `v` (in degrees at the interface, radians internally)
    //! that is applied in the body frame of the starting orientation:
    //!
    //!     R_result = R_start * Exp(v)
    //!
    //! The final rotation is only serialized back to ZXY Euler angles because
    //! the C++/FFI `Pose` interface consumes Euler angles. Euler triples are
    //! non-unique, so every property below compares rotations (SO(3)), never
    //! angle components.
    //!
    //! The project ZXY convention is defined by the C++ renderer
    //! (`RenderEngine::SetPose`: "R*v = RzRxRy*v"):
    //!
    //!     M(xa, ya, za) = Rz(za) * Rx(xa) * Ry(ya)
    //!
    //! Generators stay away from the ZXY gimbal-lock set (|xa| ~ 90 deg) and
    //! deltas are bounded well below a principal-angle ambiguity.

    use super::*;
    use crate::test_support::{pose, show, zero};
    use nalgebra::Rotation3;
    use proptest::prelude::*;

    /// Tolerance on orientation (geodesic) errors, in radians. Comfortably
    /// above f64 round-off through Exp/Euler round-trips (~1e-15), and far
    /// below the smallest semantic mistake worth catching (degrees/radians
    /// confusion is ~57x, a sign flip ~2x the angle).
    const ANG_TOL_RAD: f64 = 1e-9;

    /// Maximum |angle| for generated starting Euler components (degrees).
    /// The middle ZXY angle stays a further 10 deg below the +-90 deg
    /// gimbal-lock set; the outer two only need to avoid the wrap seam.
    const START_LIMIT: f64 = 85.0;
    const START_LIMIT_MID: f64 = 75.0;
    /// Half-width (degrees) of the denormalized axis-angle box used by the
    /// mapping tests, matching realistic tens-of-degrees search ranges.
    const RANGE_DEG: f64 = 20.0;

    /// Reconstruct a rotation from a serialized Pose using the same ZXY
    /// convention the rest of the project uses (C++ `SetPose`:
    /// Rz(za) * Rx(xa) * Ry(ya)).
    fn recon(p: &Pose) -> Rotation3<f64> {
        DirectOptimizer::from_euler_ordered("ZXY", [p.za, p.xa, p.ya], false)
    }

    /// Serialize a rotation to a Pose using production's own inverse path
    /// (`euler_angles_ordered("ZXY", extrinsic=false)`). Translation is zero;
    /// only the rotational half is meaningful. `observable` reports whether
    /// the extraction sat away from gimbal lock.
    fn serialize(r: &Rotation3<f64>) -> (Pose, bool) {
        let (angles, observable) =
            r.euler_angles_ordered(DirectOptimizer::axes_from_str("ZXY"), false);
        (
            Pose {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                xa: angles[1].to_degrees(),
                ya: angles[2].to_degrees(),
                za: angles[0].to_degrees(),
            },
            observable,
        )
    }

    /// Geodesic orientation error between two rotations, in radians.
    ///
    /// Computed as `2 * atan2(||imag(q)||, |w(q)|)` from the quaternion of
    /// the relative rotation. Do NOT replace with `(a.inverse() * b).angle()`:
    /// that is `acos((trace-1)/2)`, which saturates near the identity — fp
    /// noise of size `eps` in the trace shows up as `sqrt(eps) ~ 2e-8` rad
    /// "error" (and NaN when the acos argument lands just above 1), exactly
    /// the regime these property tests probe.
    fn ang_err(a: &Rotation3<f64>, b: &Rotation3<f64>) -> f64 {
        let q = nalgebra::UnitQuaternion::from_rotation_matrix(&(a.inverse() * b));
        2.0 * q.vector().norm().atan2(q.scalar().abs())
    }

    /// Rotation angle of `r` in [0, pi], numerically stable near identity
    /// (same atan2 construction as `ang_err`).
    fn rot_angle(r: &Rotation3<f64>) -> f64 {
        ang_err(&Rotation3::identity(), r)
    }

    /// Optimizer configured for AxisAngle over `range`, centered on `start`
    /// (budget is irrelevant: the mapping is tested directly).
    fn axis_angle_opt(start: Pose, range: Pose) -> DirectOptimizer {
        DirectOptimizer::new_in_mode(range, start, 0, RotationRepresentation::AxisAngle)
    }

    /// Unit-space pose whose rotational coordinates denote the scaled-axis
    /// delta `v_deg` (degrees) under `range` (|v_deg_i| <= range_i).
    fn unit_for_delta(v_deg: [f64; 3], range: &Pose) -> Pose {
        pose([
            0.5,
            0.5,
            0.5,
            0.5 + v_deg[0] / (2.0 * range.xa),
            0.5 + v_deg[1] / (2.0 * range.ya),
            0.5 + v_deg[2] / (2.0 * range.za),
        ])
    }

    /// apply(start, v): production's AxisAngle mapping for a body-frame
    /// scaled-axis delta `v_deg` (degrees), exercised without running DIRECT.
    fn apply(start: Pose, v_deg: [f64; 3]) -> Pose {
        let range = pose([0.0, 0.0, 0.0, RANGE_DEG, RANGE_DEG, RANGE_DEG]);
        axis_angle_opt(start, range).physical_pose_for_eval(unit_for_delta(v_deg, &range))
    }

    fn deg(v: f64) -> f64 {
        v.to_degrees()
    }

    /// C++ `RenderEngine::SetPose` literal (R*v = RzRxRy*v), transcribed from
    /// src/compute/render_engine.cu, as an independent anchor for the ZXY
    /// convention (not derived from nalgebra).
    fn cpp_setpose(xa_deg: f64, ya_deg: f64, za_deg: f64) -> nalgebra::Matrix3<f64> {
        let (sz, cz) = za_deg.to_radians().sin_cos();
        let (sx, cx) = xa_deg.to_radians().sin_cos();
        let (sy, cy) = ya_deg.to_radians().sin_cos();
        nalgebra::Matrix3::new(
            cz * cy - sz * sx * sy,
            -1.0 * sz * cx,
            cz * sy + sz * cy * sx,
            sz * cy + cz * sx * sy,
            cz * cx,
            sz * sy - cz * cy * sx,
            -1.0 * cx * sy,
            sx,
            cx * cy,
        )
    }

    #[test]
    fn project_zxy_convention_is_rz_rx_ry() {
        // Lock: recon(...) == the literal C++ SetPose matrix. If the Rust
        // composition order ever drifts from the C++ consumer, this fails
        // even if nalgebra stays internally self-consistent.
        for [xa, ya, za] in [
            [0.0, 0.0, 0.0],
            [12.5, -40.0, 5.0],
            [-60.0, 70.0, 150.0],
            [89.0, -3.0, -120.0],
        ] {
            let got = recon(&pose([0.0, 0.0, 0.0, xa, ya, za])).into_inner();
            let want = cpp_setpose(xa, ya, za);
            let err = (got - want).abs().max();
            assert!(err < 1e-12, "ZXY convention drift at {xa},{ya},{za}: {err}");
        }
    }

    #[test]
    fn serializer_inverts_reconstruction() {
        // The AxisAngle branch serializes with euler_angles_ordered("ZXY",
        // false); the SO(3) property tests reconstruct with recon(). Those
        // must be inverses up to fp, away from gimbal lock.
        for [xa, ya, za] in [
            [5.0, 12.0, -25.0],
            [-45.0, 60.0, 130.0],
            [70.0, -75.0, 10.0],
        ] {
            let p = pose([0.0, 0.0, 0.0, xa, ya, za]);
            let (q, observable) = serialize(&recon(&p));
            assert!(observable, "gimbal lock at {xa},{ya},{za}");
            let err = ang_err(&recon(&p), &recon(&q));
            assert!(err < 1e-12, "round-trip error {err} at {xa},{ya},{za}");
        }
    }

    #[test]
    fn default_rotation_style_is_euler_and_tests_override_it() {
        // Guards against Euler-branch code masquerading as AxisAngle tests.
        let opt = DirectOptimizer::new(zero(), zero(), 0);
        assert!(matches!(
            opt.settings.rotation_style,
            RotationRepresentation::Euler
        ));
        let aa = axis_angle_opt(zero(), pose([0.0, 0.0, 0.0, 5.0, 5.0, 5.0]));
        assert!(matches!(
            aa.settings.rotation_style,
            RotationRepresentation::AxisAngle
        ));
    }

    #[test]
    fn zero_delta_is_identity_at_a_nontrivial_start() {
        let start = pose([1.0, -2.0, 3.0, 30.0, -45.0, 60.0]);
        let got = apply(start, [0.0, 0.0, 0.0]);
        let err = ang_err(&recon(&start), &recon(&got));
        assert!(err < 1e-12, "zero delta moved rotation by {err} rad");
        assert!(
            (got.x - start.x).abs() < 1e-12
                && (got.y - start.y).abs() < 1e-12
                && (got.z - start.z).abs() < 1e-12,
            "zero delta moved translation: {} vs {}",
            show(&got),
            show(&start)
        );
    }

    #[test]
    fn known_local_composition() {
        // Start 90 deg about world Z; delta 90 deg about local Z:
        // R_start * Exp(v) = Rz(180), so the result must map x -> -x.
        let start = pose([0.0, 0.0, 0.0, 0.0, 0.0, 90.0]);
        let got = apply(start, [0.0, 0.0, 90.0]);
        let r = recon(&got);
        let v = r * Vector3::x();
        let err = (v - Vector3::new(-1.0, 0.0, 0.0)).norm();
        assert!(err < 1e-9, "expected x -> -x, got {v:?} (err {err})");
        // Note: for this commuting pair the world-frame semantics
        // Exp(v) * R_start agrees too; locality is pinned by the
        // equivariance proptest, not this case.
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 4096, ..ProptestConfig::default() })]

        /// PRIMARY CONTRACT: R_start^-1 * R_result ~ Exp(v).
        /// Verifies body-frame composition, multiplication order, radian
        /// handling, signs, component order, axis direction, magnitude, and
        /// the Euler serialization round-trip all at once.
        #[test]
        fn observed_local_delta_equals_requested_delta(
            sza in -START_LIMIT..START_LIMIT,
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            vx in -RANGE_DEG..RANGE_DEG,
            vy in -RANGE_DEG..RANGE_DEG,
            vz in -RANGE_DEG..RANGE_DEG,
        ) {
            let start = pose([0.0, 0.0, 0.0, sxa, sya, sza]);
            let v = [vx, vy, vz];
            let result = apply(start, v);
            let r_start = recon(&start);
            let r_result = recon(&result);
            let observed = r_start.inverse() * r_result;
            let wanted = Rotation3::from_scaled_axis(Vector3::new(
                vx.to_radians(), vy.to_radians(), vz.to_radians(),
            ));
            let err = ang_err(&observed, &wanted);
            prop_assert!(
                err < ANG_TOL_RAD,
                "local delta mismatch: start Euler (xa,ya,za) = ({},{},{}) deg; \
                 requested scaled-axis v = ({},{},{}) deg; angular error = {:.3e} deg \
                 (tol {:.1e} deg)",
                sxa, sya, sza, vx, vy, vz, deg(err), deg(ANG_TOL_RAD)
            );
        }

        /// Zero delta keeps the orientation (SO(3) only; Euler components may
        /// re-branch) and the translation stays a plain affine denorm.
        #[test]
        fn zero_delta_preserves_orientation(
            sza in -START_LIMIT..START_LIMIT,
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            tx in -50.0..50.0,
            ty in -50.0..50.0,
            tz in -50.0..50.0,
        ) {
            let start = pose([tx, ty, tz, sxa, sya, sza]);
            let got = apply(start, [0.0, 0.0, 0.0]);
            let err = ang_err(&recon(&start), &recon(&got));
            prop_assert!(err < ANG_TOL_RAD, "zero delta moved rotation by {:.3e} deg", deg(err));
            prop_assert!((got.x - tx).abs() < 1e-9);
            prop_assert!((got.y - ty).abs() < 1e-9);
            prop_assert!((got.z - tz).abs() < 1e-9);
        }

        /// apply(apply(start, v), -v) returns to the starting orientation.
        /// Validates group-inverse behavior (note: does NOT by itself
        /// distinguish local from world-frame composition; the primary
        /// contract and the equivariance property do).
        #[test]
        fn delta_then_inverse_returns_to_start(
            sza in -START_LIMIT..START_LIMIT,
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            vx in -RANGE_DEG..RANGE_DEG,
            vy in -RANGE_DEG..RANGE_DEG,
            vz in -RANGE_DEG..RANGE_DEG,
        ) {
            let start = pose([0.0, 0.0, 0.0, sxa, sya, sza]);
            let v = [vx, vy, vz];
            let p1 = apply(start, v);
            let p2 = apply(p1, [-vx, -vy, -vz]);
            let err = ang_err(&recon(&start), &recon(&p2));
            prop_assert!(
                err < ANG_TOL_RAD,
                "v then -v did not return to start: err {:.3e} deg; start ({},{},{})",
                deg(err), sxa, sya, sza
            );
        }

        /// angle(R_start^-1 * R_result) == ||v_rad||: the scaled-axis norm
        /// IS the rotation angle, so this kills degree/radian confusion in
        /// either direction (denormalization or exponential map).
        #[test]
        fn relative_angle_equals_scaled_axis_norm(
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            sza in -START_LIMIT..START_LIMIT,
            vx in -RANGE_DEG..RANGE_DEG,
            vy in -RANGE_DEG..RANGE_DEG,
            vz in -RANGE_DEG..RANGE_DEG,
        ) {
            let start = pose([0.0, 0.0, 0.0, sxa, sya, sza]);
            let v = [vx, vy, vz];
            let vr = Vector3::new(vx.to_radians(), vy.to_radians(), vz.to_radians());
            let result = apply(start, v);
            let relative = recon(&start).inverse() * recon(&result);
            // ||v|| <= 20*sqrt(3) deg < pi, so the principal angle is unambiguous.
            prop_assert!(vr.norm() < std::f64::consts::PI);
            let err = (rot_angle(&relative) - vr.norm()).abs();
            prop_assert!(
                err < ANG_TOL_RAD,
                "relative angle {} rad != scaled-axis norm {} rad (diff {:.3e})",
                rot_angle(&relative), vr.norm(), err
            );
        }

        /// Two increments about the SAME axis add: applying a*n then b*n
        /// sequentially equals applying (a+b)*n once.
        #[test]
        fn same_axis_increments_are_additive(
            axis_idx in 0usize..3,
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            sza in -START_LIMIT..START_LIMIT,
            a in -15.0f64..15.0,
            b in -15.0f64..15.0,
        ) {
            let mut n = [0.0f64; 3];
            n[axis_idx] = 1.0;
            let start = pose([0.0, 0.0, 0.0, sxa, sya, sza]);
            let seq = apply(
                apply(start, [a * n[0], a * n[1], a * n[2]]),
                [b * n[0], b * n[1], b * n[2]],
            );
            let sum = apply(start, [(a + b) * n[0], (a + b) * n[1], (a + b) * n[2]]);
            let err = ang_err(&recon(&seq), &recon(&sum));
            prop_assert!(
                err < ANG_TOL_RAD,
                "same-axis additivity broken (axis {axis_idx}, a {a}, b {b}): err {:.3e} deg",
                deg(err)
            );
        }

        /// LOCAL-frame equivariance: for any world rotation Q,
        /// apply(Q * R, v) == Q * apply(R, v) — the scaled-axis perturbation
        /// stays attached to the object frame and is natural in the world
        /// frame. Fails for the world-frame semantics Exp(v) * R whenever Q
        /// does not commute with the rotation, so this is the metamorphic
        /// guard against silently switching multiplication sides.
        #[test]
        fn delta_is_local_under_world_frame_change(
            q1 in -160.0f64..160.0,
            q2 in -60.0f64..60.0,
            q3 in -160.0f64..160.0,
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            sza in -START_LIMIT..START_LIMIT,
            vx in -RANGE_DEG..RANGE_DEG,
            vy in -RANGE_DEG..RANGE_DEG,
            vz in -RANGE_DEG..RANGE_DEG,
        ) {
            let q = DirectOptimizer::from_euler_ordered("ZXY", [q1, q2, q3], false);
            let start = pose([0.0, 0.0, 0.0, sxa, sya, sza]);
            let r_start = recon(&start);
            let v = [vx, vy, vz];

            // Rotate the object's world frame by Q, then perturb locally.
            let (moved_start, observable) = serialize(&(q * r_start));
            prop_assume!(observable);
            let lhs = recon(&apply(moved_start, v));

            // Perturb in the original world frame, then rotate the outcome.
            let rhs = q * recon(&apply(start, v));

            let err = ang_err(&lhs, &rhs);
            prop_assert!(
                err < ANG_TOL_RAD,
                "local-frame equivariance broken: err {:.3e} deg (start ({},{},{}) v ({},{},{}) Q Euler ({},{},{}))",
                deg(err), sxa, sya, sza, vx, vy, vz, q1, q2, q3
            );
        }

        /// Pure-axis sanity: v = (theta,0,0) must be a rotation by theta
        /// about the LOCAL X of the start frame (likewise Y, Z). Overlaps
        /// with the primary contract but isolates component-order bugs with
        /// far more readable failure output.
        #[test]
        fn pure_axis_deltas_rotate_about_local_axis(
            axis_idx in 0usize..3,
            sxa in -START_LIMIT_MID..START_LIMIT_MID,
            sya in -START_LIMIT..START_LIMIT,
            sza in -START_LIMIT..START_LIMIT,
            theta in -20.0f64..20.0,
        ) {
            let mut v = [0.0f64; 3];
            v[axis_idx] = theta;
            let start = pose([0.0, 0.0, 0.0, sxa, sya, sza]);
            let result = apply(start, v);
            let relative = recon(&start).inverse() * recon(&result);
            let wanted = match axis_idx {
                0 => Rotation3::from_axis_angle(&Vector3::x_axis(), theta.to_radians()),
                1 => Rotation3::from_axis_angle(&Vector3::y_axis(), theta.to_radians()),
                _ => Rotation3::from_axis_angle(&Vector3::z_axis(), theta.to_radians()),
            };
            let err = ang_err(&relative, &wanted);
            prop_assert!(
                err < ANG_TOL_RAD,
                "pure axis {axis_idx} at theta {theta} deg from start ({},{},{}): err {:.3e} deg",
                sxa, sya, sza, deg(err)
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{canonical_size, on_lattice, splat};
    use proptest::prelude::*;

    fn box_at(center: Pose, depths: [u32; 6]) -> Hyperbox {
        Hyperbox {
            cost_at_center: 0.0,
            center,
            depths,
        }
    }

    #[test]
    fn unit_box_size_is_sqrt_6() {
        let hb = box_at(splat(0.5), [0; 6]);
        let got = hb.size();
        assert!(
            (got - 6.0_f64.sqrt()).abs() < 1e-12,
            "unit size {got} != sqrt(6)"
        );
    }

    #[test]
    fn size_is_bit_identical_under_depth_permutation() {
        let a = [1u32, 2, 0, 3, 0, 4];
        let mut b = a;
        b.swap(0, 1);
        b.swap(2, 5);
        let sa = box_at(splat(0.5), a).size();
        let sb = box_at(splat(0.5), b).size();
        assert_eq!(
            sa.to_bits(),
            sb.to_bits(),
            "size() depends on depth order: {sa} vs {sb} (a={a:?} b={b:?})"
        );
    }

    #[test]
    fn longest_axis_picks_the_min_depth_axis_and_trisect_shrinks() {
        let parent = box_at(splat(0.5), [2, 0, 1, 3, 1, 4]);
        let parent_size = parent.size();

        let axis = parent.longest_axis();
        assert_eq!(axis, 1);

        let (center, [pos, neg]) = parent.trisect(axis);

        let changed: Vec<usize> = center
            .depths
            .iter()
            .zip([2u32, 0, 1, 3, 1, 4])
            .enumerate()
            .filter(|(_, (now, was))| *now != was)
            .map(|(i, _)| i)
            .collect();

        assert_eq!(changed, vec![1]);
        assert_eq!(center.depths[1], 1);
        assert_eq!(pos.depths, center.depths);
        assert_eq!(neg.depths, center.depths);
        assert!(center.size() < parent_size);
        assert!(canonical_size(pos.depths) < parent_size);
        assert!(canonical_size(neg.depths) < parent_size);
    }

    #[test]
    fn trisect_children_sit_on_the_center_lattice() {
        let parent = box_at(splat(0.5), [0; 6]);
        let axis = parent.longest_axis();
        let (center, [pos, neg]) = parent.trisect(axis);
        for (p, depths) in [
            (center.center, center.depths),
            (pos.center, pos.depths),
            (neg.center, neg.depths),
        ] {
            for (c, d) in crate::test_support::coords(&p).iter().zip(depths) {
                assert!(
                    on_lattice(*c, d),
                    "center coord {c} at depth {d} is off-lattice"
                );
            }
        }
    }

    #[test]
    fn repeated_min_depth_split_keeps_depths_within_one() {
        let mut hb = box_at(splat(0.5), [0; 6]);
        for _ in 0..18 {
            let axis = hb.longest_axis();
            let (next, _) = hb.trisect(axis);
            hb = next;
            let min = hb.depths.iter().copied().min().unwrap_or(0);
            let max = hb.depths.iter().copied().max().unwrap_or(0);
            assert!(
                max - min <= 1,
                "depths {:?} drifted more than 1 apart",
                hb.depths
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

        #[test]
        fn size_matches_canonical_sorted_sum(d0 in 0u32..8, d1 in 0u32..8, d2 in 0u32..8,
                                             d3 in 0u32..8, d4 in 0u32..8, d5 in 0u32..8) {
            let depths = [d0, d1, d2, d3, d4, d5];
            let got = box_at(splat(0.5), depths).size();
            let want = canonical_size(depths);
            prop_assert!(
                (got - want).abs() < 1e-12,
                "size {got} != canonical {want} for {depths:?}"
            );
        }
    }
}
