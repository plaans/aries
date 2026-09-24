use std::sync::Arc;
use std::{collections::HashMap, marker::PhantomData};

use crate::constraints::lprelax::LpRelaxEncoder;
use crate::constraints::lprelax::encoder::problem::{ColTag, LpRelaxProblem, RowExprType};
use crate::constraints::lprelax::wrapper::{
    LpRelaxReasonerWrapped, LpRelaxReasonerWrapper, LpRelaxReasonerWrapperTrait, LpRelaxReasonerWrapperTraitInner,
};
use crate::encoder::SchedEncoder;

use aries_solver::{
    backtrack::{Backtrack, DecLvl},
    core::state::Domains,
    reasoners::Contradiction,
};

use aries_solver_lprelax::{LpCol, LpRelax};

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
        let time = std::time::Instant::now();

        let mut lp_problem = encoder.encode(&self.ctx, Some(pre_assumption_doms));

        lp_problem.simplify(
            encoder,
            &self.ctx,
            pre_assumption_doms,
            super::super::ARIES_LPRELAX_MERGE_EQUAL_COLUMNS.get(),
        );

        build_and_bind_lp(&lp_problem, encoder, &self.ctx, pre_assumption_doms, theory);

        println!(
            "|-[LPRELAX]- Built LP *encoding* and problem after {} propagation calls (decision level {:?}, num events: {:?}) using model from level {:?} in {}s",
            self.propagation_calls,
            doms.current_decision_level(),
            doms.num_events(),
            pre_assumption_doms.current_decision_level(),
            time.elapsed().as_secs_f64(),
        );
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
        let cols = theory.add_columns(&vec![(Some(0), Some(1)); problem.cols().unwrap().len()]);

        problem
            .cols()
            .unwrap()
            .iter()
            .zip(cols.iter())
            .map(|(col_tag, col)| (*col_tag, *col))
            .collect::<HashMap<_, _>>()
    };

    // Actually build rows

    let (mut rows_coefs, mut bounds) = (vec![], vec![]);
    for row_expr in problem.rows() {
        // println!("{row_expr:?}");
        let (row_coefs, lb, ub) = {
            let row_coefs = row_expr
                .lhs()
                .iter()
                .map(|(coef, col_tag)| (*cols.get(col_tag).unwrap(), *coef))
                .chain(
                    row_expr
                        .rhs()
                        .iter()
                        .map(|(coef, col_tag)| (*cols.get(col_tag).unwrap(), -*coef)),
                )
                .collect::<Vec<_>>();
            let (lb, ub) = match row_expr.tpe {
                RowExprType::Eq => (Some(row_expr.cst()), Some(row_expr.cst())),
                RowExprType::Leq => (None, Some(row_expr.cst())),
                RowExprType::Geq => (Some(row_expr.cst()), None),
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
        debug_assert!(!row_coefs.is_empty() || lb.is_some_and(|l| l > 0) || ub.is_some_and(|u| u < 0));

        rows_coefs.push(row_coefs);
        bounds.push((lb, ub));
    }
    theory.add_rows(rows_coefs.into_iter(), &bounds);

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
            for &trans_id in transitions {
                let Some(&col) = columns.get(&ColTag::PresenceTransition(trans_id, None)) else {
                    continue;
                };
                res.entry(encoder.transitions.get_prez(trans_id, ctx))
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
                theory.tighten_column(LpCol::from(col), (Some(1), None));
            }
        } else if lit.absurd() {
            for &col in &cols {
                theory.tighten_column(LpCol::from(col), (None, Some(0)));
            }
        } else {
            let p = lit.variable();
            debug_assert!(p != Var::ZERO && lit == p.geq(1));

            for &col in &cols {
                theory.add_binding(
                    doms.presence(p),
                    AriesSignedVar::minus(p),
                    LpCol::from(col),
                    Arc::new(|v| (v == 1).then_some((LpLitType::GEQ, 1))),
                );
                theory.add_binding(
                    Lit::TRUE,
                    AriesSignedVar::plus(p),
                    LpCol::from(col),
                    Arc::new(|v| (v == 0).then_some((LpLitType::LEQ, 0))),
                );
            }
        }
    }

    // Bind term grounding columns of the LP with corresponding literals in the main CSP.

    for (term, values) in encoder
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
                theory.tighten_column(col, (None, Some(0)));
            }
        }

        for (col, x) in bindings {
            theory.add_binding(
                Lit::TRUE,
                AriesSignedVar::minus(var),
                LpCol::from(col),
                Arc::new(move |v| (v > x).then_some((LpLitType::LEQ, 0))),
            );
            theory.add_binding(
                Lit::TRUE,
                AriesSignedVar::plus(var),
                LpCol::from(col),
                Arc::new(move |v| (v < x).then_some((LpLitType::LEQ, 0))),
            );
        }
    }

    // Bind lifted support columns of the LP with corresponding literals in the main CSP.

    for &((out_trans_id, in_trans_id), active) in encoder.supports.unsorted_out() {
        if let Some(s) = active {
            let Some(&col) = columns.get(&ColTag::Support(out_trans_id, in_trans_id, None)) else {
                continue;
            };
            let s = s.variable();
            assert!(s != Var::ZERO);

            theory.add_binding(
                Lit::TRUE,
                AriesSignedVar::plus(s.variable()),
                LpCol::from(col),
                Arc::new(|v| Some((LpLitType::LEQ, v))),
            );

            // WARNING NOTE !
            // The following code snippet half-binds the causal link's activation literal to its corresponding column,
            // such that when this literal is present and set to true, the column's lower bound is set to 1.
            // However, it is deactivated because it is UNSOUND in the case where conditions can be used as out-transitions,
            // as that case forbids the LP relaxation from having an effect support 2 or more conditions, even though it is allowed in the main model.
            // As such, this binding could force two support columns two 1, making their sum equal to 2,
            // while this very sum would be constrained to be <= 1 by the lp relaxation (with conditions allowed to be out-transitions),
            // which was contradictory.
            //
            // // theory.add_binding(
            // //     doms.presence(s),
            // //     AriesSignedVar::plus(s.variable()),
            // //     LpCol::from(col),
            // //     Arc::new(|v| Some((LpLitType::GEQ, v))),
            // // );
        }
    }
}
