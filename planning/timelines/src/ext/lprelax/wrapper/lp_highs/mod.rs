use std::{collections::HashMap, marker::PhantomData};

use crate::encoder::SchedEncoder;
use crate::ext::lprelax::LpRelaxEncoder;
use crate::ext::lprelax::encoder::problem::{ColTag, LpRelaxProblem, RowExprType};
use crate::ext::lprelax::wrapper::{
    LpRelaxReasonerWrapped, LpRelaxReasonerWrapper, LpRelaxReasonerWrapperTrait, LpRelaxReasonerWrapperTraitInner,
};

use aries_solver::{
    backtrack::{Backtrack, DecLvl},
    core::state::Domains,
    reasoners::Contradiction,
};

use aries_solver_lprelax::{LpCol, LpRelax, int_cst_as_float};

impl LpRelaxReasonerWrapperTrait for LpRelaxReasonerWrapper<LpRelax> {
    fn new_wrapped(ctx: SchedEncoder, num_assumptions: usize) -> LpRelaxReasonerWrapped<Self> {
        let inner_theory = LpRelax::default();

        let wrapper = Self {
            _phantom: PhantomData,
            lprelax_encoder_cached: None,
            ctx,
            num_assumptions,
            encoding_built: false,
            num_events: 0,
            propagation_calls: 0,
        };
        LpRelaxReasonerWrapped::new(wrapper, inner_theory)
    }
}
impl LpRelaxReasonerWrapperTraitInner for LpRelaxReasonerWrapper<LpRelax> {
    type Thr = LpRelax;

    fn pre_propagate(&mut self, theory: &mut Self::Thr, model: &mut Domains) -> Result<(), Contradiction> {
        self.propagation_calls += 1;

        let old_num_events = self.num_events;
        self.num_events = model.trail().num_events();

        // Only proceed if this propagation loop didn't infer anything (quiescence)
        if self.num_events != old_num_events {
            return Ok(());
        }

        // If [we're at quiescence and]:
        // - the encoder hasn't been built
        // - we're still at the root level (before any assumptions)
        // then build the encoder.
        if self.lprelax_encoder_cached.is_none() && model.current_decision_level() == DecLvl::ROOT {
            self.build_encoder(model);
        }

        // If [we're at quiescence and]:
        // - the encoder has been built
        // - the encoding hasn't
        // - we are the (presumed last) assumption level
        // then build the encoding / lp.
        if self.lprelax_encoder_cached.is_some()
            && !self.encoding_built
            && theory.current_decision_level().to_int() as usize >= self.num_assumptions
        {
            self.build_encoding(theory, model);
            self.encoding_built = true;
        }

        Ok(())
    }

    fn post_propagate(&mut self, _theory: &mut Self::Thr, _model: &mut Domains) -> Result<(), Contradiction> {
        Ok(())
    }
}

impl LpRelaxReasonerWrapper<LpRelax> {
    fn build_encoding(&mut self, theory: &mut LpRelax, doms: &Domains) {
        debug_assert!(!self.encoding_built);
        debug_assert!(self.lprelax_encoder_cached.is_some());

        let Some((encoder, pre_assumption_doms)) = self.lprelax_encoder_cached.as_mut() else {
            unreachable!()
        };

        println!(
            "|- Building Lp *encoding* and problem after {} propagation calls (decision level {:?}, num events: {:?}) using model from level {:?}",
            self.propagation_calls,
            doms.current_decision_level(),
            doms.num_events(),
            pre_assumption_doms.current_decision_level(),
        );

        let mut lp_problem = encoder.encode(&self.ctx, Some(&pre_assumption_doms));

        let pre_simplify_rows_len = lp_problem.rows().len();
        lp_problem.simplify(&encoder, &self.ctx, &pre_assumption_doms);

        println!(
            "|- LPrelax problem simplification: {} rows removed",
            pre_simplify_rows_len - lp_problem.rows().len(),
        );
        // TODO build_and_bind_lp(&lp_problem, encoder, &self.ctx, Some(pre_assumption_doms), theory);

        build_and_bind_lp(&lp_problem, &encoder, &self.ctx, &pre_assumption_doms, theory);
    }
}

fn build_and_bind_lp(
    lp_problem: &LpRelaxProblem,
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    doms: &Domains,
    theory: &mut LpRelax,
) {
    let lp_columns = build_lp(lp_problem, theory);
    bind_lp(&lp_columns, encoder, ctx, doms, theory);
}

fn build_lp(problem: &LpRelaxProblem, theory: &mut LpRelax) -> HashMap<ColTag, LpCol> {
    use itertools::Itertools;

    // Add all columns to the LP problem.

    let cols = {
        assert!(problem.sealed());

        let cols = theory.add_columns(&vec![(Some(0.), Some(1.)); problem.cols().unwrap().len()]);

        problem
            .cols()
            .unwrap()
            .iter()
            .zip(cols.iter())
            .map(|(col_tag, col)| (*col_tag, *col))
            .collect::<HashMap<_, _>>()
    };

    // Actually build rows

    let (mut rows_coefs, mut lbs_ubs) = (vec![], vec![]);
    for row_expr in problem.rows() {
        // println!("{row_expr:?}");
        let (row_coefs, lb, ub) = {
            let row_coefs = row_expr
                .lhs()
                .iter()
                .map(|col_tag| (*cols.get(col_tag).unwrap(), 1.))
                .chain(row_expr.rhs().iter().map(|col_tag| (*cols.get(col_tag).unwrap(), -1.)))
                .collect::<Vec<_>>();
            let (lb, ub) = match row_expr.tpe {
                RowExprType::Eq => (
                    Some(int_cst_as_float(row_expr.cst())),
                    Some(int_cst_as_float(row_expr.cst())),
                ),
                RowExprType::Leq => (None, Some(int_cst_as_float(row_expr.cst()))),
                RowExprType::Geq => (Some(int_cst_as_float(row_expr.cst())), None),
            };
            debug_assert!(lb.is_some() || ub.is_some());
            (row_coefs, lb, ub)
        };
        debug_assert!(
            row_coefs.iter().duplicates_by(|&(col_tag, _)| col_tag).next().is_none(),
            "{:?} {:?}",
            rows_coefs.len(),
            row_expr
        );
        if !row_coefs.is_empty() || lb.is_some_and(|l| l > 0.) || ub.is_some_and(|u| u < 0.) {
            rows_coefs.push(row_coefs);
            lbs_ubs.push((lb, ub));
        }
    }
    theory.add_rows(&rows_coefs, &lbs_ubs);

    println!(
        "|- LPrelax problem built: {} columns and {} rows",
        cols.len(),
        rows_coefs.len(),
    );

    cols
}

fn bind_lp(
    columns: &HashMap<ColTag, LpCol>,
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    doms: &Domains,
    theory: &mut LpRelax,
) {
    use aries_solver::core::{IntCst, Lit, Var, views::Term};
    use aries_solver_lprelax::*;
    use itertools::Itertools;

    let presence_lits_and_cols = {
        let mut res = HashMap::<Lit, Vec<LpCol>>::new();

        for (source, transitions) in encoder.iter_sources() {
            for &transition_id in transitions {
                let Some(&col) = columns.get(&ColTag::PresenceTransition(transition_id, None)) else {
                    continue;
                };
                res.entry(encoder.transitions.get_prez(transition_id, ctx))
                    .or_default()
                    .push(col);
            }
            let Some(&col) = columns.get(&ColTag::PresenceSource(source, None)) else {
                continue;
            };
            res.entry(encoder.get_source_prez(source, ctx)).or_default().push(col);
        }
        res
    };

    // Bind lifted presence columns of the LP with corresponding literals in the main CSP.

    for (lit, cols) in presence_lits_and_cols {
        if lit.tautological() {
            for &col in &cols {
                theory.tighten_column(LpCol::from(col), Some(1.), None);
            }
        } else if lit.absurd() {
            for &col in &cols {
                theory.tighten_column(LpCol::from(col), None, Some(0.));
            }
        } else {
            let p = lit.variable();
            debug_assert!(p != Var::ZERO && lit == p.geq(1));

            for &col in &cols {
                theory.add_col_half_binding_default(LpCol::from(col), p);
            }
            theory.add_var_half_binding(
                p,
                std::sync::Arc::new(move |lit_: Lit| {
                    assert_eq!(lit_.variable(), p);
                    cols.iter()
                        .map(|&col| LpLit::from_model_lit(LpCol::from(col), lit_))
                        .collect()
                }),
            );
        }
    }

    // Bind term grounding columns of the LP with corresponding literals in the main CSP.

    for (term, values) in encoder
        .terms_ground
        .iter_sorted_all_only_assignments()
        .chunk_by(|&(term, _)| term)
        .into_iter()
    {
        debug_assert!(!term.is_cst());
        let values = values.collect::<Vec<_>>();

        let var = term.variable();
        assert!(var != Var::ZERO);

        // The recorded values are values of the *term* `a*x + b`, while the CSP literals are
        // on `x`. So `x = (v - b) / a`, and a `v` with a nonzero remainder is unrealizable.
        // Note however that most of the times, `a` will be 1 and `b` 0.
        let factor = term.scaled_var.factor;
        debug_assert!(factor != 0, "a non-constant term shouldn't have a 0 factor");
        const OVERFLOW_MSG: &str = "overflow while computing the variable value of a term grounding";
        let var_value_of = move |v: IntCst| -> Option<IntCst> {
            // `None` means strictly "no integer `x` satisfies `a*x + b == v`". An arithmetic
            // overflow must therefore panic rather than return `None`:
            // the caller pins the column to 0 on `None`,
            // and doing that to a *realizable* value would cut off a feasible
            // solution and make the relaxation wrongly report infeasibility.
            //
            // The checked rem/div can only trip on `INT_CST_MIN` with `factor == -1`,
            // as division by zero is excluded by the `factor != 0` assert above.
            let num = v.checked_sub(term.constant).expect(OVERFLOW_MSG);
            (num.checked_rem(factor).expect(OVERFLOW_MSG) == 0).then(|| num.checked_div(factor).expect(OVERFLOW_MSG))
        };

        let mut bindings: Vec<(LpCol, IntCst)> = Vec::with_capacity(values.len());
        for &(_, v) in &values {
            let Some(&col) = columns.get(&ColTag::TermGround(term, v)) else {
                continue;
            };
            if let Some(var_value) = var_value_of(v) {
                bindings.push((col, var_value));
            } else {
                // No integer assignment of `var` yields the term value `v`.
                theory.tighten_column(col, None, Some(0.));
            }
        }

        let mappings: Vec<(usize, IntCst)> = bindings.iter().map(|&(col, x)| (col.index(), x)).collect();
        theory.add_var_half_binding(
            var,
            std::sync::Arc::new(move |lit: Lit| {
                assert_eq!(lit.variable(), var);
                mappings
                    .iter()
                    .filter_map(|&(col, x)| {
                        (lit.entails(var.lt(x)) || lit.entails(var.gt(x))).then_some(LpLit::leq(LpCol::from(col), 0))
                    })
                    .collect()
            }),
        );

        for (col, x) in bindings {
            theory.add_col_half_binding(
                col,
                std::sync::Arc::new(move |lplit: LpLit| {
                    assert_eq!(lplit.col, col);
                    if lplit.tpe == LpLitType::GEQ && lplit.val == 1 {
                        smallvec::smallvec![var.geq(x), var.leq(x)]
                    } else {
                        Default::default()
                    }
                }),
            );
        }
    }

    // Bind lifted support columns of the LP with corresponding literals in the main CSP.

    for &((out_transition_id, in_transition_id), active) in encoder.supports.unsorted_out() {
        if let Some(s) = active {
            let s = s.variable();
            debug_assert!(s != Var::ZERO);

            let Some(&col) = columns.get(&ColTag::Support(out_transition_id, in_transition_id, None)) else {
                continue;
            };

            theory.add_col_half_binding_default(col, s);
            // The causal link's `active`` literal (whose variable is `s`) is optional and scoped to the in-transition's presence.
            // So it could happen that although `s` is true, it doesn't carry any meaning because it is considered absent (the presence literal is false).
            // As such, in our binding, we can only force the corresponding column to be true when the variable also is AND when the
            // presence is known to be true (statically here, because we cannot access its value dynamically to use in the binding closure).
            // But because in the LP the transitions' presences are not implied by the support column's value being 0,
            // we can always safely derive the column to be 0 when the variable is.
            // Ideally, a reification literal for the conjunction (active /\ prez(active)) should be bound to the column, but we cannot do that here.
            // We could also add a constraint in the model to force `active` to be 0 when `prez(active)` is.
            // This would prevent eager propagation of `active` in the main model but thus could have consequences on its performance.
            let scope_is_fixed = doms.entails(doms.presence(s));
            theory.add_var_half_binding(
                s,
                std::sync::Arc::new(move |lit: Lit| {
                    assert_eq!(lit.variable(), s);
                    let lplit = LpLit::from_model_lit(col, lit);
                    if scope_is_fixed || lplit.tpe == LpLitType::LEQ {
                        smallvec::smallvec![lplit]
                    } else {
                        Default::default()
                    }
                }),
            );
        }
    }
}
