//! Hyperbox geometry tests: the size formula, the min-depth split, and the
//! trisection lattice invariants.

use super::*;
use crate::fixtures::{canonical_size, coords, on_lattice, splat};
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
        for (c, d) in coords(&p).iter().zip(depths) {
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

#[test]
fn hyperbox_size_matches_depth_formula() {
    let unit = box_at(splat(0.5), [0; 6]);
    let unit_size = unit.size();
    assert!(
        (unit_size - 6.0_f64.sqrt()).abs() < 1e-12,
        "unit box size {unit_size} != sqrt(6)"
    );

    let hb = Hyperbox {
        cost_at_center: 25.0,
        center: Pose {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            xa: 0.0,
            ya: 0.0,
            za: 0.0,
        },
        depths: [3, 3, 3, 3, 3, 9],
    };
    // size = sqrt(Σ 3^{-2 d_i}) = sqrt(5/729 + 1/387420489)
    let expected = (5.0_f64 / 729.0 + 1.0 / 387_420_489.0).sqrt();
    let got = hb.size();
    assert!(
        (got - expected).abs() < 1e-12,
        "size {got} != expected {expected}"
    );
}
