use crate::variable::Variable;
use crate::solve::{Goal, Objective, SolveItem};

// TODO: use a smart pointer
pub struct Model {
    variables: Vec<Variable>,
    solve_item: SolveItem<'a>,
}

impl Model {

    /// Create a new satisfaction model.
    pub fn satisfy() -> Self {
        let variables = Vec::new();
        let solve_item = SolveItem::Satisfy;
        Model { variables, solve_item }
    }

    /// Create a new optimization model.
    pub fn optimize(variable: Variable, goal: Goal) -> Self {
        let variables = vec![variable];
        let objective = Objective { goal, variable: &variable };
        let solve_item = SolveItem::Optimize(objective);
        Model { variables, solve_item }
    }

    /// Create a new minimization model for the given variable.
    pub fn minimize(variable: Variable) -> Self {
        let variables = vec![variable];
        let solve_item = SolveItem::Optimize(())
        Model { variables, solve_item }
    }

    /// Return the model solve item.
    pub fn solve_item(&self) -> &SolveItem {
        &self.solve_item
    }
}

// impl From<SolveItem> for Model {

//     fn from(solve_item: SolveItem) -> Self {
//         if let Some(variable) = solve_item.variable() {
            
//         }
//     }
// }
