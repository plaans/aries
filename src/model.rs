use crate::solve::SolveItem;

pub struct Model {
    solve_item: SolveItem,
}

impl Model {

    /// Return the model solve item.
    pub fn solve_item(self: &Self) -> &SolveItem {
        &self.solve_item
    }
}

