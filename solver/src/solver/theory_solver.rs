use crate::backtrack::{Backtrack, DecLvl};
use crate::core::state::Domains;
use crate::core::Lit;
use crate::model::lang::reification::Expr;
use crate::solver::solver_impl::BindingResult;
use crate::solver::{Contradiction, Theory};

// TODO: remove this useless layer
pub struct TheorySolver {
    pub theory: Box<dyn Theory>,
}

impl TheorySolver {
    pub fn new(theory: Box<dyn Theory>) -> TheorySolver {
        TheorySolver { theory }
    }

    pub fn bind(&mut self, lit: Lit, expr: &Expr, domains: &mut Domains) -> BindingResult {
        self.theory.bind(lit, expr, domains)
    }

    pub fn process(&mut self, domains: &mut Domains) -> Result<(), Contradiction> {
        self.theory.propagate(domains)
    }

    pub fn print_stats(&self) {
        self.theory.print_stats()
    }
}

impl Clone for TheorySolver {
    fn clone(&self) -> Self {
        TheorySolver {
            theory: self.theory.clone_box(),
        }
    }
}

impl Backtrack for TheorySolver {
    fn save_state(&mut self) -> DecLvl {
        self.theory.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.theory.num_saved()
    }

    fn restore_last(&mut self) {
        self.theory.restore_last()
    }

    fn restore(&mut self, saved_id: DecLvl) {
        self.theory.restore(saved_id)
    }
}
