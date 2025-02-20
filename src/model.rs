use std::collections::HashSet;

use anyhow::ensure;
use anyhow::Result;

use crate::solve::Objective;
use crate::variable::Variable;
use crate::solve::Goal;
use crate::solve::SolveItem;

pub struct Model<'a> {
    variables: HashSet<Variable>,
    solve_item: SolveItem<'a>,
}

impl<'a> Model<'a> {

    /// Create a new empty satisfaction model.
    pub fn new() -> Self {
        let variables = HashSet::new();
        let solve_item = SolveItem::Satisfy;
        Model { variables, solve_item }
    }

    /// Turn the model into an optimization problem on the given variable
    /// 
    /// Return an `Error` iff the variable is not defined in the model.
    pub fn optimize(&mut self, goal: Goal, variable: &'a Variable) -> Result<()> {
        ensure!(self.variables.contains(variable), "the variable is not in the model");
        let objective = Objective { goal, variable };
        self.solve_item = SolveItem::Optimize(objective);
        Ok(())
    }

    /// Return the model solve item.
    pub fn solve_item(&self) -> &SolveItem {
        &self.solve_item
    }

    /// Return the model variables.
    pub fn variables(&self) -> &HashSet<Variable> {
        &self.variables
    }

    /// Add the given variable to the model.
    pub fn add_variable(&mut self, variable: Variable) {
        debug_assert!(self.variables.insert(variable), "variable should not already be in the model")
    }
}