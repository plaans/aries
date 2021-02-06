use crate::solver::{Binding, BindingResult};
use crate::{Theory, TheoryResult};
use aries_backtrack::Backtrack;
use aries_backtrack::Q;
use aries_model::expressions::ExprHandle;
use aries_model::lang::Bound;
use aries_model::{Model, ModelEvents, WModel};

pub struct TheorySolver {
    theory: Box<dyn Theory>,
}

impl TheorySolver {
    pub fn new(theory: Box<dyn Theory>) -> TheorySolver {
        TheorySolver { theory }
    }

    pub fn bind(
        &mut self,
        lit: Bound,
        expr: ExprHandle,
        interner: &mut Model,
        queue: &mut Q<Binding>,
    ) -> BindingResult {
        self.theory.bind(lit, expr, interner, queue)
    }

    pub fn process(&mut self, queue: &mut ModelEvents, model: &mut WModel) -> TheoryResult {
        self.theory.propagate(queue, model)
    }

    pub fn print_stats(&self) {
        self.theory.print_stats()
    }
}

impl Backtrack for TheorySolver {
    fn save_state(&mut self) -> u32 {
        self.theory.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.theory.num_saved()
    }

    fn restore_last(&mut self) {
        self.theory.restore_last()
    }

    fn restore(&mut self, saved_id: u32) {
        self.theory.restore(saved_id)
    }
}
