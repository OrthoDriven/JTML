//! Shared helpers for the optimizer test suites. Compiled only under `cargo test`.

use crate::pose::Pose;

/// `(size, cost)` scatter plots of a box tree via plotters.
pub(crate) mod viz {
    use std::error::Error;
    use std::path::Path;

    use plotters::prelude::*;

    use crate::direct::poh::{convex_hull, POHPoint};
    use crate::direct::tree::Hyperbox;
    use crate::direct::DirectOptimizer;

    /// The three layers of the (size, cost) diagram.
    struct Layers {
        /// Every box with a finite size and cost.
        all: Vec<(f64, f64)>,
        /// Cheapest box per size column — exactly what `determine_potentially_optimal`
        /// pulls out of the `BTreeMap` before hulling.
        candidates: Vec<(f64, f64)>,
        /// What `convex_hull` actually returns.
        hull: Vec<(f64, f64)>,
    }

    fn layers(boxes: &[Hyperbox]) -> Layers {
        let mut all: Vec<(f64, f64)> = boxes
            .iter()
            .map(|b| (b.size(), b.cost_at_center))
            .filter(|(s, c)| s.is_finite() && c.is_finite())
            .collect();
        // size ascending, then cost ascending, so the first entry of each column
        // is that column's minimum
        all.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));

        let mut candidates: Vec<(f64, f64)> = Vec::new();
        for &(size, cost) in &all {
            // group on the bit pattern, matching BTreeMap<OrderedFloat<f64>, _>
            let same_column = candidates
                .last()
                .is_some_and(|&(prev, _)| prev.to_bits() == size.to_bits());
            if !same_column {
                candidates.push((size, cost));
            }
        }

        let poh: Vec<POHPoint> = candidates
            .iter()
            .map(|&(size, cost)| POHPoint { size, cost })
            .collect();
        let hull = convex_hull(&poh)
            .into_iter()
            .map(|p| (p.size, p.cost))
            .collect();

        Layers {
            all,
            candidates,
            hull,
        }
    }

    /// Pad a range by 5%, widening degenerate (all-equal) ranges so plotters
    /// doesn't get a zero-height axis — common when the cost is flat.
    fn bounds(vals: impl Iterator<Item = f64>) -> (f64, f64) {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for v in vals {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        let span = hi - lo;
        if span <= f64::EPSILON * hi.abs().max(1.0) {
            let pad = hi.abs().max(1.0) * 0.1;
            return (lo - pad, hi + pad);
        }

        (lo - span * 0.05, hi + span * 0.05)
    }

    pub fn draw_2d_graph(boxes: &[Hyperbox], path: &Path) -> Result<(), Box<dyn Error>> {
        let l = layers(boxes);
        if l.all.is_empty() {
            return Err("no boxes with finite size and cost".into());
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }

        let root = SVGBackend::new(path, (900, 640)).into_drawing_area();
        root.fill(&WHITE)?;

        let (x_lo, x_hi) = bounds(l.all.iter().map(|p| p.0));
        let (y_lo, y_hi) = bounds(l.all.iter().map(|p| p.1));

        let caption = format!(
            "{} boxes / {} size columns / {} potentially optimal",
            l.all.len(),
            l.candidates.len(),
            l.hull.len()
        );
        let mut chart = ChartBuilder::on(&root)
            .caption(caption, ("sans-serif", 26).into_font())
            .margin(12)
            .x_label_area_size(46)
            .y_label_area_size(68)
            .build_cartesian_2d(x_lo..x_hi, y_lo..y_hi)?;

        chart
            .configure_mesh()
            .x_desc("box size  d = sqrt(Σ 3^(-2·dᵢ))")
            .y_desc("cost at center")
            .draw()?;

        let grey = RGBColor(180, 180, 180);
        chart
            .draw_series(
                l.all
                    .iter()
                    .map(|&(x, y)| Circle::new((x, y), 2, grey.filled())),
            )?
            .label(format!("all boxes ({})", l.all.len()))
            .legend(move |(x, y)| Circle::new((x + 10, y), 2, grey.filled()));

        chart
            .draw_series(
                l.candidates
                    .iter()
                    .map(|&(x, y)| Circle::new((x, y), 4, BLUE.stroke_width(2))),
            )?
            .label(format!("column minima ({})", l.candidates.len()))
            .legend(|(x, y)| Circle::new((x + 10, y), 4, BLUE.stroke_width(2)));

        chart.draw_series(LineSeries::new(l.hull.iter().copied(), RED.stroke_width(2)))?;
        chart
            .draw_series(
                l.hull
                    .iter()
                    .map(|&(x, y)| Circle::new((x, y), 5, RED.filled())),
            )?
            .label(format!("potentially optimal ({})", l.hull.len()))
            .legend(|(x, y)| Circle::new((x + 10, y), 5, RED.filled()));

        chart
            .configure_series_labels()
            .background_style(WHITE.mix(0.85))
            .border_style(BLACK)
            .position(SeriesLabelPosition::UpperLeft)
            .draw()?;

        root.present()?;

        Ok(())
    }

    impl DirectOptimizer {
        pub(crate) fn snapshot_boxes(&self) -> Vec<Hyperbox> {
            self.boxes
                .values()
                .flat_map(|row| row.values().copied())
                .collect()
        }
    }

    #[expect(clippy::panic, reason = "viz helper — panicking is acceptable in test support code")]
    pub fn plot_boxes(opt: &DirectOptimizer, name: &str) {
        let path = std::path::PathBuf::from("plots").join(format!("{name}.svg"));
        draw_2d_graph(&opt.snapshot_boxes(), &path)
            .unwrap_or_else(|e| panic!("failed to plot {}: {e}", path.display()));
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn draw() -> Result<(), Box<dyn std::error::Error>> {
            let root = SVGBackend::new("plotters-doc-data/0.svg", (640, 480)).into_drawing_area();
            root.fill(&WHITE)?;
            let mut chart = ChartBuilder::on(&root)
                .caption("y=x^2", ("sans-serif", 50).into_font())
                .margin(5)
                .x_label_area_size(30)
                .y_label_area_size(30)
                .build_cartesian_2d(-1f32..1f32, -0.1f32..1f32)?;

            chart.configure_mesh().draw()?;

            chart
                .draw_series(LineSeries::new(
                    (-50..=50).map(|x| x as f32 / 50.0).map(|x| (x, x * x)),
                    RED,
                ))?
                .label("y = x^2")
                .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED));

            chart
                .configure_series_labels()
                .background_style(WHITE.mix(0.8))
                .border_style(BLACK)
                .draw()?;

            root.present()?;

            Ok(())
        }
    }
}

pub(crate) fn coords(p: &Pose) -> [f64; 6] {
    [p.x, p.y, p.z, p.xa, p.ya, p.za]
}

pub(crate) fn pose(c: [f64; 6]) -> Pose {
    let [x, y, z, xa, ya, za] = c;
    Pose {
        x,
        y,
        z,
        xa,
        ya,
        za,
    }
}

pub(crate) fn splat(v: f64) -> Pose {
    pose([v; 6])
}

pub(crate) fn zero() -> Pose {
    splat(0.0)
}

pub(crate) fn show(p: &Pose) -> String {
    format!(
        "({:.6},{:.6},{:.6},{:.6},{:.6},{:.6})",
        p.x, p.y, p.z, p.xa, p.ya, p.za
    )
}

pub(crate) fn dist(a: &Pose, b: &Pose) -> f64 {
    coords(a)
        .iter()
        .zip(coords(b))
        .map(|(u, v)| (u - v) * (u - v))
        .sum::<f64>()
        .sqrt()
}

/// physical[i] = start[i] + (unit[i] - 0.5) * 2 * range[i]
pub(crate) fn denorm(start: Pose, range: Pose, unit: Pose) -> Pose {
    let s = coords(&start);
    let r = coords(&range);
    let u = coords(&unit);
    let mut out = [0.0; 6];
    for ((slot, si), (ui, ri)) in out.iter_mut().zip(s).zip(u.iter().zip(r)) {
        *slot = si + (ui - 0.5) * 2.0 * ri;
    }
    pose(out)
}

/// Inverse of `denorm`. Axes with `range_i == 0` map back to 0.5.
pub(crate) fn invert(start: Pose, range: Pose, physical: Pose) -> Pose {
    let s = coords(&start);
    let r = coords(&range);
    let p = coords(&physical);
    let mut out = [0.5; 6];
    for ((slot, si), (pi, ri)) in out.iter_mut().zip(s).zip(p.iter().zip(r)) {
        if ri != 0.0 {
            *slot = 0.5 + (pi - si) / (2.0 * ri);
        }
    }
    pose(out)
}

/// Unit-space center at depth `d` must sit at an odd multiple of `3^{-d}/2`.
pub(crate) fn on_lattice(center: f64, depth: u32) -> bool {
    let scale = 3f64.powi(depth as i32);
    let scaled = center * 2.0 * scale;
    let nearest = scaled.round();
    let n = nearest as i64;
    (scaled - nearest).abs() < 1e-9 && n.unsigned_abs() % 2 == 1
}

/// `size = sqrt(Σ 3^{-2 d_i})` with terms summed in sorted order so the
/// reference is permutation-invariant.
pub(crate) fn canonical_size(depths: [u32; 6]) -> f64 {
    let mut terms: [f64; 6] = depths.map(|d| {
        let v = 3f64.powi(d as i32);
        1.0 / (v * v)
    });
    terms.sort_by(|a, b| a.total_cmp(b));
    terms.iter().sum::<f64>().sqrt()
}

pub(crate) fn permute_coords(c: [f64; 6], perm: [usize; 6]) -> [f64; 6] {
    let mut out = [0.0; 6];
    for (slot, src) in out.iter_mut().zip(perm) {
        if let Some(&val) = c.get(src) {
            *slot = val;
        }
    }
    out
}

pub(crate) fn permute_pose(p: Pose, perm: [usize; 6]) -> Pose {
    pose(permute_coords(coords(&p), perm))
}

pub(crate) fn unpermute_pose(p: Pose, perm: [usize; 6]) -> Pose {
    let c = coords(&p);
    let mut out = [0.0; 6];
    for (src, dst) in c.iter().zip(perm) {
        if let Some(slot) = out.get_mut(dst) {
            *slot = *src;
        }
    }
    pose(out)
}
