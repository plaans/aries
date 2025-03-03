use std::collections::HashMap;

use crate::domain::IntDomain;
use crate::parameter::Parameter;
use crate::solve::Goal;
use crate::solve::Objective;
use crate::solve::SolveItem;
use crate::traits::Identifiable;
use crate::types::Id;
use crate::variable::BoolVariable;
use crate::variable::IntVariable;
use crate::variable::SharedBoolVariable;
use crate::variable::SharedIntVariable;
use crate::variable::Variable;

pub struct Model {
    parameters: HashMap<Id, Parameter>,
    variables: HashMap<Id, Variable>,
    solve_item: SolveItem,
}

impl Model {

    /// Create a new empty satisfaction model.
    pub fn new() -> Self {
        let parameters = HashMap::new();
        let variables = HashMap::new();
        let solve_item = SolveItem::Satisfy;
        Model { parameters, variables, solve_item }
    }

    // ------------------------------------------------------------

    /// Return the solve item.
    pub fn solve_item(&self) -> &SolveItem {
        &self.solve_item
    }

    /// Return an iterator over the variables.
    pub fn variables(&self) -> impl Iterator<Item = &Variable> {
        self.variables.values()
    }

    /// Return an iterator over the parameters.
    pub fn parameters(&self) -> impl Iterator<Item = &Parameter> {
        self.parameters.values()
    }

    /// Return the number of variables.
    pub fn nb_variables(&self) -> usize {
        self.variables.len()
    }

    // ------------------------------------------------------------

    /// Add the given variable to the model.
    /// 
    /// Returns whether the variable was newly inserted. That is:
    ///  - `true` if the variable is new
    ///  - `false` if it was already known
    fn add_variable(&mut self, variable: Variable) -> bool {
        let known = self.variables.contains_key(variable.id());
        if known {
            debug_assert_eq!(self.variables.get(variable.id()).unwrap(), &variable);
        } else {
            self.variables.insert(variable.id().clone(), variable);
        }
        !known
    }

    /// Add the given parameter to the model.
    /// 
    /// Returns whether the parameter was newly inserted. That is:
    ///  - `true` if the parameter is new
    ///  - `false` if it was already known
    fn add_parameter(&mut self, parameter: Parameter) -> bool {
        let known = self.parameters.contains_key(parameter.id());
        if known {
            debug_assert_eq!(self.parameters.get(parameter.id()).unwrap(), &parameter);
        } else {
            self.parameters.insert(parameter.id().clone(), parameter);
        }
        !known
    }

    // ------------------------------------------------------------

    /// Transform the model into an optimization problem on the given variable.
    /// Add the variable to the model if needed.
    /// 
    /// Returns whether the value was newly inserted. That is:
    ///  - `true` if the variable is new
    ///  - `false` if it was already known
    pub fn optimize(&mut self, goal: Goal, variable: impl Into<Variable>) -> bool {
        let variable = variable.into();
        let objective = Objective::new(goal, variable.clone());
        self.solve_item = SolveItem::Optimize(objective);
        self.add_variable(variable)
    }

    // ------------------------------------------------------------

    /// Create a new integer variable and add it to the model.
    /// 
    /// The variable is returned.
    pub fn new_int_variable(&mut self, id: Id, domain: IntDomain) -> SharedIntVariable {
        let variable: SharedIntVariable = IntVariable::new(id, domain).into();
        self.add_variable(variable.clone().into());
        variable
    }

    /// Create a new boolean variable and add it to the model.
    /// 
    /// The variable is returned.
    pub fn new_bool_variable(&mut self, id: Id) -> SharedBoolVariable {
        let variable: SharedBoolVariable = BoolVariable::new(id).into();
        self.add_variable(variable.clone().into());
        variable
    }
}

#[cfg(test)]
mod tests {

    use crate::domain::IntRange;

    use super::*;

    /// Return a simple satisfaction model and its variables.
    /// 
    /// It has four variables:
    ///  - x int in \[2,5\]
    ///  - y int in \[3,9\]
    ///  - a bool
    ///  - b bool
    fn simple_model() -> (SharedIntVariable, SharedIntVariable, SharedBoolVariable, SharedBoolVariable, Model) {
        let domain_x: IntDomain = IntRange::new(2,5).unwrap().into();
        let domain_y: IntDomain = IntRange::new(3,9).unwrap().into();

        let mut model = Model::new();

        let x = model.new_int_variable("x".to_string(), domain_x);
        let y = model.new_int_variable("y".to_string(), domain_y);
        
        let a = model.new_bool_variable("a".to_string());
        let b = model.new_bool_variable("b".to_string());

        (x, y, a, b, model)
    }

    #[test]
    fn basic_sat_model() {
        let (x, y, a, b, model) = simple_model();

        let variables: Vec<&Variable> = model.variables().collect();

        assert!(variables.contains(&&x.into()));
        assert!(variables.contains(&&y.into()));
        assert!(variables.contains(&&a.into()));
        assert!(variables.contains(&&b.into()));

        assert_eq!(variables.len(), 4);
        assert_eq!(model.nb_variables(), 4);

        assert!(model.solve_item().is_satisfy());
    }

    #[test]
    fn basic_min_model() {
        let (x, y, a, b, mut model) = simple_model();

        let domain_z: IntDomain = IntRange::new(3,9).unwrap().into();
        let z = model.new_int_variable("z".to_string(), domain_z);

        model.optimize(Goal::Maximize, z.clone());

        let variables: Vec<&Variable> = model.variables().collect();

        assert!(variables.contains(&&x.into()));
        assert!(variables.contains(&&y.into()));
        assert!(variables.contains(&&z.into()));
        assert!(variables.contains(&&a.into()));
        assert!(variables.contains(&&b.into()));

        assert_eq!(variables.len(), 5);
        assert_eq!(model.nb_variables(), 5);

        assert!(model.solve_item().is_optimize());
    }
}