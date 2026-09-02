//! DIRECT knob vocabulary. Settings declare *what* the search is; the
//! optimizer materializes it. The two representation-mode enums live in
//! `crate::space` (which must not import anything from `direct::`) and are
//! re-exported here for settings-side callers.

use crate::direct::poh::{POHPoint, convex_hull, pareto_front};
pub use crate::space::{RotationRepresentation, TranslationRepresentation};

/// Which size-column representatives DIRECT trisects next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PohStrategy {
    #[default]
    ConvexHull,
    Pareto,
}

impl PohStrategy {
    /// Apply the configured selection strategy to size-ascending candidates.
    pub fn select(self, candidates: &[POHPoint]) -> Vec<POHPoint> {
        match self {
            PohStrategy::ConvexHull => convex_hull(candidates),
            PohStrategy::Pareto => pareto_front(candidates),
        }
    }
}

/// Per-axis minimum physical box width; `None` leaves an axis unbounded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinBoxSize {
    pub values: [Option<f64>; 6],
}

impl MinBoxSize {
    pub fn uniform(size: f64) -> Self {
        Self {
            values: [Some(size); 6],
        }
    }
}

impl Default for MinBoxSize {
    fn default() -> Self {
        Self {
            values: [Some(0.1); 6],
        }
    }
}

/// Basin (BOBYQA) tuning, carried by `Refinement::Bobyqa` so a disabled
/// refinement cannot orphan knobs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BobyqaSettings {
    pub max_evals: u64,
    pub rho_beg: f64,
    pub rho_end: f64,
    pub npt: usize,
}

// A *derived* Default would zero `rho_beg`/`rho_end` (tripping basin's
// driver.rs rho-ordering assert on the first run) and `max_evals` (a
// silently no-op refinement). These constants are the production values,
// moved here from the FFI-side hardcodes.
impl Default for BobyqaSettings {
    fn default() -> Self {
        Self {
            max_evals: 500,
            rho_beg: 0.5,
            rho_end: 1e-3,
            npt: 28,
        }
    }
}

/// Optional post-DIRECT local refinement.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Refinement {
    #[default]
    None,
    Bobyqa(BobyqaSettings),
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DirectSettings {
    pub poh_strategy: PohStrategy,
    pub min_box_size: MinBoxSize,
    pub rotation: RotationRepresentation,
    pub translation: TranslationRepresentation,
    pub refinement: Refinement,
}

impl DirectSettings {
    /// The production settings formerly hardcoded in `bridge::new_rust_opt`.
    pub fn production(use_bobyqa: bool) -> Self {
        Self {
            poh_strategy: PohStrategy::Pareto,
            min_box_size: MinBoxSize::uniform(0.5),
            rotation: RotationRepresentation::AxisAngle,
            translation: TranslationRepresentation::CameraCentered,
            refinement: if use_bobyqa {
                Refinement::Bobyqa(BobyqaSettings::default())
            } else {
                Refinement::None
            },
        }
    }
}
