use std::collections::HashMap;

use anyhow::anyhow;
use anyhow::ensure;
use anyhow::Result;

use crate::constraint::Constraint;
use crate::domain::IntDomain;
use crate::parameter::BoolParameter;
use crate::parameter::IntParameter;
use crate::parameter::Parameter;
use crate::parameter::SharedBoolParameter;
use crate::parameter::SharedIntParameter;
use crate::parvar::ParVar;
use crate::solve::Goal;
use crate::solve::Objective;
use crate::solve::SolveItem;
use crate::traits::Identifiable;
use crate::types::Id;
use crate::types::Int;
use crate::variable::BasicVariable;
use crate::variable::BoolVariable;
use crate::variable::IntVariable;
use crate::variable::SharedBoolVariable;
use crate::variable::SharedIntVariable;
use crate::variable::Variable;

pub struct Model {
    parameters: HashMap<Id, Parameter>,
    variables: HashMap<Id, Variable>,
    constraints: Vec<Box<dyn Constraint>>,
    solve_item: SolveItem,
}

impl Model {

    /// Create a new empty satisfaction model.
    pub fn new() -> Self {
        let parameters = HashMap::new();
        let variables = HashMap::new();
        let constraints = Vec::new();
        let solve_item = SolveItem::Satisfy;
        Model { parameters, variables, constraints, solve_item }
    }

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

    /// Return an iterator over the constraints.
    pub fn constraints(&self) -> impl Iterator<Item = &Box<dyn Constraint>> {
        self.constraints.iter()
    }

    /// Return the number of variables.
    pub fn nb_variables(&self) -> usize {
        self.variables.len()
    }

    /// Return the number of parameters.
    pub fn nb_parameters(&self) -> usize {
        self.parameters.len()
    }

    /// Return the number of constraints.
    pub fn nb_constraints(&self) -> usize {
        self.constraints.len()
    }

    /// Get the variable with the given id.
    /// 
    /// Fail if no variable has the given id.
    pub fn get_variable(&self, id: &Id) -> Result<&Variable> {
        self.variables.get(id)
            .ok_or_else(|| anyhow!("variable '{}' is not defined", id))
    }

    /// Get the parameter with the given id.
    /// 
    /// Fail if no parameter has the given id.
    pub fn get_parameter(&self, id: &Id) -> Result<&Parameter> {
        self.parameters.get(id)
            .ok_or_else(|| anyhow!("parameter '{}' is not defined", id))
    }

    /// Return `true` if a variable has the given id.
    pub fn contains_variable_id(&self, id: &Id) -> bool {
        self.get_variable(id).is_ok()
    }

    /// Return `true` if a parameter has the given id.
    pub fn contains_parameter_id(&self, id: &Id) -> bool {
        self.get_parameter(id).is_ok()
    }

    // Return `true` a variable or parameter has the given id.
    pub fn contains_id(&self, id: &Id) -> bool {
        self.contains_variable_id(id) || self.contains_parameter_id(id)
    }

    /// Add the given variable to the model.
    /// 
    /// Fail if the variable id is already taken.
    fn add_variable(&mut self, variable: impl Into<Variable>) -> Result<()> {
        let variable = variable.into();
        let known = self.contains_id(variable.id());
        ensure!(!known, "variable '{}' is already defined", variable.id());
        self.variables.insert(variable.id().clone(), variable);
        Ok(())
    }

    /// Add the given parameter to the model.
    /// 
    /// Fail if the parameter id is already taken.
    fn add_parameter(&mut self, parameter: impl Into<Parameter>) -> Result<()> {
        let parameter = parameter.into();
        let known = self.contains_id(parameter.id());
        ensure!(!known, "parameter '{}' is already defined", parameter.id());
        self.parameters.insert(parameter.id().clone(), parameter);
        Ok(())
    }

    /// Add the given parvar to the model.
    /// 
    /// Fail if its id is already defined.
    fn add_parvar(&mut self, parvar: impl Into<ParVar>) -> Result<()> {
        let parvar = parvar.into();
        match parvar {
            ParVar::Par(p) => {
                if !self.parameters.contains_key(p.id()) {
                    self.add_parameter(p)?
                }
            },
            ParVar::Var(v) => {
                if !self.variables.contains_key(v.id()) {
                    self.add_variable(v)?
                }
            },
        }
        Ok(())
    }

    /// Transform the model into an optimization problem on the given variable.
    /// 
    /// Fail if the variable id is unkown.
    pub fn optimize(&mut self, goal: Goal, variable: impl Into<BasicVariable>) -> Result<()> {
        let variable = variable.into();
        ensure!(self.contains_variable_id(variable.id()), "variable '{}' is not defined", variable.id());
        let objective = Objective::new(goal, variable.clone());
        self.solve_item = SolveItem::Optimize(objective);
        Ok(())
    }

    /// Transform the model into an minimization problem on the given variable.
    /// 
    /// Fail if the variable id is unkown.
    pub fn minimize(&mut self, variable: impl Into<BasicVariable>) -> Result<()> {
        self.optimize(Goal::Minimize, variable)
    }

    /// Transform the model into an maximization problem on the given variable.
    /// 
    /// Fail if the variable id is unkown.
    pub fn maximize(&mut self, variable: impl Into<BasicVariable>) -> Result<()> {
        self.optimize(Goal::Maximize, variable)
    }

    /// Create a new integer variable and add it to the model.
    /// 
    /// Fail if the variable id is already taken.
    pub fn new_int_variable(&mut self, id: Id, domain: IntDomain) -> Result<SharedIntVariable> {
        let variable: SharedIntVariable = IntVariable::new(id, domain).into();
        self.add_variable(variable.clone())?;
        Ok(variable)
    }

    /// Create a new boolean variable and add it to the model.
    /// 
    /// Fail if the variable id is already taken.
    pub fn new_bool_variable(&mut self, id: Id) -> Result<SharedBoolVariable> {
        let variable: SharedBoolVariable = BoolVariable::new(id).into();
        self.add_variable(variable.clone())?;
        Ok(variable)
    }

    /// Create a new integer parameter and add it to the model.
    /// 
    /// Fail if the parameter id is already taken.
    pub fn new_int_parameter(&mut self, id: Id, value: Int) -> Result<SharedIntParameter> {
        let parameter: SharedIntParameter = IntParameter::new(id, value).into();
        self.add_parameter(parameter.clone())?;
        Ok(parameter)
    }

    /// Create a new boolean parameter and add it to the model.
    /// 
    /// Fail if the parameter id is already taken.
    pub fn new_bool_parameter(&mut self, id: Id, value: bool) -> Result<SharedBoolParameter> {
        let parameter: SharedBoolParameter = BoolParameter::new(id, value).into();
        self.add_parameter(parameter.clone())?;
        Ok(parameter)
    }

    /// Add the given constraint to the model.
    /// If needed, its arguments are added to the model.
    /// 
    /// Fail if an argument A of the constraint fulfills the two following conditions:
    ///  - A is not in the model
    ///  - A cannot be added to the model
    pub fn add_constraint(&mut self, constraint: Box<dyn Constraint>) -> Result<()> {
        for arg in constraint.args() {
            self.add_parvar(arg)?;
        }
        self.constraints.push(constraint);
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use crate::constraint::builtins::IntEq;
    use crate::domain::IntRange;

    use super::*;

    /// Return a simple satisfaction model, its variables and parameters.
    /// 
    /// It has two variables, two parameters and one constraint:
    ///  - x int in \[2,5\]
    ///  - y int in \[-3,3\]
    ///  - t = 4
    ///  - s = true
    ///  - c: y = x
    fn simple_model() -> (SharedIntVariable, SharedIntVariable, SharedIntParameter, SharedBoolParameter, Model) {
        let domain_x: IntDomain = IntRange::new(2,5).unwrap().into();
        let domain_y: IntDomain = IntRange::new(-3,3).unwrap().into();

        let mut model = Model::new();

        let x = model.new_int_variable("x".to_string(), domain_x).unwrap();
        let y = model.new_int_variable("y".to_string(), domain_y).unwrap();
        let t = model.new_int_parameter("t".to_string(), 4).unwrap();
        let s = model.new_bool_parameter("s".to_string(), true).unwrap();

        let c = IntEq::new(x.clone(), y.clone());
        model.add_constraint(Box::new(c)).unwrap();

        (x, y, t, s, model)
    }

    #[test]
    fn basic_sat_model() {
        let (x, y, t, s, model) = simple_model();

        let variables: Vec<&Variable> = model.variables().collect();
        let parameters: Vec<&Parameter> = model.parameters().collect();
        let constraints: Vec<&Box<dyn Constraint>> = model.constraints().collect();

        assert!(variables.contains(&&x.into()));
        assert!(variables.contains(&&y.into()));

        assert!(parameters.contains(&&t.into()));
        assert!(parameters.contains(&&s.into()));

        assert_eq!(variables.len(), 2);
        assert_eq!(parameters.len(), 2);
        assert_eq!(constraints.len(), 1);

        assert_eq!(model.nb_variables(), 2);
        assert_eq!(model.nb_parameters(), 2);
        assert_eq!(model.nb_constraints(), 1);

        assert!(model.solve_item().is_satisfy());
    }

    #[test]
    fn basic_min_model() {
        let (x, y, t, s, mut model) = simple_model();

        let domain_z = IntRange::new(3,9).unwrap().into();
        let z = model.new_int_variable("z".to_string(), domain_z).unwrap();

        model.optimize(Goal::Maximize, z.clone()).unwrap();

        let variables: Vec<&Variable> = model.variables().collect();
        let parameters: Vec<&Parameter> = model.parameters().collect();
        let constraints: Vec<&Box<dyn Constraint>> = model.constraints().collect();

        assert!(variables.contains(&&x.into()));
        assert!(variables.contains(&&y.into()));
        assert!(variables.contains(&&z.into()));

        assert!(parameters.contains(&&t.into()));
        assert!(parameters.contains(&&s.into()));

        assert_eq!(variables.len(), 3);
        assert_eq!(parameters.len(), 2);
        assert_eq!(constraints.len(), 1);

        assert_eq!(model.nb_variables(), 3);
        assert_eq!(model.nb_parameters(), 2);
        assert_eq!(model.nb_constraints(), 1);

        assert!(model.solve_item().is_optimize());
    }

    #[test]
    fn optimize() {
        let (x, y, _, _, _) = simple_model();

        let mut model = Model::new();
        model.add_variable(x.clone()).unwrap();

        assert!(model.optimize(Goal::Minimize, x).is_ok());
        assert!(model.optimize(Goal::Minimize, y).is_err());
    }

    #[test]
    fn get_vars_and_pars() {
        let (x, y, t, s, model) = simple_model();

        assert_eq!(*model.get_variable(x.id()).unwrap(), Variable::from(x.clone()));
        assert_eq!(*model.get_variable(y.id()).unwrap(), Variable::from(y.clone()));
        assert!(model.get_variable(t.id()).is_err());
        assert!(model.get_variable(s.id()).is_err());
        
        assert_eq!(*model.get_parameter(t.id()).unwrap(), Parameter::from(t.clone()));
        assert_eq!(*model.get_parameter(s.id()).unwrap(), Parameter::from(s.clone()));
        assert!(model.get_parameter(x.id()).is_err());
        assert!(model.get_parameter(y.id()).is_err());
    }

    #[test]
    fn contains() {
        let (x, y, t, s, model) = simple_model();
        let unknown = "unknown".to_string();

        assert!(!model.contains_parameter_id(x.id()));
        assert!(!model.contains_parameter_id(y.id()));
        assert!(model.contains_parameter_id(t.id()));
        assert!(model.contains_parameter_id(s.id()));
        assert!(!model.contains_parameter_id(&unknown));

        assert!(model.contains_variable_id(x.id()));
        assert!(model.contains_variable_id(y.id()));
        assert!(!model.contains_variable_id(t.id()));
        assert!(!model.contains_variable_id(s.id()));
        assert!(!model.contains_variable_id(&unknown));

        assert!(model.contains_id(x.id()));
        assert!(model.contains_id(y.id()));
        assert!(model.contains_id(t.id()));
        assert!(model.contains_id(s.id()));
        assert!(!model.contains_id(&unknown));
    }

    #[test]
    fn same_id() {
        let mut model = Model::new();

        let _x = model.new_bool_variable("x".to_string()).unwrap();
        assert!(model.new_bool_variable("x".to_string()).is_err());
        assert!(model.new_int_parameter("x".to_string(), 5).is_err());
    }
}