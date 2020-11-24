use crate::lang::{BAtom, BVar, Expr, Fun, IVar, IntCst, Interner};
use crate::queues::{QReader, QWriter, Q};
use crate::Theory;
use aries_sat::all::Lit;
use aries_sat::solver::{ConflictHandlingResult, PropagationResult};
use std::collections::HashMap;
use std::convert::*;
use std::num::NonZeroU32;

pub struct ModularSMT {
    literal_bindings: Q<Lit>,
    pub interner: Interner,
    sat: SatSolver,
    theories: Vec<TheoryModule>,
    queues: Vec<QReader<Lit>>,
}
impl ModularSMT {
    pub fn new(model: Interner) -> ModularSMT {
        let literal_bindings = Q::new();
        let sat = SatSolver::new(literal_bindings.writer());
        ModularSMT {
            literal_bindings,
            interner: model,
            sat,
            theories: Vec::new(),
            queues: Vec::new(),
        }
    }

    pub fn add_theory(&mut self, theory: Box<dyn Theory>) {
        let module = TheoryModule { theory };
        self.theories.push(module);
        self.queues.push(self.literal_bindings.reader());
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
        let queue = Q::new();
        let mut out = queue.writer();
        let mut reader = queue.reader();
        for atom in constraints {
            match self.sat.enforce(*atom, &mut self.interner, &mut out) {
                EnforceResult::Enforced => (),
                EnforceResult::Reified(l) => out.push(Binding::new(l, *atom)),
                EnforceResult::Refined => (),
            }
        }

        while let Some(binding) = reader.pop() {
            let mut supported = false;
            let expr = self.interner.expr_of(binding.atom);
            // if the BAtom has not a corresponding expr, then it is a free variable and we can stop.
            if let Some(expr) = expr {
                match self.sat.bind(binding.lit, expr, &mut out) {
                    BindingResult::Enforced => supported = true,
                    BindingResult::Unsupported => {}
                    BindingResult::Refined => supported = true,
                }
                for theory in &mut self.theories {
                    match theory.bind(binding.lit, binding.atom, &mut self.interner, &mut out) {
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
            println!("PROPAGATE LOOP");
            if !self.propagate_and_backtrack_to_consistent() {
                println!("UNSAT");
                return false;
            }
            if let Some(decision) = self.next_decision() {
                self.decide(decision);
            } else {
                println!("SAT: no choice left");
                return true;
            }
        }
    }

    pub fn next_decision(&mut self) -> Option<Lit> {
        self.sat.sat.next_decision()
    }

    pub fn decide(&mut self, decision: Lit) {
        self.sat.sat.decide(decision);
        self.literal_bindings.writer().set_backtrack_point();
        self.literal_bindings.writer().push(decision);
        for th in &mut self.theories {
            th.set_backtrack_point();
        }
    }

    pub fn propagate_and_backtrack_to_consistent(&mut self) -> bool {
        loop {
            println!(" propagation and backtrack loop");
            match self.sat.propagate() {
                SatPropagationResult::Backtracked(n) => {
                    println!("  Backtracked {}", n.get());
                    for th in &mut self.theories {
                        for _ in 0..n.get() {
                            th.backtrack();
                        }
                    }
                    // skip theory propagations to repeat sat propagation,
                    continue;
                }
                SatPropagationResult::Inferred => (),
                SatPropagationResult::NoOp => (),
                SatPropagationResult::Unsat => return false,
            }
            println!("  SAT OK");

            let mut contradiction_found = false;
            for i in 0..self.theories.len() {
                debug_assert!(!contradiction_found);
                let th = &mut self.theories[i];
                let queue = &mut self.queues[i];
                if !queue.is_empty() {
                    match th.process(queue) {
                        TheoryResult::Consistent => {
                            println!("Theory: consistent");
                        }
                        TheoryResult::Contradiction(clause) => {
                            println!("  Theory: CONTRADICTION");
                            // learnt a new clause, add it to sat
                            // and skip the rest of the propagation
                            println!("   clause: {:?}", &clause);
                            self.sat.sat.add_forgettable_clause(&clause);
                            contradiction_found = true;
                            break;
                        }
                    }
                }
            }
            if !contradiction_found {
                // if we reach this point, no contradiction has been found
                break;
            }
        }
        let mut r = self.literal_bindings.reader();
        while let Some(l) = r.pop() {
            println!("{:?}", l);
        }
        true
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

impl ModularSMT {}

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

pub struct SatSolver {
    inferred: QWriter<Lit>, // TODO: rename
    sat: aries_sat::solver::Solver,
    tautology: Option<Lit>,
    map: HashMap<BVar, Lit>,
}
impl SatSolver {
    pub fn new(output: QWriter<Lit>) -> SatSolver {
        SatSolver {
            inferred: output,
            sat: aries_sat::solver::Solver::default(),
            tautology: None,
            map: Default::default(),
        }
    }

    fn bind(&mut self, reif: Lit, e: &Expr, bindings: &mut QWriter<Binding>) -> BindingResult {
        match e.fun {
            Fun::And => unimplemented!(),
            Fun::Or => {
                let mut disjuncts = Vec::with_capacity(e.args.len());
                for &a in &e.args {
                    let a = BAtom::try_from(a).expect("not a boolean");
                    let lit = self.reify(a);
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

    fn enforce(&mut self, b: BAtom, i: &mut Interner, bindings: &mut QWriter<Binding>) -> EnforceResult {
        // force literal to be true
        // TODO: we should check if the variable already exists and if not, provide tautology instead
        let lit = self.reify(b);
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
                        let lit = self.reify(a);
                        bindings.push(Binding::new(lit, a));
                        lits.push(lit);
                    }
                    self.sat.add_clause(&lits);
                    EnforceResult::Refined
                }
                _ => EnforceResult::Reified(self.reify(b)),
            }
        } else {
            EnforceResult::Enforced
        }
    }

    fn reify(&mut self, b: BAtom) -> Lit {
        let lit = match b.var {
            Some(x) if self.map.contains_key(&x) => self.map[&x],
            Some(x) => {
                let lit = self.sat.add_var().true_lit();
                self.map.insert(x, lit);
                lit
            }
            None => self.tautology(),
        };
        if b.negated {
            !lit
        } else {
            lit
        }
    }

    pub fn propagate(&mut self) -> SatPropagationResult {
        match self.sat.propagate() {
            PropagationResult::Conflict(clause) => {
                // we must handle conflict and backtrack in theory
                match self.sat.handle_conflict(clause) {
                    ConflictHandlingResult::Backtracked {
                        num_backtracks,
                        inferred,
                    } => {
                        for _ in 0..num_backtracks.get() {
                            self.inferred.backtrack();
                        }
                        self.inferred.push(inferred);
                        SatPropagationResult::Backtracked(num_backtracks)
                    }
                    ConflictHandlingResult::Unsat => SatPropagationResult::Unsat,
                }
            }
            PropagationResult::Inferred(lits) => {
                if lits.is_empty() {
                    SatPropagationResult::NoOp
                } else {
                    self.inferred.append(lits.iter().copied());
                    SatPropagationResult::Inferred
                }
            }
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
}

impl TheoryModule {
    pub fn bind(
        &mut self,
        lit: Lit,
        atom: BAtom,
        interner: &mut Interner,
        queue: &mut QWriter<Binding>,
    ) -> BindingResult {
        self.theory.bind(lit, atom, interner, queue)
    }

    pub fn process(&mut self, queue: &mut QReader<Lit>) -> TheoryResult {
        self.theory.propagate(queue)
    }

    pub fn backtrack(&mut self) {
        self.theory.backtrack();
    }

    pub fn set_backtrack_point(&mut self) {
        self.theory.set_backtrack_point();
    }
}

pub enum TheoryResult {
    Consistent,
    // TODO: make this a slice to avoid allocation
    Contradiction(Vec<Lit>),
}
