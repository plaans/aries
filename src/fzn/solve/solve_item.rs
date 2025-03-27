use crate::fzn::solve::Objective;
use crate::fzn::var::BasicVar;

#[derive(PartialEq, Debug)]
pub enum SolveItem {
    Satisfy,
    Optimize(Objective),
}

impl SolveItem {
    /// Return the objective variable if available.
    pub fn variable(&self) -> Option<&BasicVar> {
        match self {
            SolveItem::Satisfy => None,
            SolveItem::Optimize(objective) => Some(objective.variable()),
        }
    }

    /// Returns `true` if the solve item is a `Satisfy` value.
    pub fn is_satisfy(&self) -> bool {
        matches!(self, SolveItem::Satisfy)
    }

    /// Returns `true` if the solve item is an `Optimize` value.
    pub fn is_optimize(&self) -> bool {
        matches!(self, SolveItem::Optimize(_))
    }
}

impl Default for SolveItem {
    fn default() -> Self {
        Self::Satisfy
    }
}

#[cfg(test)]
mod tests {
    use crate::fzn::domain::BoolDomain;
    use crate::fzn::solve::Goal;
    use crate::fzn::var::VarBool;

    use super::*;

    #[test]
    fn objective_variable() {
        let x: BasicVar =
            VarBool::new(BoolDomain::Both, "x".to_string(), false).into();
        let objective = Objective::new(Goal::Maximize, x.clone());

        let sat_item = SolveItem::Satisfy;
        let opt_item = SolveItem::Optimize(objective);

        assert_eq!(sat_item.variable(), None);
        assert_eq!(opt_item.variable(), Some(&x));
    }

    #[test]
    fn is_thing() {
        let x: BasicVar =
            VarBool::new(BoolDomain::Both, "x".to_string(), false).into();
        let objective = Objective::new(Goal::Maximize, x.clone());

        let sat_item = SolveItem::Satisfy;
        let opt_item = SolveItem::Optimize(objective);

        assert!(sat_item.is_satisfy());
        assert!(!sat_item.is_optimize());
        assert!(opt_item.is_optimize());
        assert!(!opt_item.is_satisfy());
    }
}
