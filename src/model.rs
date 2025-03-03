use std::collections::HashMap;

use crate::domain::IntDomain;
use crate::parameter::BoolParameter;
use crate::parameter::IntParameter;
use crate::parameter::Parameter;
use crate::parameter::SharedBoolParameter;
use crate::parameter::SharedIntParameter;
use crate::solve::Goal;
use crate::solve::Objective;
use crate::solve::SolveItem;
use crate::traits::Identifiable;
use crate::types::Id;
use crate::types::Int;
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

    /// Return the number of parameters.
    pub fn nb_parameters(&self) -> usize {
        self.parameters.len()
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

    // ------------------------------------------------------------

    /// Create a new integer parameter and add it to the model.
    /// 
    /// The parameter is returned.
    pub fn new_int_parameter(&mut self, id: Id, value: Int) -> SharedIntParameter {
        let parameter: SharedIntParameter = IntParameter::new(id, value).into();
        self.add_parameter(parameter.clone().into());
        parameter
    }

    /// Create a new boolean parameter and add it to the model.
    /// 
    /// The parameter is returned.
    pub fn new_bool_parameter(&mut self, id: Id, value: bool) -> SharedBoolParameter {
        let parameter: SharedBoolParameter = BoolParameter::new(id, value).into();
        self.add_parameter(parameter.clone().into());
        parameter
    }

}

#[cfg(test)]
mod tests {

    use crate::domain::IntRange;

    use super::*;

    /// Return a simple satisfaction model, its variables and parameters.
    /// 
    /// It has two variables and two parameters:
    ///  - x int in \[2,5\]
    ///  - y bool
    ///  - t = 4
    ///  - s = true
    fn simple_model() -> (SharedIntVariable, SharedBoolVariable, SharedIntParameter, SharedBoolParameter, Model) {
        let domain_x: IntDomain = IntRange::new(2,5).unwrap().into();

        let mut model = Model::new();

        let x = model.new_int_variable("x".to_string(), domain_x);
        let y = model.new_bool_variable("y".to_string());
        let t = model.new_int_parameter("t".to_string(), 4);
        let s = model.new_bool_parameter("s".to_string(), true);
        
        (x, y, t, s, model)
    }

    #[test]
    fn basic_sat_model() {
        let (x, y, t, s, model) = simple_model();

        let variables: Vec<&Variable> = model.variables().collect();
        let parameters: Vec<&Parameter> = model.parameters().collect();

        assert!(variables.contains(&&x.into()));
        assert!(variables.contains(&&y.into()));

        assert!(parameters.contains(&&t.into()));
        assert!(parameters.contains(&&s.into()));

        assert_eq!(variables.len(), 2);
        assert_eq!(parameters.len(), 2);

        assert_eq!(model.nb_variables(), 2);
        assert_eq!(model.nb_parameters(), 2);

        assert!(model.solve_item().is_satisfy());
    }

    #[test]
    fn basic_min_model() {
        let (x, y, t, s, mut model) = simple_model();

        let domain_z = IntRange::new(3,9).unwrap().into();
        let z = model.new_int_variable("z".to_string(), domain_z);

        model.optimize(Goal::Maximize, z.clone());

        let variables: Vec<&Variable> = model.variables().collect();
        let parameters: Vec<&Parameter> = model.parameters().collect();

        assert!(variables.contains(&&x.into()));
        assert!(variables.contains(&&y.into()));
        assert!(variables.contains(&&z.into()));

        assert!(parameters.contains(&&t.into()));
        assert!(parameters.contains(&&s.into()));

        assert_eq!(variables.len(), 3);
        assert_eq!(parameters.len(), 2);

        assert_eq!(model.nb_variables(), 3);
        assert_eq!(model.nb_parameters(), 2);

        assert!(model.solve_item().is_optimize());
    }
}