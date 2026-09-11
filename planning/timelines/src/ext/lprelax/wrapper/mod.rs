mod traits;

use std::{collections::HashMap, marker::PhantomData};

use crate::{
    encoder::SchedEncoder,
    ext::lprelax::{
        LpRelaxEncoder,
        encoding::{LpRelaxEncoding, LpRelaxProblem},
    },
};
use aries_solver::{
    backtrack::{Backtrack, DecLvl},
    core::{state::Domains, views::Dom},
    reasoners::Contradiction,
};

use aries_solver_lprelax::LpCol;
use traits::LpRelaxReasonerWrapperTraitInner;
pub(crate) use traits::{LpRelaxReasonerWrapped, LpRelaxReasonerWrapperTrait};

#[derive(Clone)]
pub struct LpRelaxReasonerWrapper<T: aries_solver::reasoners::Theory> {
    _phantom: PhantomData<fn() -> T>,

    encoder: LpRelaxEncoder,
    ctx: SchedEncoder,

    built: bool,
    num_events: u32,
    propagation_calls: usize,
    // relations: crate::ext::lprelax::LpRelaxEncodingRelations,
    // col_tags: std::collections::HashSet<crate::ext::lprelax::ColTag>,
    // row_exprs: Vec<crate::ext::lprelax::RowExpr>,
}

impl LpRelaxReasonerWrapperTrait for LpRelaxReasonerWrapper<aries_solver_lprelax::LpRelax> {
    fn new_wrapped(encoder: LpRelaxEncoder, ctx: SchedEncoder) -> LpRelaxReasonerWrapped<Self> {
        let /*mut*/ inner_theory = aries_solver_lprelax::LpRelax::default();

        // let mut enc = super::LpRelaxEncoding::default();
        // enc.encode(ctx, &mut inner_theory);
        // enc.print_encoding(ctx);
        // enc.print_stats();

        // let relations = super::LpRelaxEncodingRelations::from(&encoder.main, &encoder.ctx);
        // let (col_tags, row_exprs) = relations.build_col_tags_and_row_exprs(&encoder.main, &encoder.ctx);

        let wrapper = Self {
            _phantom: PhantomData,
            encoder,
            ctx,
            built: false,
            num_events: 0,
            propagation_calls: 0,
            // relations: Default::default(),
            // col_tags: Default::default(),
            // row_exprs: Default::default(),
        };

        LpRelaxReasonerWrapped::new(wrapper, inner_theory)
    }
}
impl LpRelaxReasonerWrapperTraitInner for LpRelaxReasonerWrapper<aries_solver_lprelax::LpRelax> {
    type Thr = aries_solver_lprelax::LpRelax;

    fn pre_propagate(&mut self, theory: &mut Self::Thr, model: &mut Domains) -> Result<(), Contradiction> {
        if self.built {
            return Ok(());
        }
        self.propagation_calls += 1;

        let old_num_events = self.num_events;
        self.num_events = model.trail().num_events();

        if !self.built && old_num_events == self.num_events && model.current_decision_level() == DecLvl::ROOT {
            println!("|- Building lp after {} propagation calls", self.propagation_calls);

            let (encoding, lp_problem) = self.encoder.encode(&self.ctx);

            // for row in &lp_problem.rows {
            //     println!("{row:?}");
            // }

            let columns = build_lp(&lp_problem, theory);
            bind_lp(&columns, &encoding, &self.encoder, &self.ctx, theory);

            /*let t0 = std::time::Instant::now();

            self.relations = super::LpRelaxEncodingRelations::from(&self.encoder, &self.ctx);
            (self.col_tags, self.row_exprs) = self.relations.build_col_tags_and_row_exprs(&self.encoder, &self.ctx);

            let mut enc = super::LpRelaxEncoding::default();

            enc.stats.relations_time = t0.elapsed();
            let t1 = std::time::Instant::now();

            // TODO: simplify row_exprs, replacing row_exprs' terms whose values are now known with constants, and removing / ignoring the row if no variables remain in it.
            // ALSO TODO: do the encoding / relations gathering at this point (would be great if possible)
            enc._build_lp(self.col_tags.clone(), self.row_exprs.clone(), theory);
            enc._bind_lp_to_main_model(&self.relations, theory);

            enc.stats.build_lp_time = t1.elapsed();

            enc.stats.total_time = t0.elapsed();

            enc.print_stats();*/

            self.built = true;
        }

        Ok(())
    }

    fn post_propagate(&mut self, _theory: &mut Self::Thr, _model: &mut Domains) -> Result<(), Contradiction> {
        Ok(())
    }
}

fn build_lp(
    problem: &LpRelaxProblem,
    /*dom: impl aries_solver::core::views::Dom,*/ theory: &mut aries_solver_lprelax::LpRelax,
) -> HashMap<super::ColTag, LpCol> {
    use super::ColTag;
    use crate::ext::lprelax::RowExpr;
    use aries_solver_lprelax::LpCol;
    use itertools::Itertools;

    // Add all columns to the LP problem.

    let cols: HashMap<ColTag, LpCol> = theory
        .add_columns(&vec![(Some(0.), Some(1.)); problem.cols().len()])
        .into_iter()
        .zip(problem.cols())
        .map(|(col, col_tag)| (col_tag.clone(), col))
        .collect();
    let rows = problem.rows();

    // Add all rows to the LP problem.

    let (mut rows_coefs, mut lbs_ubs) = (vec![], vec![]);
    for row_expr in rows {
        // println!("{row_expr:?}");
        let (row_coefs, lb, ub) = match row_expr {
            RowExpr::Eq(lhs, rhs) => (
                lhs.iter()
                    .map(|col_tag| (*cols.get(col_tag).unwrap(), 1.))
                    .chain(rhs.iter().map(|col_tag| (*cols.get(col_tag).unwrap(), -1.)))
                    .collect_vec(),
                Some(0.),
                Some(0.),
            ),
            RowExpr::Geq(lhs, rhs) => (
                lhs.iter()
                    .map(|col_tag| (*cols.get(col_tag).unwrap(), 1.))
                    .chain(rhs.iter().map(|col_tag| (*cols.get(col_tag).unwrap(), -1.)))
                    .collect_vec(),
                Some(0.),
                None,
            ),
            RowExpr::Leq(lhs, rhs) => (
                lhs.iter()
                    .map(|col_tag| (*cols.get(col_tag).unwrap(), 1.))
                    .chain(rhs.iter().map(|col_tag| (*cols.get(col_tag).unwrap(), -1.)))
                    .collect_vec(),
                None,
                Some(0.),
            ),
            RowExpr::Leq1(lhs) => (
                lhs.iter()
                    .map(|col_tag| (*cols.get(col_tag).unwrap(), 1.))
                    .collect_vec(),
                None,
                Some(1.),
            ),
        };
        debug_assert!(
            row_coefs.iter().duplicates_by(|&(col_tag, _)| col_tag).next().is_none(),
            "{:?} {:?}",
            rows_coefs.len(),
            row_expr
        );

        rows_coefs.push(row_coefs);
        lbs_ubs.push((lb, ub));
    }
    theory.add_rows(&rows_coefs, &lbs_ubs);

    cols
}

fn bind_lp(
    columns: &HashMap<super::ColTag, LpCol>,
    encoding: &LpRelaxEncoding,
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    theory: &mut aries_solver_lprelax::LpRelax,
) {
    use crate::ext::lprelax::ColTag;
    use aries_solver::core::{IntCst, Lit, Var, views::Term};
    use aries_solver_lprelax::*;
    use itertools::Itertools;

    let presence_lits_and_cols = {
        let mut res = HashMap::<Lit, Vec<LpCol>>::new();

        for source in encoder.iter_sources(ctx) {
            let source_prez = encoder.get_source(source, ctx).map_or(Lit::TRUE, |task| task.presence);
            res.entry(source_prez)
                .or_default()
                .push(*columns.get(&ColTag::PresenceSource(source)).unwrap());
        }
        for (transition_id, _) in encoder.iter_transitions() {
            let transition_prez = encoder.transitions.get_prez(transition_id, ctx);
            res.entry(transition_prez)
                .or_default()
                .push(*columns.get(&ColTag::PresenceTransition(transition_id)).unwrap());
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

    for (term, values) in encoding
        .iter_terms_assignments()
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
            let col = *columns.get(&ColTag::TermGround(term, v)).unwrap();
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

    for ((out_transition_id, in_transition_id), active) in encoder.iter_supports(ctx) {
        if let Some(s) = active {
            let s = s.variable();
            debug_assert!(s != Var::ZERO);

            let col = *columns
                .get(&ColTag::Support(out_transition_id, in_transition_id))
                .unwrap();

            theory.add_var_half_binding_default(s, col);
            theory.add_col_half_binding_default(col, s);
        }
    }
}
