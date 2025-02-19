use super::Objective;

pub enum SolveItem {
    Satisfy,
    Optimize(Objective),
}