//! Potentially-optimal-hyperbox (POH) selection: which size-column
//! representatives DIRECT trisects next.
//!
//! Candidates arrive size-ascending (BTreeMap iteration over
//! [`crate::direct::tree::DirectTree`]), one per size column — the cheapest
//! still-refinable box of that column. [`PohStrategy::select`] then
//! applies the configured strategy: the Jones convex lower hull, or the
//! Pareto (monotone cost) front.


#[cfg(test)]
mod test;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct POHPoint {
    pub size: f64,
    pub cost: f64,
}


pub fn pareto_front(candidates: &[POHPoint]) -> Vec<POHPoint> {
    let mut pts: Vec<POHPoint> = candidates.to_vec();
    // largest size first; cost ascending breaks ties so equal-size points
    // (shouldn't occur post-dedup, but be defensive) prefer the cheaper one
    pts.sort_by(|a, b| b.size.total_cmp(&a.size).then(a.cost.total_cmp(&b.cost)));

    let mut front = Vec::with_capacity(pts.len());
    let mut best_cost = f64::INFINITY;
    for p in pts {
        if p.cost.is_finite() && p.cost < best_cost {
            front.push(p);
            best_cost = p.cost;
        }
    }
    front.reverse(); // back to ascending size, matching convex_hull's convention
    return front;
}

pub fn convex_hull(candidates: &[POHPoint]) -> Vec<POHPoint> {
    let mut hull: Vec<POHPoint> = Vec::new();

    if candidates.len() < 2 {
        hull.extend(candidates);
        return hull;
    }
    for p in candidates.iter() {
        while (hull.len() >= 2)
            && (cross(
                (hull[hull.len() - 2].size, hull[hull.len() - 2].cost),
                (hull[hull.len() - 1].size, hull[hull.len() - 1].cost),
                (p.size, p.cost),
            ) < 0.0)
        {
            hull.pop();
        }
        hull.push(*p);
    }
    let cut = hull
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.cost.total_cmp(&b.cost).then(b.size.total_cmp(&a.size)))
        .map(|(i, _)| i)
        .unwrap_or(0);
    return hull.split_off(cut);
}

fn cross(o: (f64, f64), p1: (f64, f64), p2: (f64, f64)) -> f64 {
    return (p1.0 - o.0) * (p2.1 - o.1) - (p1.1 - o.1) * (p2.0 - o.0);
}
