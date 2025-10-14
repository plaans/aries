use smallvec::SmallVec;

use crate::{
    core::{IntCst, Lit, VarRef},
    model::{
        extensions::PartialBoolAssignment,
        lang::expr::{or, And, Leq, Or},
        Label, Model,
    },
    reif::ReifExpr,
};

pub trait Store {
    fn new_literal(&mut self, presence: Lit) -> Lit;
    fn get_implicant(&mut self, e: ReifExpr) -> Lit;
    fn add_implies(&mut self, l: Lit, e: ReifExpr);
    fn bounds(&self, _var: VarRef) -> (IntCst, IntCst) {
        (IntCst::MIN, IntCst::MAX)
    }
    fn entails(&self, l: Lit) -> bool;
    fn presence(&self, var: VarRef) -> Lit;

    fn conjunctive_scope(&mut self, lits: &[Lit]) -> Lit;
    fn tautology_of_scope(&mut self, scope: Lit) -> Lit;
}

impl<L: Label> Store for Model<L> {
    fn new_literal(&mut self, presence: Lit) -> Lit {
        self.state.new_optional_var(0, 1, presence).geq(1)
    }
    fn get_implicant(&mut self, e: ReifExpr) -> Lit {
        let l = self.half_reify(e.clone());
        println!("{l:?} -> {e:?}");
        l
    }

    fn add_implies(&mut self, l: Lit, e: ReifExpr) {
        println!("{l:?} -> {e:?}");
        self.enforce_if(l, e);
    }
    fn bounds(&self, var: VarRef) -> (IntCst, IntCst) {
        self.state.bounds(var)
    }
    fn entails(&self, l: Lit) -> bool {
        self.state.entails(l)
    }

    fn presence(&self, var: VarRef) -> Lit {
        self.state.presence_literal(var)
    }
    fn conjunctive_scope(&mut self, presence_variables: &[Lit]) -> Lit {
        self.get_conjunctive_scope(presence_variables)
    }
    fn tautology_of_scope(&mut self, scope: Lit) -> Lit {
        self.get_tautology_of_scope(scope)
    }
}

pub trait HReif {
    fn enforce_if(&self, l: Lit, store: &mut dyn Store);
    fn conj_scope(&self, prez: &dyn Fn(VarRef) -> Lit) -> Lits;
    fn scope(&self, store: &mut dyn Store) -> Lit {
        let conj_scope = self.conj_scope(&|v| store.presence(v));
        store.conjunctive_scope(&conj_scope)
    }
    fn opt_enforce_if(&self, l: Lit, store: &mut dyn Store) {
        let scope = self.scope(store);
        let implicant = if scope == store.presence(l.variable()) {
            l // TODO: here we should instead test that scope => prez(l)
        } else if store.entails(l) {
            store.tautology_of_scope(scope)
        } else {
            let imp = store.new_literal(scope);
            or([imp, !scope]).enforce_if(l, store);
            imp
        };
        self.enforce_if(implicant, store);
    }
    fn implicant(&self, store: &mut dyn Store) -> Lit {
        let scope = self.scope(store);
        let implicant = store.new_literal(scope);
        self.enforce_if(implicant, store);
        implicant
    }
    fn enforce(&self, store: &mut dyn Store) {
        let scope = self.scope(store);
        let imp = store.tautology_of_scope(scope);
        self.enforce_if(imp, store);
    }
}

pub type Lits = SmallVec<[Lit; 2]>;

impl HReif for ReifExpr {
    fn enforce_if(&self, l: Lit, store: &mut dyn Store) {
        store.add_implies(l, self.clone());
    }

    fn conj_scope(&self, prez: &dyn Fn(VarRef) -> Lit) -> Lits {
        let vs = self.scope(prez);
        // TODO: give flattening context
        let conj_scope = vs.to_conjunction(|_| Option::<[Lit; 0]>::None, |l| l == Lit::TRUE);
        SmallVec::from_iter(conj_scope.literals())
    }
}

// Derive `impl HReif` for Expression convertible to `ReifExpr`
crate::impl_reif!(Lit);
crate::impl_reif!(Or);
crate::impl_reif!(And);
crate::impl_reif!(Leq);

#[macro_export]
macro_rules! impl_reif {
    ($A: ty) => {
        impl HReif for $A
        where
            $A: Clone,
            ReifExpr: From<$A>,
        {
            fn enforce_if(&self, l: Lit, store: &mut dyn Store) {
                ReifExpr::from(self.clone()).enforce_if(l, store);
            }
            fn conj_scope(&self, prez: &dyn Fn(VarRef) -> Lit) -> Lits {
                ReifExpr::from(self.clone()).conj_scope(prez)
            }
        }
        impl HReif for &$A
        where
            $A: Clone,
            ReifExpr: From<$A>,
        {
            fn enforce_if(&self, l: Lit, store: &mut dyn Store) {
                ReifExpr::from(<$A>::clone(self)).enforce_if(l, store);
            }
            fn conj_scope(&self, prez: &dyn Fn(VarRef) -> Lit) -> Lits {
                ReifExpr::from(<$A>::clone(self)).conj_scope(prez)
            }
        }
    };
}

pub fn exclu_choice<T: HReif>(a: T, b: T) -> ExclusiveChoice<T> {
    ExclusiveChoice(a, b)
}

/// Represent a choice between two incompatible choices.
/// `ExclusiveChoice(a, b) <=> a or b` however it is in addition known
/// that  `(a -> !b) and (b -> !a)` (i.e. the two choices are mutually exclusive).
///
/// When enforced (half-reified to an always true literal),
/// we can thus create a single variable `l` and impose:
///   - `l -> a`
///   - `!l -> b`
pub struct ExclusiveChoice<T: HReif>(T, T);

impl<T: HReif> HReif for ExclusiveChoice<T> {
    fn enforce_if(&self, l: Lit, store: &mut dyn Store) {
        if store.entails(l) {
            // a tautolgy, create a single variable representing both options
            let choice_var = store.new_literal(store.presence(l.variable()));
            self.0.opt_enforce_if(choice_var, store);
            self.1.opt_enforce_if(!choice_var, store);
        } else {
            // no optimisation possible, resort to general formulation
            let a = self.0.implicant(store);
            let b = self.1.implicant(store);
            or([a, b]).opt_enforce_if(l, store);
        }
    }
    fn conj_scope(&self, prez: &dyn Fn(VarRef) -> Lit) -> Lits {
        let mut sa = self.0.conj_scope(prez);
        let sb = self.1.conj_scope(prez);
        sa.extend_from_slice(&sb);
        sa
    }
}

#[cfg(test)]
mod test {
    use crate::model::lang::{expr::neq, IAtom};

    use super::*;
    use crate::core::state::Term;

    /// All different with potentially optional variables
    struct AllDifferent(Vec<IAtom>);

    /// True if the the two atoms are different, and undefined if at least one is absent
    struct Different(IAtom, IAtom);

    impl HReif for AllDifferent {
        fn enforce_if(&self, l: Lit, store: &mut dyn Store) {
            for (i, t1) in self.0.iter().copied().enumerate() {
                for t2 in self.0[i + 1..].iter().copied() {
                    Different(t1, t2).opt_enforce_if(l, store);
                }
            }
        }

        fn conj_scope(&self, _prez: &dyn Fn(VarRef) -> Lit) -> Lits {
            smallvec::smallvec![]
        }
    }

    impl HReif for Different {
        fn enforce_if(&self, l: Lit, store: &mut dyn Store) {
            neq(self.0, self.1).opt_enforce_if(l, store);
        }

        fn conj_scope(&self, prez: &dyn Fn(VarRef) -> Lit) -> Lits {
            smallvec::smallvec![prez(self.0.variable()), prez(self.1.variable())]
        }
    }

    #[test]
    fn test() {
        let n = 3;
        let m: &mut Model<String> = &mut Model::new();
        let mut tasks = Vec::with_capacity(n);
        for i in 1..=n {
            let pi = m.new_presence_variable(Lit::TRUE, format!("p{i}")).true_lit();
            let ti = m.new_optional_ivar(0, 100, pi, format!("t{i}"));
            tasks.push(IAtom::from(ti));
        }

        let _activator = m.new_bvar("activator").true_lit();
        let no = AllDifferent(tasks);
        no.opt_enforce_if(Lit::TRUE, m);
        //no.opt_enforce_if(_activator, m);
        m.print_state();
    }
}
