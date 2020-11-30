use crate::backtrack::Backtrack;
use crate::lang::{BAtom, BVar, Expr, Fun, IVar, IntCst, Interner};
use crate::model::{BoolModel, Model, ModelEvents, WModel, WriterId};
use crate::queues::{QReader, Q};
use crate::Theory;
use aries_sat::all::Lit;
use aries_sat::solver::{ConflictHandlingResult, PropagationResult};
use std::collections::HashMap;
use std::convert::*;
use std::num::NonZeroU32;

pub struct ModularSMT {
    pub interner: Interner,
    sat: SatSolver,
    theories: Vec<TheoryModule>,
    queues: Vec<ModelEvents>,
    model: Model,
    num_saved_states: u32,
}
impl ModularSMT {
    fn sat_token() -> WriterId {
        WriterId::new(1)
    }
    fn decision_token() -> WriterId {
        WriterId::new(0)
    }
    fn theory_token(theory_num: u8) -> WriterId {
        WriterId::new(2 + theory_num)
    }

    pub fn new(expr_trees: Interner) -> ModularSMT {
        let model = Model::default();
        let sat = SatSolver::new(Self::sat_token(), model.bool_event_reader());
        ModularSMT {
            interner: expr_trees,
            sat,
            theories: Vec::new(),
            queues: Vec::new(),
            model,
            num_saved_states: 0,
        }
    }
    pub fn add_theory(&mut self, theory: Box<dyn Theory>) {
        let module = TheoryModule {
            theory,
            num_saved_state: 0,
        };
        self.theories.push(module);
        self.queues.push(self.model.readers());
    }

    pub fn value_of(&self, bvar: BVar) -> Option<bool> {
        self.model.bools.value_of(bvar)
    }

    pub fn domain_of(&self, ivar: IVar) -> Option<(IntCst, IntCst)> {
        for th in &self.theories {
            let od = th.theory.domain_of(ivar);
            if od.is_some() {
                return od;
            }
        }
        None
    }

    pub fn enforce(&mut self, constraints: &[BAtom]) {
        let mut queue = Q::new();
        let mut reader = queue.reader();
        let model = &mut self.model.bools;

        for atom in constraints {
            match self.sat.enforce(*atom, &mut self.interner, &mut queue, model) {
                EnforceResult::Enforced => (),
                EnforceResult::Reified(l) => queue.push(Binding::new(l, *atom)),
                EnforceResult::Refined => (),
            }
        }

        while let Some(binding) = reader.pop() {
            let mut supported = false;
            let expr = self.interner.expr_of(binding.atom);
            // if the BAtom has not a corresponding expr, then it is a free variable and we can stop.

            if let Some(expr) = expr {
                match self.sat.bind(binding.lit, expr, &mut queue, model) {
                    BindingResult::Enforced => supported = true,
                    BindingResult::Unsupported => {}
                    BindingResult::Refined => supported = true,
                }
                for theory in &mut self.theories {
                    match theory.bind(binding.lit, binding.atom, &mut self.interner, &mut queue) {
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

impl Backtrack for ModularSMT {
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

type BoolChanges = QReader<(Lit, WriterId)>;

pub struct SatSolver {
    sat: aries_sat::solver::Solver,
    tautology: Option<Lit>,
    token: WriterId,
    changes: BoolChanges,
}
impl SatSolver {
    pub fn new(token: WriterId, changes: BoolChanges) -> SatSolver {
        SatSolver {
            sat: aries_sat::solver::Solver::default(),
            tautology: None,
            token,
            changes,
        }
    }

    fn bind(&mut self, reif: Lit, e: &Expr, bindings: &mut Q<Binding>, model: &mut BoolModel) -> BindingResult {
        match e.fun {
            Fun::And => unimplemented!(),
            Fun::Or => {
                let mut disjuncts = Vec::with_capacity(e.args.len());
                for &a in &e.args {
                    let a = BAtom::try_from(a).expect("not a boolean");
                    let lit = self.reify(a, model);
                    bindings.push(Binding::new(lit, a));
                    disjuncts.push(lit);
                }
                let mut clause = Vec::with_capacity(disjuncts.len() + 1);
                // make reif => disjuncts
                clause.push(!reif);
                disjuncts.iter().for_each(|l| clause.push(*l));
                self.sat.add_clause(&clause);
                for disjunct in disjuncts {
                    // enforce disjunct => reif
                    clause.clear();
                    clause.push(!disjunct);
                    clause.push(reif);
                    self.sat.add_clause(&clause);
                }
                BindingResult::Refined
            }
            _ => BindingResult::Unsupported,
        }
    }

    fn tautology(&mut self) -> Lit {
        if let Some(tauto) = self.tautology {
            tauto
        } else {
            let tauto = self.sat.add_var().true_lit();
            self.tautology = Some(tauto);
            self.sat.add_clause(&[tauto]);
            tauto
        }
    }

    fn enforce(
        &mut self,
        b: BAtom,
        i: &mut Interner,
        bindings: &mut Q<Binding>,
        model: &mut BoolModel,
    ) -> EnforceResult {
        // force literal to be true
        // TODO: we should check if the variable already exists and if not, provide tautology instead
        let lit = self.reify(b, model);
        self.sat.add_clause(&[lit]);

        if let Some(e) = i.expr_of(b) {
            match e.fun {
                Fun::And => {
                    // TODO: we should enforce all members directly
                    bindings.push(Binding::new(lit, b));
                    EnforceResult::Refined
                }
                Fun::Or => {
                    let mut lits = Vec::with_capacity(e.args.len());
                    for &a in &e.args {
                        let a = BAtom::try_from(a).expect("not a boolean");
                        let lit = self.reify(a, model);
                        bindings.push(Binding::new(lit, a));
                        lits.push(lit);
                    }
                    self.sat.add_clause(&lits);
                    EnforceResult::Refined
                }
                _ => EnforceResult::Reified(self.reify(b, model)),
            }
        } else {
            EnforceResult::Enforced
        }
    }

    fn reify(&mut self, b: BAtom, model: &mut BoolModel) -> Lit {
        let lit = match b.var {
            Some(x) => match model.literal_of(x) {
                Some(lit) => lit,
                None => {
                    let lit = self.sat.add_var().true_lit();
                    model.bind(x, lit);
                    lit
                }
            },
            None => self.tautology(),
        };
        if b.negated {
            !lit
        } else {
            lit
        }
    }

    pub fn propagate(&mut self, model: &mut BoolModel) -> SatPropagationResult {
        /// process pending model events
        while let Some((lit, writer)) = self.changes.pop() {
            if writer != self.token {
                self.sat.assume(lit);
            } else {
                debug_assert_eq!(
                    self.sat.get_literal(lit),
                    Some(true),
                    "We set a literal ourselves, but the solver does know aboud id"
                );
            }
        }
        match self.sat.propagate() {
            PropagationResult::Conflict(clause) => {
                // we must handle conflict and backtrack in theory
                match self.sat.handle_conflict(clause) {
                    ConflictHandlingResult::Backtracked {
                        num_backtracks,
                        inferred,
                    } => {
                        model.restore(model.num_saved() - num_backtracks.get());
                        model.set(inferred, self.token);
                        SatPropagationResult::Backtracked(num_backtracks)
                    }
                    ConflictHandlingResult::Unsat => SatPropagationResult::Unsat,
                }
            }
            PropagationResult::Inferred(lits) => {
                if lits.is_empty() {
                    SatPropagationResult::NoOp
                } else {
                    for l in lits {
                        model.set(*l, self.token);
                    }
                    SatPropagationResult::Inferred
                }
            }
        }
    }
}

impl Backtrack for SatSolver {
    fn save_state(&mut self) -> u32 {
        self.sat.save_state().num_decisions() - 1
    }

    fn num_saved(&self) -> u32 {
        self.sat.decision_level().num_decisions()
    }

    fn restore_last(&mut self) {
        if !self.sat.backtrack() {
            panic!("No state to restore.");
        }
    }
}

pub enum SatPropagationResult {
    /// The solver and the inference queue have backtracked `n` (n > 0) to handle a conflict.
    /// A literal (implied by the learnt clause) is placed on the queue.
    /// Note: propagation might not be finished as the learnt literal might
    /// not have been propagated.
    Backtracked(NonZeroU32),
    /// No conflict detected, some inferred literals are placed on the queue
    Inferred,
    /// No inference was made
    NoOp,
    Unsat,
}

pub struct TheoryModule {
    theory: Box<dyn Theory>,
    num_saved_state: u32,
}

impl TheoryModule {
    pub fn bind(&mut self, lit: Lit, atom: BAtom, interner: &mut Interner, queue: &mut Q<Binding>) -> BindingResult {
        self.theory.bind(lit, atom, interner, queue)
    }

    pub fn process(&mut self, queue: &mut ModelEvents, model: &mut WModel) -> TheoryResult {
        self.theory.propagate(queue, model)
    }
}

impl Backtrack for TheoryModule {
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

pub enum TheoryResult {
    Consistent,
    // TODO: make this a slice to avoid allocation
    Contradiction(Vec<Lit>),
}
