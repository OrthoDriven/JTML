//! POH selection tests: the Jones potentially-optimal oracle cross-checked
//! against the convex-hull implementation, plus hull edge cases.

use super::*;
use proptest::prelude::*;

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
    convex_hull(pts)
        .into_iter()
        .map(|p| (p.size, p.cost))
        .collect()
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
}
