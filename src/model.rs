use std::collections::HashSet;

use crate::parameter::Parameter;
use crate::solve::Goal;
use crate::solve::Objective;
use crate::solve::SolveItem;
use crate::variable::SharedVariable;

pub struct Model {
    parameters: HashSet<Parameter>,
    variables: HashSet<SharedVariable>,
    solve_item: SolveItem,
}

impl Model {

    /// Create a new empty satisfaction model.
    pub fn new() -> Self {
        let parameters = HashSet::new();
        let variables = HashSet::new();
        let solve_item = SolveItem::Satisfy;
        Model { parameters, variables, solve_item }
    }

    /// Transform the model into an optimization problem on the given variable.
    /// Add the variable to the model if needed.
    /// 
    /// Returns whether the value was newly inserted. That is:
    ///  - `true` if the variable is new
    ///  - `false` if it was already known
    pub fn optimize(&mut self, goal: Goal, variable: SharedVariable) -> bool {
        let objective = Objective::new(goal, variable.clone());
        self.solve_item = SolveItem::Optimize(objective);
        self.variables.insert(variable)
    }

    /// Return the solve item.
    pub fn solve_item(&self) -> &SolveItem {
        &self.solve_item
    }

    /// Return an iterator over the variables.
    pub fn variables(&self) -> impl Iterator<Item = &SharedVariable> {
        self.variables.iter()
    }

    /// Return the number of variables.
    pub fn nb_variables(&self) -> usize {
        self.variables.len()
    }

    /// Add the given parameter to the model.
    pub fn add_parameter(&mut self, parameter: Parameter) {
        debug_assert!(
            self.parameters.insert(parameter),
            "the parameter should not be already in the model",
        )
    }

    /// Add the given variable to the model.
    /// 
    /// Returns whether the value was newly inserted. That is:
    ///  - `true` if the variable is new
    ///  - `false` if it was already known
    pub fn add_variable(&mut self, variable: SharedVariable) -> bool {
        self.variables.insert(variable)
    }
}

#[cfg(test)]
mod tests {

    use crate::domain::IntRange;
    use crate::variable::BoolVariable;
    use crate::variable::IntVariable;

    use super::*;

    /// Return a simple satisfaction model and its variables.
    /// 
    /// It has four variables:
    ///  - x int in \[2,5\]
    ///  - y int in \[3,9\]
    ///  - a bool
    ///  - b bool
    fn simple_model() -> (SharedVariable, SharedVariable, SharedVariable, SharedVariable, Model) {
        let x: SharedVariable = IntVariable::new(
            IntRange::new(2,5).unwrap().into(),
        ).into();

        let y: SharedVariable = IntVariable::new(
            IntRange::new(3,9).unwrap().into(),
        ).into();

        let a: SharedVariable = BoolVariable::new().into();
        let b: SharedVariable = BoolVariable::new().into();
        
        let mut model = Model::new();

        model.add_variable(x.clone());
        model.add_variable(y.clone());
        model.add_variable(a.clone());
        model.add_variable(b.clone());

        (x, y, a, b, model)
    }

    #[test]
    fn basic_sat_model() {
        let (x, y, a, b, model) = simple_model();

        let variables: Vec<&SharedVariable> = model.variables().collect();

        assert!(variables.contains(&&x));
        assert!(variables.contains(&&y));
        assert!(variables.contains(&&a));
        assert!(variables.contains(&&b));

        assert_eq!(variables.len(), 4);
        assert_eq!(model.nb_variables(), 4);

        assert!(model.solve_item().is_satisfy());
    }

    #[test]
    fn basic_min_model() {
        let (x, y, a, b, mut model) = simple_model();

        let z: SharedVariable = IntVariable::new(
            IntRange::new(-5,7).unwrap().into(),
        ).into();

        // z should be added here
        model.optimize(Goal::Maximize, z.clone());

        let variables: Vec<&SharedVariable> = model.variables().collect();

        assert!(variables.contains(&&x));
        assert!(variables.contains(&&y));
        assert!(variables.contains(&&z));
        assert!(variables.contains(&&a));
        assert!(variables.contains(&&b));

        assert_eq!(variables.len(), 5);
        assert_eq!(model.nb_variables(), 5);

        assert!(model.solve_item().is_optimize());
    }
}