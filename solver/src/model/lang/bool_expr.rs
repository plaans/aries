use crate::{
    model::lang::{
        expr::{And, Leq, Or, or},
        max::{EqMax, EqMin},
        *,
    },
    prelude::*,
    reif::ReifExpr,
};

/// Representation of a boolean expression, that can be reified, made conditional or enforced
/// in a [`Model`].
pub trait BoolExpr<Ctx: Store> {
    /// Enforce the expression to be true when `implicant` is true and defined.
    ///
    /// IMPORTANT: it must be the case that expression is defined whenever `implicant` is.
    fn enforce_if(&self, implicant: Lit, ctx: &mut Ctx);

    /// Returns a set of literals that must all be true for the expression to be valid.
    /// The list is interpreted as a set: order and redundant elements are ignored.
    ///
    /// Examples:
    ///   - (a < b) would have a conjunctive scope `[prez(a), prez(b)]` as it is only valid when both
    ///     a and b are present. The conjunctive scope is thus the list their presence variable.
    fn conj_scope(&self, ctx: &Ctx) -> Conjunction; // TODO: should be Conjunction

    /// Return a single literal that is true iff all leterals of the conjunctive scope are true.
    fn scope(&self, ctx: &mut Ctx) -> Lit {
        let conj_scope = self.conj_scope(ctx);
        ctx.conjunctive_scope(&conj_scope)
    }

    /// Enforce that if the expression is in scope and `l` is true and defined, then the expression should be true.
    fn opt_enforce_if(&self, l: Lit, ctx: &mut Ctx) {
        let expression_scope = self.scope(ctx);
        let enabler_scope = ctx.presence_literal(l);

        // get the scope of both the expression and the enabler
        let scope = ctx.conjunctive_scope(&[expression_scope, enabler_scope]);

        let implicant = if scope == enabler_scope {
            // l already has the right scope so use it directly
            l // TODO: here we should instead test that scope => prez(l)
        } else if ctx.entails(l) {
            // `l` is always true when defined, so we can use an always true literal in the common scope
            ctx.tautology_of_scope(scope)
        } else {
            // we need to create a new literal that is true whenever it is defined and l is true.
            let imp = ctx.new_literal(scope);
            // if in scope, and l, then imp should be true
            or([!scope, !l, imp]).enforce(ctx);
            imp
        };

        self.enforce_if(implicant, ctx);
    }

    /// Half-reifies the expression, posting a constraint that is
    /// enforces the expression to be true whenever the return literal is.
    fn implicant(&self, ctx: &mut Ctx) -> Lit {
        let scope = self.scope(ctx);
        let implicant = ctx.new_literal(scope);
        self.enforce_if(implicant, ctx);
        implicant
    }

    /// Fully reifies the expression into a literal.
    ///
    /// Note that for this to be possible, it must be possible to build the logical negation of the expression.
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

    /// Enforces that the expression is true whenever it is defined.
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

    //TODO: implement all other methods to make sure we use the most specific implementation
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

#[cfg(test)]
mod test {
    use crate::{
        core::views::{Dom, Term},
        model::{
            Label,
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
