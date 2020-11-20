use crate::lang::{BAtom, BVar, Expr, Fun, Interner};
use crate::queues::{QReader, QWriter, Q};
use crate::Theory;
use aries_sat::all::Lit;
use std::collections::HashMap;
use std::convert::*;

pub struct ModularSMT {
    literal_bindings: Q<Lit>,
    interner: Interner,
    sat: SatSolver,
    theories: Vec<TheoryModule>,
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
        }
    }

    pub fn add_theory(&mut self, theory: Box<dyn Theory>) {
        let module = TheoryModule {
            input: self.literal_bindings.reader(),
            theory,
        };
        self.theories.push(module);
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
    _output: QWriter<Lit>, // TODO: rename
    sat: aries_sat::solver::Solver,
    tautology: Option<Lit>,
    map: HashMap<BVar, Lit>,
}
impl SatSolver {
    pub fn new(output: QWriter<Lit>) -> SatSolver {
        SatSolver {
            _output: output,
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
}

pub struct TheoryModule {
    input: QReader<Lit>,
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

    pub fn process(&mut self) -> TheoryResult {
        self.theory.propagate(&mut self.input)
    }
}

pub enum TheoryResult {
    Consistent,
    Contradiction(Vec<Lit>),
}
