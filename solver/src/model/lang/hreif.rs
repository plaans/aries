use crate::{
    core::{IntCst, Lit, VarRef, views::Dom},
    model::{
        Label, Model,
        lang::{
            expr::{And, Leq, Or, or},
            linear::LinearLeq,
            max::{EqMax, EqMin},
        },
    },
    prelude::*,
    reif::ReifExpr,
};

pub trait Store: Dom {
    fn new_literal(&mut self, presence: Lit) -> Lit;
    fn new_optional_var(&mut self, lb: IntCst, ub: IntCst, presence: Lit) -> VarRef;
    fn get_implicant(&mut self, e: ReifExpr) -> Lit;
    fn add_implies(&mut self, l: Lit, e: ReifExpr);
    // fn bounds(&self, var: VarRef) -> (IntCst, IntCst);
    // fn entails(&self, l: Lit) -> bool;

    // /// Returns the literal indicating whether the variable is present.
    // ///
    // /// See [`presence`] for a more general version.
    // fn presence_of_var(&self, var: VarRef) -> Lit;
    fn conjunctive_scope(&mut self, lits: &[Lit]) -> Lit;
    fn tautology_of_scope(&mut self, scope: Lit) -> Lit;

    // /// Returns the literal indicate the whether the term is present.
    // ///
    // /// Note: this method is not dyn-compatible.
    // /// [`presence_of_var`] may be used as a more verbose fall-back.
    // fn presence(&self, var: impl Term) -> Lit
    // where
    //     Self: Sized,
    // {
    //     self.presence_of_var(var.variable())
    // }
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
    T: ModelWrapper + Dom,
{
    fn new_literal(&mut self, presence: Lit) -> Lit {
        self.get_model_mut().state.new_optional_var(0, 1, presence).geq(1)
    }
    fn new_optional_var(&mut self, lb: IntCst, ub: IntCst, presence: Lit) -> VarRef {
        self.get_model_mut().state.new_optional_var(lb, ub, presence)
    }
    fn get_implicant(&mut self, e: ReifExpr) -> Lit {
        self.get_model_mut().half_reify(e.clone())
    }

    fn add_implies(&mut self, l: Lit, e: ReifExpr) {
        //println!("[{:?}] {l:?} -> {e:?}", self.presence(l)); // TODO: remove
        self.get_model_mut().enforce_if(l, e);
    }

    fn conjunctive_scope(&mut self, presence_variables: &[Lit]) -> Lit {
        self.get_model_mut().get_conjunctive_scope(presence_variables)
    }
    fn tautology_of_scope(&mut self, scope: Lit) -> Lit {
        self.get_model_mut().get_tautology_of_scope(scope)
    }
}
pub trait BoolExpr<Ctx: Store> {
    fn enforce_if(&self, l: Lit, ctx: &mut Ctx);

    /// Returns a set of literals that must all be true for the expression to be valid.
    /// The list is interpreted as a set: order and redundant elements are ignored.
    ///
    /// Examples:
    ///   - (a < b) would have a conjunctive scope `[prez(a), prez(b)]` as it is only valid when both
    ///     a and b are present. The conjunctive scope is thus the list their presence variable.
    fn conj_scope(&self, ctx: &Ctx) -> Conjunction; // TODO: should be Conjunction
    fn scope(&self, ctx: &mut Ctx) -> Lit {
        let conj_scope = self.conj_scope(ctx);
        ctx.conjunctive_scope(&conj_scope)
    }
    fn opt_enforce_if(&self, l: Lit, ctx: &mut Ctx) {
        let scope = self.scope(ctx);
        let implicant = if scope == ctx.presence(l.variable()) {
            l // TODO: here we should instead test that scope => prez(l)
        } else if ctx.entails(l) {
            ctx.tautology_of_scope(scope)
        } else {
            let imp = ctx.new_literal(scope);
            or([imp, !scope]).enforce_if(l, ctx);
            imp
        };
        self.enforce_if(implicant, ctx);
    }
    fn implicant(&self, ctx: &mut Ctx) -> Lit {
        let scope = self.scope(ctx);
        let implicant = ctx.new_literal(scope);
        self.enforce_if(implicant, ctx);
        implicant
    }
    fn reified<'a, NotSelf>(&'a self, ctx: &mut Ctx) -> Lit
    where
        Self: Sized,
        &'a Self: std::ops::Not<Output = NotSelf>,
        NotSelf: BoolExpr<Ctx>,
    {
        let implicant = self.implicant(ctx);
        let negated = !self;
        negated.enforce_if(!implicant, ctx);
        implicant
    }
    fn enforce(&self, ctx: &mut Ctx) {
        self.opt_enforce_if(Lit::TRUE, ctx);
    }
}

impl<Ctx: Store, T: BoolExpr<Ctx>> BoolExpr<Ctx> for &T {
    fn enforce_if(&self, l: Lit, ctx: &mut Ctx) {
        (*self).enforce_if(l, ctx);
    }

    fn conj_scope(&self, ctx: &Ctx) -> Conjunction {
        (*self).conj_scope(ctx)
    }
    fn implicant(&self, ctx: &mut Ctx) -> Lit {
        (*self).implicant(ctx)
    }

    //TODO: - check that we call the right one
    //      - implement all other methods to make sure we use the most specific implementation
}

impl<Ctx: Store> BoolExpr<Ctx> for ReifExpr {
    fn enforce_if(&self, l: Lit, ctx: &mut Ctx) {
        ctx.add_implies(l, self.clone());
    }

    fn conj_scope(&self, ctx: &Ctx) -> Conjunction {
        let vs = self.scope(|v| ctx.presence_literal(v));
        // TODO: give flattening context
        let conj_scope = vs.to_conjunction(|_| Option::<[Lit; 0]>::None, |l| l == Lit::TRUE);
        Conjunction::from_iter(conj_scope.literals())
    }
    fn implicant(&self, ctx: &mut Ctx) -> Lit {
        if let ReifExpr::Lit(l) = self {
            *l // short circuit happy case
        } else {
            ctx.get_implicant(self.clone())
        }
    }
}

// Derive `impl BoolExpr<_>` for Expression convertible to `ReifExpr`
crate::impl_reif!(Lit);
crate::impl_reif!(Or);
crate::impl_reif!(And);
crate::impl_reif!(Leq);
crate::impl_reif!(LinearLeq);
crate::impl_reif!(EqMax);
crate::impl_reif!(EqMin);

#[macro_export]
macro_rules! impl_reif {
    ($A: ty) => {
        impl<Ctx: Store> BoolExpr<Ctx> for $A
        where
            $A: Clone,
            ReifExpr: From<$A>,
        {
            fn enforce_if(&self, l: Lit, ctx: &mut Ctx) {
                ReifExpr::from(self.clone()).enforce_if(l, ctx);
            }
            fn conj_scope(&self, ctx: &Ctx) -> Conjunction {
                ReifExpr::from(self.clone()).conj_scope(ctx)
            }
            fn implicant(&self, ctx: &mut Ctx) -> Lit {
                ctx.get_implicant(ReifExpr::from(self.clone()))
            }
            // TODO: add reification impl
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

impl<Ctx: Store, T: BoolExpr<Ctx>> BoolExpr<Ctx> for ExclusiveChoice<T> {
    fn enforce_if(&self, l: Lit, ctx: &mut Ctx) {
        if ctx.entails(l) {
            // a tautolgy, create a single variable representing both options
            let choice_var = ctx.new_literal(ctx.presence_literal(l));
            self.alt1.opt_enforce_if(choice_var, ctx);
            self.alt2.opt_enforce_if(!choice_var, ctx);
        } else {
            // no optimisation possible, resort to general formulation
            let a = self.alt1.implicant(ctx);
            let b = self.alt2.implicant(ctx);
            or([a, b]).opt_enforce_if(l, ctx);
        }
    }
    fn conj_scope(&self, ctx: &Ctx) -> Conjunction {
        let mut sa = self.alt1.conj_scope(ctx).into_lits();
        let sb = self.alt2.conj_scope(ctx);
        sa.extend_from_slice(&sb);
        sa.into()
    }
}

#[cfg(test)]
mod test {
    use crate::{
        core::views::Term,
        model::{
            extensions::DomainsExt,
            lang::{
                Atom, IAtom,
                expr::{lt, neq},
            },
        },
        solver::{SearchLimit, Solver},
    };

    use super::*;

    /// All different with potentially optional variables
    struct AllDifferent(Vec<IAtom>);

    /// True if the the two atoms are different, and undefined if at least one is absent
    struct Different(IAtom, IAtom);

    impl<Ctx: Store> BoolExpr<Ctx> for AllDifferent {
        fn enforce_if(&self, l: Lit, ctx: &mut Ctx) {
            for (i, t1) in self.0.iter().copied().enumerate() {
                for t2 in self.0[i + 1..].iter().copied() {
                    Different(t1, t2).opt_enforce_if(l, ctx);
                }
            }
        }

        fn conj_scope(&self, _ctx: &Ctx) -> Conjunction {
            Conjunction::tautology()
        }
    }

    impl<Ctx: Store> BoolExpr<Ctx> for Different {
        fn enforce_if(&self, l: Lit, ctx: &mut Ctx) {
            neq(self.0, self.1).opt_enforce_if(l, ctx);
        }

        fn conj_scope(&self, ctx: &Ctx) -> Conjunction {
            [ctx.presence_literal(self.0), ctx.presence_literal(self.1)].into()
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
        no.opt_enforce_if(Lit::TRUE, &mut m);
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
        s.enumerate_with(
            &vars,
            |sol| {
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
            },
            SearchLimit::None,
        )
        .expect("error returned by enumerate");
        counter
    }

    type TaskId = usize;

    struct Ordered(TaskId, TaskId);

    struct ModelWithMetadata {
        starts: Vec<IAtom>,
        model: Model<String>,
    }
    impl ModelWrapper for ModelWithMetadata {
        type Lbl = String;

        fn get_model(&self) -> &Model<Self::Lbl> {
            &self.model
        }

        fn get_model_mut(&mut self) -> &mut Model<Self::Lbl> {
            &mut self.model
        }
    }

    impl Dom for ModelWithMetadata {
        fn upper_bound(&self, svar: SignedVar) -> IntCst {
            self.model.upper_bound(svar)
        }

        fn presence(&self, var: VarRef) -> Lit {
            self.model.presence(var)
        }
    }

    impl BoolExpr<ModelWithMetadata> for Ordered {
        fn enforce_if(&self, l: Lit, ctx: &mut ModelWithMetadata) {
            let c = lt(ctx.starts[self.0], ctx.starts[self.1]);
            c.opt_enforce_if(l, ctx);
        }

        fn conj_scope(&self, ctx: &ModelWithMetadata) -> Conjunction {
            let c = lt(ctx.starts[self.0], ctx.starts[self.1]);
            c.conj_scope(ctx)
        }
    }

    #[test]
    fn test_ctx2() {
        let mut store: Model<String> = Model::new();
        let s1 = store.new_ivar(0, 1000, "start1");
        let s2 = store.new_ivar(0, 1000, "start2");
        let s3 = store.new_ivar(0, 1000, "start3");
        let mut model = ModelWithMetadata {
            starts: vec![s1.into(), s2.into(), s3.into()],
            model: store,
        };
        let x = Ordered(1, 2);
        let y: &dyn BoolExpr<_> = &x;

        y.opt_enforce_if(Lit::FALSE, &mut model);

        let e = ReifExpr::And(crate::core::literals::Lits::new());
        e.opt_enforce_if(Lit::TRUE, &mut model);
    }
}
