//! Pure domain model for registration pipelines.
//!
//! This crate knows nothing about Qt, QML, or any UI. Everything here is
//! plain Rust and unit-testable without a display server.

// -------------------------------------------------------------------------
// Cost functions
// -------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CostFunction {
    #[default]
    DirectDilation,
    DirectDilationConstrainZ,
    DirectDilationOldPoleConstraint,
    DirectDilationNewPoleConstraint,
    DirectDilationOldT1,
    DirectDilationMahfouzVariant,
    SymmetryTrap,
}

impl CostFunction {
    /// Every variant, in the order a UI should offer them.
    ///
    /// Keep in sync with `ordinal` below — which is an exhaustive match, so
    /// adding a variant is a compile error there and the test at the bottom
    /// of this file catches any drift between the two.
    pub const ALL: [CostFunction; 7] = [
        CostFunction::DirectDilation,
        CostFunction::DirectDilationConstrainZ,
        CostFunction::DirectDilationOldPoleConstraint,
        CostFunction::DirectDilationNewPoleConstraint,
        CostFunction::DirectDilationOldT1,
        CostFunction::DirectDilationMahfouzVariant,
        CostFunction::SymmetryTrap,
    ];

    /// Human-readable label. Presentation-ish, but keeping it here means the
    /// match is exhaustive and a new variant cannot be silently unnamed.
    pub fn label(self) -> &'static str {
        match self {
            CostFunction::DirectDilation => "Direct Dilation",
            CostFunction::DirectDilationConstrainZ => "Direct Dilation - Constrain Z",
            CostFunction::DirectDilationOldPoleConstraint => {
                "Direct Dilation - Old Pole Constraint"
            }
            CostFunction::DirectDilationNewPoleConstraint => {
                "Direct Dilation - New Pole Constraint"
            }
            CostFunction::DirectDilationOldT1 => "Direct Dilation - Old T1",
            CostFunction::DirectDilationMahfouzVariant => "Direct Dilation - Mahfouz Variant",
            CostFunction::SymmetryTrap => "Symmetry Trap",
        }
    }

    /// Position in `ALL`. Exhaustive match: a new variant breaks the build here.
    pub fn ordinal(self) -> usize {
        match self {
            CostFunction::DirectDilation => 0,
            CostFunction::DirectDilationConstrainZ => 1,
            CostFunction::DirectDilationOldPoleConstraint => 2,
            CostFunction::DirectDilationNewPoleConstraint => 3,
            CostFunction::DirectDilationOldT1 => 4,
            CostFunction::DirectDilationMahfouzVariant => 5,
            CostFunction::SymmetryTrap => 6,
        }
    }

    pub fn from_ordinal(ordinal: usize) -> Option<CostFunction> {
        CostFunction::ALL.get(ordinal).copied()
    }
}

// -------------------------------------------------------------------------
// Stages and pipelines
// -------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageDef {
    pub name: String,
    pub cost_function: CostFunction,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegistrationDef {
    pub stages: Vec<StageDef>,
}

impl RegistrationDef {
    /// A pipeline with `count` default stages, named "Stage 1".."Stage N".
    pub fn with_default_stages(count: usize) -> Self {
        let mut def = RegistrationDef::default();
        for _ in 0..count {
            def.push_default_stage();
        }
        def
    }

    pub fn len(&self) -> usize {
        self.stages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }

    pub fn stage(&self, index: usize) -> Option<&StageDef> {
        self.stages.get(index)
    }

    /// Append a stage with a generated, non-colliding name.
    /// Returns the index of the new stage.
    pub fn push_default_stage(&mut self) -> usize {
        let number = self.next_stage_number();

        self.stages.push(StageDef {
            name: format!("Stage {number}"),
            cost_function: CostFunction::default(),
        });

        self.stages.len() - 1
    }

    pub fn remove_stage(&mut self, index: usize) -> Option<StageDef> {
        if index >= self.stages.len() {
            return None;
        }
        Some(self.stages.remove(index))
    }

    /// Swap adjacent stages. Returns false if the move is out of range.
    pub fn swap_stages(&mut self, a: usize, b: usize) -> bool {
        if a >= self.stages.len() || b >= self.stages.len() || a == b {
            return false;
        }
        self.stages.swap(a, b);
        true
    }

    pub fn set_cost_function(&mut self, index: usize, cost_function: CostFunction) -> bool {
        let Some(stage) = self.stages.get_mut(index) else {
            return false;
        };

        if stage.cost_function == cost_function {
            return false;
        }

        stage.cost_function = cost_function;
        true
    }

    /// One past the highest "Stage N" already in use, so names never collide
    /// after a removal. Stateless on purpose.
    fn next_stage_number(&self) -> usize {
        self.stages
            .iter()
            .filter_map(|stage| stage.name.strip_prefix("Stage "))
            .filter_map(|suffix| suffix.parse::<usize>().ok())
            .max()
            .map_or(1, |highest| highest + 1)
    }
}

// -------------------------------------------------------------------------
// Tests — the whole point of keeping this crate Qt-free
// -------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_and_ordinal_agree() {
        for (index, cost_function) in CostFunction::ALL.iter().enumerate() {
            assert_eq!(
                cost_function.ordinal(),
                index,
                "{cost_function:?} misplaced in ALL"
            );
            assert_eq!(CostFunction::from_ordinal(index), Some(*cost_function));
        }
    }

    #[test]
    fn generated_names_do_not_collide_after_removal() {
        let mut def = RegistrationDef::with_default_stages(3);
        def.remove_stage(0);
        def.push_default_stage();

        let names: Vec<&str> = def.stages.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Stage 2", "Stage 3", "Stage 4"]);
    }

    #[test]
    fn set_cost_function_reports_whether_it_changed() {
        let mut def = RegistrationDef::with_default_stages(1);
        assert!(!def.set_cost_function(0, CostFunction::DirectDilation));
        assert!(def.set_cost_function(0, CostFunction::SymmetryTrap));
        assert!(!def.set_cost_function(9, CostFunction::SymmetryTrap));
    }

    #[test]
    fn swap_rejects_out_of_range() {
        let mut def = RegistrationDef::with_default_stages(2);
        assert!(def.swap_stages(0, 1));
        assert!(!def.swap_stages(0, 5));
        assert!(!def.swap_stages(1, 1));
    }
}
