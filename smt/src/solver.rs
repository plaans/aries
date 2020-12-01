pub mod sat_solver;
pub mod theory_solver;

use crate::backtrack::Backtrack;
use crate::model::lang::BAtom;
use crate::model::{Model, ModelEvents, WriterId};
use crate::queues::Q;
use crate::{Theory, TheoryResult};
use aries_sat::all::Lit;

use crate::solver::sat_solver::{SatPropagationResult, SatSolver};
use crate::solver::theory_solver::TheorySolver;

pub struct SMTSolver {
    pub model: Model,
    sat: SatSolver,
    theories: Vec<TheorySolver>,
    queues: Vec<ModelEvents>,
    num_saved_states: u32,
}
impl SMTSolver {
    fn sat_token() -> WriterId {
        WriterId::new(1)
    }
    fn decision_token() -> WriterId {
        WriterId::new(0)
    }
    fn theory_token(theory_num: u8) -> WriterId {
        WriterId::new(2 + theory_num)
    }

    pub fn new(model: Model) -> SMTSolver {
        let sat = SatSolver::new(Self::sat_token(), model.bool_event_reader());
        SMTSolver {
            model,
            sat,
            theories: Vec::new(),
            queues: Vec::new(),
            num_saved_states: 0,
        }
    }
    pub fn add_theory(&mut self, theory: Box<dyn Theory>) {
        let module = TheorySolver::new(theory);
        self.theories.push(module);
        self.queues.push(self.model.readers());
    }

    pub fn enforce(&mut self, constraints: &[BAtom]) {
        let mut queue = Q::new();
        let mut reader = queue.reader();

        for atom in constraints {
            match self.sat.enforce(*atom, &mut self.model, &mut queue) {
                EnforceResult::Enforced => (),
                EnforceResult::Reified(l) => queue.push(Binding::new(l, *atom)),
                EnforceResult::Refined => (),
            }
        }

        while let Some(binding) = reader.pop() {
            let mut supported = false;
            let expr = self.model.expressions.expr_of(binding.atom);
            // if the BAtom has not a corresponding expr, then it is a free variable and we can stop.

            if let Some(expr) = expr {
                match self.sat.bind(binding.lit, expr, &mut queue, &mut self.model.bools) {
                    BindingResult::Enforced => supported = true,
                    BindingResult::Unsupported => {}
                    BindingResult::Refined => supported = true,
                }
                for theory in &mut self.theories {
                    match theory.bind(binding.lit, binding.atom, &mut self.model, &mut queue) {
                        BindingResult::Enforced => supported = true,
                        BindingResult::Unsupported => {}
                        BindingResult::Refined => supported = true,
                    }
                }
            }

            assert!(supported, "Unsupported binding")
        }
    }

    pub fn solve(&mut self) -> bool {
        loop {
            if !self.propagate_and_backtrack_to_consistent() {
                // UNSAT
                return false;
            }
            if let Some(decision) = self.next_decision() {
                self.decide(decision);
            } else {
                // SAT: consistent + no choices left
                return true;
            }
        }
    }

    pub fn next_decision(&mut self) -> Option<Lit> {
        self.sat.sat.next_decision()
    }

    pub fn decide(&mut self, decision: Lit) {
        self.save_state();
        self.model.bools.set(decision, Self::decision_token());
    }

    pub fn propagate_and_backtrack_to_consistent(&mut self) -> bool {
        loop {
            let bool_model = &mut self.model.bools;
            match self.sat.propagate(bool_model) {
                SatPropagationResult::Backtracked(n) => {
                    let bt_point = self.num_saved_states - n.get();
                    self.restore(bt_point);

                    // skip theory propagations to repeat sat propagation,
                    continue;
                }
                SatPropagationResult::Inferred => (),
                SatPropagationResult::NoOp => (),
                SatPropagationResult::Unsat => return false,
            }

            let mut contradiction_found = false;
            for i in 0..self.theories.len() {
                debug_assert!(!contradiction_found);
                let th = &mut self.theories[i];
                let queue = &mut self.queues[i];
                match th.process(queue, &mut self.model.writer(Self::theory_token(i as u8))) {
                    TheoryResult::Consistent => {
                        // theory is consistent
                    }
                    TheoryResult::Contradiction(clause) => {
                        // theory contradiction.
                        // learnt a new clause, add it to sat
                        // and skip the rest of the propagation
                        self.sat.sat.add_forgettable_clause(&clause);
                        contradiction_found = true;
                        break;
                    }
                }
            }
            if !contradiction_found {
                // if we reach this point, no contradiction has been found
                break;
            }
        }
        true
    }
}

impl Backtrack for SMTSolver {
    fn save_state(&mut self) -> u32 {
        self.num_saved_states += 1;
        let n = self.num_saved_states - 1;
        assert_eq!(self.model.save_state(), n);
        assert_eq!(self.sat.save_state(), n);
        for th in &mut self.theories {
            assert_eq!(th.save_state(), n);
        }
        n
    }

    fn num_saved(&self) -> u32 {
        self.num_saved_states
    }

    fn restore_last(&mut self) {
        assert!(self.num_saved() > 0, "No state to restore");
        let last = self.num_saved() - 1;
        self.restore(last);
        self.num_saved_states -= 1;
    }

    fn restore(&mut self, saved_id: u32) {
        self.num_saved_states = saved_id;
        self.model.restore(saved_id);
        self.sat.restore(saved_id);
        for th in &mut self.theories {
            th.restore(saved_id);
        }
    }
}

#[derive(Copy, Clone)]
pub struct Binding {
    lit: Lit,
    atom: BAtom,
}
impl Binding {
    pub fn new(lit: Lit, atom: BAtom) -> Binding {
        Binding { lit, atom }
    }
}

pub enum EnforceResult {
    Enforced,
    Reified(Lit),
    Refined,
}

pub enum BindingResult {
    Enforced,
    Unsupported,
    Refined,
}
