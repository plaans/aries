use std::collections::HashMap;
use std::rc::Rc;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Result;

use crate::constraint::Constraint;
use crate::domain::BoolDomain;
use crate::domain::IntDomain;
use crate::par::Par;
use crate::par::ParBool;
use crate::par::ParInt;
use crate::solve::Goal;
use crate::solve::Objective;
use crate::solve::SolveItem;
use crate::traits::Flatzinc;
use crate::traits::Name;
use crate::types::Int;
use crate::var::BasicVar;
use crate::var::Var;
use crate::var::VarBool;
use crate::var::VarBoolArray;
use crate::var::VarInt;
use crate::var::VarIntArray;

#[derive(Debug)]
pub struct Model {
    parameters: Vec<Par>,
    variables: Vec<Var>,
    constraints: Vec<Constraint>,
    solve_item: SolveItem,
    name_par: HashMap<String, Par>,
    name_var: HashMap<String, Var>,
}

impl Model {
    /// Create a new empty satisfaction model.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the solve item.
    pub fn solve_item(&self) -> &SolveItem {
        &self.solve_item
    }

    /// Return an iterator over the variables.
    pub fn variables(&self) -> impl Iterator<Item = &Var> {
        self.variables.iter()
    }

    /// Return an iterator over the parameters.
    pub fn parameters(&self) -> impl Iterator<Item = &Par> {
        self.parameters.iter()
    }

    /// Return an iterator over the constraints.
    pub fn constraints(&self) -> impl Iterator<Item = &Constraint> {
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

    /// Get the variable with the given name.
    ///
    /// Fail if no variable has the given name.
    pub fn get_variable(&self, name: &String) -> Result<&Var> {
        self.name_var
            .get(name)
            .ok_or_else(|| anyhow!("variable '{}' is not defined", name))
    }

    /// Get the int variable with the given name.
    ///
    /// Fail if no int variable has the given name.
    pub fn get_var_int(&self, name: &String) -> Result<Rc<VarInt>> {
        self.get_variable(name)?.clone().try_into()
    }

    /// Get the int array variable with the given name.
    ///
    /// Fail if no int array variable has the given name.
    pub fn get_var_int_array(&self, name: &String) -> Result<Rc<VarIntArray>> {
        self.get_variable(name)?.clone().try_into()
    }

    /// Get the bool variable with the given name.
    ///
    /// Fail if no bool variable has the given name.
    pub fn get_var_bool(&self, name: &String) -> Result<Rc<VarBool>> {
        self.get_variable(name)?.clone().try_into()
    }

    /// Get the bool array variable with the given name.
    ///
    /// Fail if no bool array variable has the given name.
    pub fn get_var_bool_array(&self, name: &String) -> Result<Rc<VarBoolArray>> {
        self.get_variable(name)?.clone().try_into()
    }

    /// Get the parameter with the given name.
    ///
    /// Fail if no parameter has the given name.
    pub fn get_par(&self, name: &String) -> Result<&Par> {
        self.name_par
            .get(name)
            .ok_or_else(|| anyhow!("parameter '{}' is not defined", name))
    }

    /// Get the int parameter with the given name.
    ///
    /// Fail if no int parameter has the given name.
    pub fn get_par_int(&self, name: &String) -> Result<Rc<ParInt>> {
        self.get_par(name)?.clone().try_into()
    }

    /// Get the bool parameter with the given name.
    ///
    /// Fail if no bool parameter has the given name.
    pub fn get_par_bool(&self, name: &String) -> Result<Rc<ParBool>> {
        self.get_par(name)?.clone().try_into()
    }

    /// Return `true` if a variable has the given name.
    pub fn contains_var_name(&self, name: &String) -> bool {
        self.get_variable(name).is_ok()
    }

    /// Return `true` if a parameter has the given name.
    pub fn contains_par_name(&self, name: &String) -> bool {
        self.get_par(name).is_ok()
    }

    // Return `true` if a variable or parameter has the given name.
    pub fn contains_name(&self, name: &String) -> bool {
        self.contains_var_name(name) || self.contains_par_name(name)
    }

    /// Add the given variable to the model.
    ///
    /// Fail if the variable name is already taken.
    fn add_var(&mut self, variable: impl Into<Var>) -> Result<()> {
        let variable = variable.into();
        if let Some(name) = variable.name() {
            if self.contains_name(name) {
                bail!("variable '{}' is already defined", name);
            } else {
                self.name_var.insert(name.clone(), variable.clone());
            }
        }
        self.variables.push(variable);
        Ok(())
    }

    /// Add the given parameter to the model.
    ///
    /// Fail if the parameter name is already taken.
    fn add_par(&mut self, parameter: impl Into<Par>) -> Result<()> {
        let parameter = parameter.into();
        if self.contains_name(parameter.name()) {
            bail!("parameter '{}' is already defined", parameter.name());
        } else {
            self.name_par
                .insert(parameter.name().clone(), parameter.clone());
        }
        self.parameters.push(parameter);
        Ok(())
    }

    /// Transform the model into a satisfaction problem.
    pub fn satisfy(&mut self) {
        self.solve_item = SolveItem::Satisfy;
    }

    /// Transform the model into an optimization problem on the given variable.
    ///
    /// Fail if the variable is not in the model.
    pub fn optimize(&mut self, goal: Goal, variable: impl Into<BasicVar>) -> Result<()> {
        let variable = variable.into();
        ensure!(
            self.variables.contains(&variable.clone().into()),
            "variable '{:?}' is not defined",
            variable.name()
        );
        let objective = Objective::new(goal, variable);
        self.solve_item = SolveItem::Optimize(objective);
        Ok(())
    }

    /// Transform the model into an minimization problem on the given variable.
    ///
    /// Fail if the variable is not in the model.
    pub fn minimize(&mut self, variable: impl Into<BasicVar>) -> Result<()> {
        self.optimize(Goal::Minimize, variable)
    }

    /// Transform the model into an maximization problem on the given variable.
    ///
    /// Fail if the variable is not in the model.
    pub fn maximize(&mut self, variable: impl Into<BasicVar>) -> Result<()> {
        self.optimize(Goal::Maximize, variable)
    }

    /// Create a new integer variable and add it to the model.
    pub fn new_var_int(&mut self, domain: IntDomain, name: Option<String>) -> Result<Rc<VarInt>> {
        let variable: Rc<VarInt> = VarInt::new(domain, name).into();
        self.add_var(variable.clone())?;
        Ok(variable)
    }

    /// Create a new boolean variable and add it to the model.
    pub fn new_var_bool(
        &mut self,
        domain: BoolDomain,
        name: Option<String>,
    ) -> Result<Rc<VarBool>> {
        let variable: Rc<VarBool> = VarBool::new(domain, name).into();
        self.add_var(variable.clone())?;
        Ok(variable)
    }

    /// Create a new integer parameter and add it to the model.
    ///
    /// Fail if the parameter name is already taken.
    pub fn new_par_int(&mut self, name: String, value: Int) -> Result<Rc<ParInt>> {
        let parameter: Rc<ParInt> = ParInt::new(name, value).into();
        self.add_par(parameter.clone())?;
        Ok(parameter)
    }

    /// Create a new boolean parameter and add it to the model.
    ///
    /// Fail if the parameter name is already taken.
    pub fn new_par_bool(&mut self, name: String, value: bool) -> Result<Rc<ParBool>> {
        let parameter: Rc<ParBool> = ParBool::new(name, value).into();
        self.add_par(parameter.clone())?;
        Ok(parameter)
    }

    /// Add the given constraint to the model.
    /// If needed, its arguments are added to the model.
    ///
    /// TODO: the constraint args might be unkown from the model
    pub fn add_constraint(&mut self, constraint: Constraint) -> Result<()> {
        self.constraints.push(constraint);
        Ok(())
    }
}

impl Default for Model {
    fn default() -> Self {
        Self {
            parameters: Default::default(),
            variables: Default::default(),
            constraints: Default::default(),
            solve_item: Default::default(),
            name_par: Default::default(),
            name_var: Default::default(),
        }
    }
}

impl Flatzinc for Model {
    fn fzn(&self) -> String {
        let mut s = String::new();
        s.extend(self.parameters.iter().map(|p| p.fzn()));
        s.extend(self.variables.iter().map(|v| v.fzn()));
        s
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
    fn simple_model() -> (Rc<VarInt>, Rc<VarInt>, Rc<ParInt>, Rc<ParBool>, Model) {
        let domain_x: IntDomain = IntRange::new(2, 5).unwrap().into();
        let domain_y: IntDomain = IntRange::new(-3, 3).unwrap().into();

        let mut model = Model::new();

        let x = model.new_var_int(domain_x, Some("x".to_string())).unwrap();
        let y = model.new_var_int(domain_y, Some("y".to_string())).unwrap();
        let t = model.new_par_int("t".to_string(), 4).unwrap();
        let s = model.new_par_bool("s".to_string(), true).unwrap();

        let c = IntEq::new(x.clone(), y.clone());
        model.add_constraint(c.into()).unwrap();

        (x, y, t, s, model)
    }

    #[test]
    fn basic_sat_model() {
        let (x, y, t, s, model) = simple_model();

        let variables: Vec<&Var> = model.variables().collect();
        let parameters: Vec<&Par> = model.parameters().collect();
        let constraints: Vec<&Constraint> = model.constraints().collect();

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

        let domain_z = IntRange::new(3, 9).unwrap().into();
        let z = model.new_var_int(domain_z, Some("z".to_string())).unwrap();

        model.optimize(Goal::Maximize, z.clone()).unwrap();

        let variables: Vec<&Var> = model.variables().collect();
        let parameters: Vec<&Par> = model.parameters().collect();
        let constraints: Vec<&Constraint> = model.constraints().collect();

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
        model.add_var(x.clone()).unwrap();

        assert!(model.optimize(Goal::Minimize, x).is_ok());
        assert!(model.optimize(Goal::Minimize, y).is_err());
    }

    #[test]
    fn get_vars_and_pars() {
        let (x, y, t, s, model) = simple_model();

        assert_eq!(
            *model.get_variable(&x.name().clone().unwrap()).unwrap(),
            Var::from(x.clone())
        );
        assert_eq!(
            *model.get_variable(&y.name().clone().unwrap()).unwrap(),
            Var::from(y.clone())
        );
        assert!(model.get_variable(t.name()).is_err());
        assert!(model.get_variable(s.name()).is_err());

        assert_eq!(*model.get_par(t.name()).unwrap(), Par::from(t.clone()));
        assert_eq!(*model.get_par(s.name()).unwrap(), Par::from(s.clone()));
        // assert!(model.get_par(&x.name().unwrap()).is_err());
        // assert!(model.get_par(&y.name().unwrap()).is_err());
    }

    #[test]
    fn contains() {
        let (x, y, t, s, model) = simple_model();
        let unknown = "unknown".to_string();

        assert!(!model.contains_par_name(&x.name().clone().unwrap()));
        assert!(!model.contains_par_name(&y.name().clone().unwrap()));
        assert!(model.contains_par_name(t.name()));
        assert!(model.contains_par_name(s.name()));
        assert!(!model.contains_par_name(&unknown));

        assert!(model.contains_var_name(&x.name().clone().unwrap()));
        assert!(model.contains_var_name(&y.name().clone().unwrap()));
        assert!(!model.contains_var_name(t.name()));
        assert!(!model.contains_var_name(s.name()));
        assert!(!model.contains_var_name(&unknown));

        assert!(model.contains_name(&x.name().clone().unwrap()));
        assert!(model.contains_name(&y.name().clone().unwrap()));
        assert!(model.contains_name(t.name()));
        assert!(model.contains_name(s.name()));
        assert!(!model.contains_name(&unknown));
    }

    #[test]
    fn same_name() {
        let mut model = Model::new();

        let _x = model
            .new_var_bool(BoolDomain, Some("x".to_string()))
            .unwrap();
        assert!(model
            .new_var_bool(BoolDomain, Some("x".to_string()))
            .is_err());
        assert!(model.new_par_int("x".to_string(), 5).is_err());
    }
}
