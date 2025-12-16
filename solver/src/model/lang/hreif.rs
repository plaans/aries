use smallvec::SmallVec;

use crate::{
    core::{state::Term, IntCst, Lit, VarRef},
    model::{
        extensions::PartialBoolAssignment,
        lang::expr::{or, And, Leq, Or},
        Label, Model,
    },
    reif::ReifExpr,
};

pub trait Store {
    fn new_literal(&mut self, presence: Lit) -> Lit;
    fn new_optional_var(&mut self, lb: IntCst, ub: IntCst, presence: Lit) -> VarRef;
    fn get_implicant(&mut self, e: ReifExpr) -> Lit;
    fn add_implies(&mut self, l: Lit, e: ReifExpr);
    fn bounds(&self, var: VarRef) -> (IntCst, IntCst);
    fn entails(&self, l: Lit) -> bool;

    /// Returns the literal indicating whether the variable is present.
    ///
    /// See [`presence`] for a more general version.
    fn presence_of_var(&self, var: VarRef) -> Lit;
    fn conjunctive_scope(&mut self, lits: &[Lit]) -> Lit;
    fn tautology_of_scope(&mut self, scope: Lit) -> Lit;

    /// Returns the literal indicate the whether the term is present.
    ///
    /// Note: this method is not dyn-compatible.
    /// [`presence_of_var`] may be used as a more verbose fall-back.
    fn presence(&self, var: impl Term) -> Lit
    where
        Self: Sized,
    {
        self.presence_of_var(var.variable())
    }
}

pub trait ModelWrapper {
    type Lbl: Label;
    fn get_model(&self) -> &Model<Self::Lbl>;
    fn get_model_mut(&mut self) -> &mut Model<Self::Lbl>;
}
impl<L: Label> ModelWrapper for Model<L> {
    type Lbl = L;

    fn get_model(&self) -> &Model<Self::Lbl> {
        self
    }

    fn get_model_mut(&mut self) -> &mut Model<Self::Lbl> {
        self
    }
}

impl<T> Store for T
where
    T: ModelWrapper,
{
    fn new_literal(&mut self, presence: Lit) -> Lit {
        self.get_model_mut().state.new_optional_var(0, 1, presence).geq(1)
    }
    fn new_optional_var(&mut self, lb: IntCst, ub: IntCst, presence: Lit) -> VarRef {
        self.get_model_mut().state.new_optional_var(lb, ub, presence)
    }
    fn get_implicant(&mut self, e: ReifExpr) -> Lit {
        let l = self.get_model_mut().half_reify(e.clone());
        l
    }

    fn add_implies(&mut self, l: Lit, e: ReifExpr) {
        //println!("[{:?}] {l:?} -> {e:?}", self.presence(l)); // TODO: remove
        self.get_model_mut().enforce_if(l, e);
    }
    fn bounds(&self, var: VarRef) -> (IntCst, IntCst) {
        self.get_model().state.bounds(var)
    }
    fn entails(&self, l: Lit) -> bool {
        self.get_model().state.entails(l)
    }

    fn presence_of_var(&self, var: VarRef) -> Lit {
        self.get_model().state.presence_literal(var)
    }
    fn conjunctive_scope(&mut self, presence_variables: &[Lit]) -> Lit {
        self.get_model_mut().get_conjunctive_scope(presence_variables)
    }
    fn tautology_of_scope(&mut self, scope: Lit) -> Lit {
        self.get_model_mut().get_tautology_of_scope(scope)
    }
}

pub trait BoolExpr<Ctx> {
    fn enforce_if(&self, l: Lit, ctx: &Ctx, store: &mut dyn Store);

    /// Returns a set of literals that must all be true for the expression to be valid.
    /// The list is interpreted as a set: order and redundant elements are ignored.
    ///
    /// Examples:
    ///   - (a < b) would have a conjunctive scope `[prez(a), prez(b)]` as it is only valid when both
    ///     a and b are present. The conjunctive scope is thus the list their presence variable.
    fn conj_scope(&self, ctx: &Ctx, store: &dyn Store) -> Lits;
    fn scope(&self, ctx: &Ctx, store: &mut dyn Store) -> Lit {
        let conj_scope = self.conj_scope(ctx, store);
        store.conjunctive_scope(&conj_scope)
    }
    fn opt_enforce_if(&self, l: Lit, ctx: &Ctx, store: &mut dyn Store) {
        let scope = self.scope(ctx, store);
        let implicant = if scope == store.presence_of_var(l.variable()) {
            l // TODO: here we should instead test that scope => prez(l)
        } else if store.entails(l) {
            store.tautology_of_scope(scope)
        } else {
            let imp = store.new_literal(scope);
            or([imp, !scope]).enforce_if(l, ctx, store);
            imp
        };
        self.enforce_if(implicant, ctx, store);
    }
    fn implicant(&self, ctx: &Ctx, store: &mut dyn Store) -> Lit {
        let scope = self.scope(ctx, store);
        let implicant = store.new_literal(scope);
        self.enforce_if(implicant, ctx, store);
        implicant
    }
    fn enforce(&self, ctx: &Ctx, store: &mut dyn Store) {
        self.opt_enforce_if(Lit::TRUE, ctx, store);
    }
}

impl<Ctx, T: BoolExpr<Ctx>> BoolExpr<Ctx> for &T {
    fn enforce_if(&self, l: Lit, ctx: &Ctx, store: &mut dyn Store) {
        (*self).enforce_if(l, ctx, store);
    }

    fn conj_scope(&self, ctx: &Ctx, store: &dyn Store) -> Lits {
        (*self).conj_scope(ctx, store)
    }
    fn implicant(&self, ctx: &Ctx, store: &mut dyn Store) -> Lit {
        (*self).implicant(ctx, store)
    }

    //TODO: - check that we call the right one
    //      - implement all other methods to make sure we use the most specific implementation
}

pub type Lits = SmallVec<[Lit; 2]>;
#[macro_export]
macro_rules! lits {
    ($($x:tt)*) => {smallvec::smallvec![$($x)*]}
}

impl<Ctx> BoolExpr<Ctx> for ReifExpr {
    fn enforce_if(&self, l: Lit, _ctx: &Ctx, store: &mut dyn Store) {
        store.add_implies(l, self.clone());
    }

    fn conj_scope(&self, _ctx: &Ctx, store: &dyn Store) -> Lits {
        let vs = self.scope(|v| store.presence_of_var(v));
        // TODO: give flattening context
        let conj_scope = vs.to_conjunction(|_| Option::<[Lit; 0]>::None, |l| l == Lit::TRUE);
        SmallVec::from_iter(conj_scope.literals())
    }
    fn implicant(&self, _ctx: &Ctx, store: &mut dyn Store) -> Lit {
        if let ReifExpr::Lit(l) = self {
            *l // short circuit happy case
        } else {
            store.get_implicant(self.clone())
        }
    }
}

// Derive `impl BoolExpr<_>` for Expression convertible to `ReifExpr`
crate::impl_reif!(Lit);
crate::impl_reif!(Or);
crate::impl_reif!(And);
crate::impl_reif!(Leq);

#[macro_export]
macro_rules! impl_reif {
    ($A: ty) => {
        impl<Ctx> BoolExpr<Ctx> for $A
        where
            $A: Clone,
            ReifExpr: From<$A>,
        {
            fn enforce_if(&self, l: Lit, ctx: &Ctx, store: &mut dyn Store) {
                ReifExpr::from(self.clone()).enforce_if(l, ctx, store);
            }
            fn conj_scope(&self, ctx: &Ctx, store: &dyn Store) -> Lits {
                ReifExpr::from(self.clone()).conj_scope(ctx, store)
            }
            fn implicant(&self, _ctx: &Ctx, store: &mut dyn Store) -> Lit {
                store.get_implicant(ReifExpr::from(self.clone()))
            }
        }
    };
}

pub fn exclu_choice<T>(alt1: T, alt2: T) -> ExclusiveChoice<T> {
    ExclusiveChoice { alt1, alt2 }
}

/// Represent a choice between two incompatible choices.
/// `ExclusiveChoice(a, b) <=> a or b` however it is in addition known
/// that  `(a -> !b) and (b -> !a)` (i.e. the two choices are mutually exclusive).
///
/// When enforced (half-reified to an always true literal),
/// we can thus create a single variable `l` and impose:
///   - `l -> a`
///   - `!l -> b`
pub struct ExclusiveChoice<T> {
    /// First alternative (exclusive to the second one)
    alt1: T,
    /// Second alternative (exclusive to the first one)
    alt2: T,
}

impl<Ctx, T: BoolExpr<Ctx>> BoolExpr<Ctx> for ExclusiveChoice<T> {
    fn enforce_if(&self, l: Lit, ctx: &Ctx, store: &mut dyn Store) {
        if store.entails(l) {
            // a tautolgy, create a single variable representing both options
            let choice_var = store.new_literal(store.presence_of_var(l.variable()));
            self.alt1.opt_enforce_if(choice_var, ctx, store);
            self.alt2.opt_enforce_if(!choice_var, ctx, store);
        } else {
            // no optimisation possible, resort to general formulation
            let a = self.alt1.implicant(ctx, store);
            let b = self.alt2.implicant(ctx, store);
            or([a, b]).opt_enforce_if(l, ctx, store);
        }
    }
    fn conj_scope(&self, ctx: &Ctx, store: &dyn Store) -> Lits {
        let mut sa = self.alt1.conj_scope(ctx, store);
        let sb = self.alt2.conj_scope(ctx, store);
        sa.extend_from_slice(&sb);
        sa
    }
}

#[cfg(test)]
mod test {
    use crate::{
        model::{
            extensions::AssignmentExt,
            lang::{
                expr::{lt, neq},
                Atom, IAtom,
            },
        },
        solver::Solver,
    };

    use super::*;
    use crate::core::state::Term;

    /// All different with potentially optional variables
    struct AllDifferent(Vec<IAtom>);

    /// True if the the two atoms are different, and undefined if at least one is absent
    struct Different(IAtom, IAtom);

    fn prez(v: impl Term, store: &dyn Store) -> Lit {
        store.presence_of_var(v.variable())
    }

    impl<Ctx> BoolExpr<Ctx> for AllDifferent {
        fn enforce_if(&self, l: Lit, ctx: &Ctx, store: &mut dyn Store) {
            for (i, t1) in self.0.iter().copied().enumerate() {
                for t2 in self.0[i + 1..].iter().copied() {
                    Different(t1, t2).opt_enforce_if(l, ctx, store);
                }
            }
        }

        fn conj_scope(&self, _ctx: &Ctx, _store: &dyn Store) -> Lits {
            lits![]
        }
    }

    impl<Ctx> BoolExpr<Ctx> for Different {
        fn enforce_if(&self, l: Lit, ctx: &Ctx, store: &mut dyn Store) {
            neq(self.0, self.1).opt_enforce_if(l, ctx, store);
        }

        fn conj_scope(&self, _ctx: &Ctx, store: &dyn Store) -> Lits {
            lits![prez(self.0, store), prez(self.1, store)]
        }
    }

    #[test]
    fn test_alldiff_exp() {
        let n = 2;
        let mut m: Model<String> = Model::new();
        let mut tasks = Vec::with_capacity(n);
        for i in 1..=n {
            let pi = m.new_presence_variable(Lit::TRUE, format!("p{i}")).true_lit();
            let ti = m.new_optional_ivar(0, 1, pi, format!("t{i}"));
            tasks.push(IAtom::from(ti));
        }

        let _activator = m.new_bvar("activator").true_lit();
        let no = AllDifferent(tasks.clone());
        no.opt_enforce_if(Lit::TRUE, &(), &mut m);
        //no.opt_enforce_if(_activator, &mut m);
        m.print_state();

        let s = Solver::new(m);
        let count = enumerate(s, &tasks);
        assert_eq!(count, 7);
    }
    fn enumerate<L: Label>(mut s: Solver<L>, ints: &[IAtom]) -> i32 {
        let mut vars = Vec::with_capacity(ints.len() * 2);
        for &i in ints {
            vars.push(i.variable());
            vars.push(s.model.presence(i.variable()).variable());
        }
        vars.sort();
        vars.dedup();
        let mut counter = 0;
        s.enumerate_with(&vars, |sol| {
            counter += 1;
            println!("Sol {}", &counter);
            for i in ints {
                print!("  {i:?}: ");
                if sol.present(*i) == Some(true) {
                    println!("{:?}", sol.evaluate(Atom::from(*i)).unwrap());
                } else {
                    println!("-");
                }
            }
        })
        .expect("error returned by enumerate");
        counter
    }

    type TaskId = usize;
    struct Starts(Vec<IAtom>);

    struct Ordered(TaskId, TaskId);

    impl BoolExpr<Starts> for Ordered {
        fn enforce_if(&self, l: Lit, ctx: &Starts, store: &mut dyn Store) {
            let c = lt(ctx.0[self.0], ctx.0[self.1]);
            c.opt_enforce_if(l, ctx, store);
        }

        fn conj_scope(&self, ctx: &Starts, store: &dyn Store) -> Lits {
            let c = lt(ctx.0[self.0], ctx.0[self.1]);
            c.conj_scope(ctx, store)
        }
    }

    #[test]
    fn test_ctx2() {
        let mut store: Model<String> = Model::new();
        let s1 = store.new_ivar(0, 1000, "start1");
        let s2 = store.new_ivar(0, 1000, "start2");
        let s3 = store.new_ivar(0, 1000, "start3");
        let starts = Starts(vec![s1.into(), s2.into(), s3.into()]);
        let x = Ordered(1, 2);
        let y: &dyn BoolExpr<_> = &x;

        y.opt_enforce_if(Lit::FALSE, &starts, &mut store);

        let e = ReifExpr::And(crate::core::literals::Lits::new());
        e.opt_enforce_if(Lit::TRUE, &starts, &mut store);
    }
}
