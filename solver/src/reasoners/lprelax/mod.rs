use std::sync::Arc;

use crate::backtrack::{Backtrack, DecLvl, ObsTrailCursor};
use crate::collections::ref_store::RefMap;
use crate::core::state::{Domains, DomainsSnapshot, Event, Explanation, InferenceCause};
use crate::core::{INT_CST_MAX, INT_CST_MIN, IntCst, Lit, VarRef};
use crate::reasoners::{Contradiction, ReasonerId, Theory};

type Float = f64;

pub fn float_as_int_cst(value: Float) -> IntCst {
    if value <= INT_CST_MIN.into() {
        INT_CST_MIN
    } else if value >= INT_CST_MAX.into() {
        INT_CST_MAX
    } else {
        debug_assert!(value.fract().abs() < 1e-9);
        value as IntCst
    }
}

pub type ExplainIisBoundsFn = Arc<dyn Fn(highs::Col, bool, Float) -> Option<Vec<Lit>> + Send + Sync>;
pub type UpdateLpBoundsFn = Arc<dyn Fn(Lit) -> Option<Vec<(highs::Col, bool, Float)>> + Send + Sync>;

pub struct LpNewColumnMetadata {
    var_ref: VarRef,
    explain_iis_bounds: ExplainIisBoundsFn,
    update_lp_bounds: UpdateLpBoundsFn,
}

///
///
/// TODO: Improve highs API:
/// - Change RowProblem and try_optimise such that no conversion to ColProblem needs to take place
/// - Avoid having to clone lp_problem only to move its allocated vectors(' pointers) to the C API. Tackling this naively could result in dangling pointers.
pub struct LpRelax {
    id: ReasonerId,
    model_events: ObsTrailCursor<Event>,
    saved: DecLvl,

    // TODO: INTERNAL `trail` AS IN STN THEORY, TO BE ABLE TO RESTORE BOUNDS
    lp_optim_sense: highs::Sense,
    lp_prob: highs::RowProblem,
    lp_cached_model: highs::Model,

    explain_iis_bounds: Vec<Option<ExplainIisBoundsFn>>,
    update_lp_bounds: RefMap<VarRef, UpdateLpBoundsFn>,
}

unsafe impl Send for LpRelax {}
unsafe impl Sync for LpRelax {}

fn new_lp_model(lp_prob: highs::RowProblem, lp_optim_sense: highs::Sense) -> highs::Model {
    lp_prob.try_optimise(lp_optim_sense).unwrap()
    // let lp_model = lp_prob.try_optimise(sense).unwrap();
    // self.lp_model.set_option("iis_strategy", 0b00110)
    // lp_model
}

fn cache_lp_model(lp_prob: highs::RowProblem, lp_optim_sense: highs::Sense, lp_cached_model: &mut highs::Model) {
    lp_cached_model.overwrite(lp_prob).unwrap();
    lp_cached_model.set_sense(lp_optim_sense);
    // self.lp_model.set_option("iis_strategy", 0b00110)
}

impl Clone for LpRelax {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            model_events: self.model_events.clone(),
            saved: self.saved,
            lp_optim_sense: self.lp_optim_sense,
            lp_prob: self.lp_prob.clone(),
            lp_cached_model: new_lp_model(self.lp_prob.clone(), self.lp_optim_sense),
            explain_iis_bounds: self.explain_iis_bounds.clone(),
            update_lp_bounds: self.update_lp_bounds.clone(),
        }
    }
}

impl Default for LpRelax {
    fn default() -> Self {
        let lp_optim_sense = highs::Sense::Minimise;
        let lp_prob = highs::RowProblem::default();
        let lp_cached_model = new_lp_model(lp_prob.clone(), lp_optim_sense);

        Self {
            id: ReasonerId::LpRelax,
            model_events: ObsTrailCursor::default(),
            saved: DecLvl::default(),
            lp_optim_sense,
            lp_prob,
            lp_cached_model,
            explain_iis_bounds: vec![],
            update_lp_bounds: RefMap::default(),
        }
    }
}

impl LpRelax {
    pub fn add_column<N: Into<Float> + Copy, B: std::ops::RangeBounds<N>>(&mut self, bounds: B) -> highs::Col {
        assert!(self.explain_iis_bounds.len() == self.lp_prob.num_cols());

        self.explain_iis_bounds.push(None);
        self.lp_prob.add_column(0., bounds)
    }

    pub fn update_column_metadata(&mut self, col: highs::Col, metadata: LpNewColumnMetadata) {
        self.explain_iis_bounds
            .insert(col.index(), Some(metadata.explain_iis_bounds));
        self.update_lp_bounds
            .insert(metadata.var_ref, metadata.update_lp_bounds);
    }

    pub fn add_row<
        N: Into<Float> + Copy,
        B: std::ops::RangeBounds<N>,
        ITEM: std::borrow::Borrow<(highs::Col, Float)>,
        I: IntoIterator<Item = ITEM>,
    >(
        &mut self,
        bounds: B,
        row_factors: I,
    ) {
        self.lp_prob.add_row(bounds, row_factors)
    }
}

impl Theory for LpRelax {
    fn identity(&self) -> ReasonerId {
        self.id
    }

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        let mut need_propagation = false;

        // Go through variables' bounds and update LP correspondingly
        while let Some(event) = self.model_events.pop(model.trail()) {
            if let Some(func) = self.update_lp_bounds.get(event.affected_bound.variable())
                && let Some(updates) = func(event.new_literal())
            {
                for (col, upper, bound) in updates {
                    let (old_lb, old_ub) = self.lp_prob.get_column_bounds(col);
                    let bounds = if upper { old_lb..bound } else { bound..old_ub };

                    self.lp_prob.change_column_bounds(col, bounds);

                    need_propagation = true;
                }
            }
        }

        if !need_propagation {
            return Ok(());
        }
        cache_lp_model(self.lp_prob.clone(), self.lp_optim_sense, &mut self.lp_cached_model);

        let Ok(Err(iis)) = self.lp_cached_model.try_solve_mut() else {
            return Ok(());
            // NOTE: bound tightening wouldn't actually be that appropriate to do here / this way...
            // ...because it's not really a "propagation" ! it's more like a bunch of "decisions"...
        };
        // The LP is infeasible: we return a conflict from an (I?)IS of the LP.

        let lits = iis
            .columns()
            .iter()
            .flat_map(|&(col, status)| {
                let (lb, ub) = self.lp_prob.get_column_bounds(col);

                if let Some(func) = &self.explain_iis_bounds[col.index()] {
                    match status {
                        highs::HighsIisBoundStatus::Upper => func(col, true, ub).unwrap_or_default(),
                        highs::HighsIisBoundStatus::Lower => func(col, false, lb).unwrap_or_default(),
                        _ => vec![],
                    }
                } else {
                    vec![]
                }
            })
            .collect();

        Err(Contradiction::Explanation(Explanation { lits }))
    }

    fn explain(
        &mut self,
        _literal: Lit,
        _context: InferenceCause,
        _model: &DomainsSnapshot,
        _out_explanation: &mut Explanation,
    ) {
        unreachable!();
        // since we're not doing bound tightening (yet? and even if we did this wouldn't be the most appropriate place to do it ?),
        // no value is ever updated by this reasoner. it just detects a conflict if there is one and returns it.
    }

    fn print_stats(&self) {
        println!("TODO");
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}

impl Backtrack for LpRelax {
    fn save_state(&mut self) -> DecLvl {
        self.saved += 1;
        self.saved
    }

    fn num_saved(&self) -> u32 {
        self.saved.to_int()
    }

    fn restore_last(&mut self) {
        self.saved -= 1;
    }
}

#[cfg(test)]
pub mod test {
    use std::sync::Arc;

    use crate::core::Relation;
    use crate::core::state::{Cause, Domains, Explanation};
    use crate::reasoners::lprelax::{LpNewColumnMetadata, LpRelax, float_as_int_cst};
    use crate::reasoners::{Contradiction, Theory};

    #[test]
    fn test_highs1() {
        let mut d = Domains::new();

        let a = d.new_var(0, 1);
        let b = d.new_var(0, 1);

        let mut theory = LpRelax::default();

        let ai = theory.add_column(0..1);
        let bi = theory.add_column(0..1);

        theory.add_row(1.., [(ai, 1.), (bi, 1.)]);

        theory.update_column_metadata(
            ai,
            LpNewColumnMetadata {
                var_ref: a,
                explain_iis_bounds: Arc::new(move |col, upper, bound| {
                    assert_eq!(col, ai);
                    if upper {
                        Some(vec![a.leq(float_as_int_cst(bound.ceil()))])
                    } else {
                        Some(vec![a.geq(float_as_int_cst(bound.floor()))])
                    }
                }),
                update_lp_bounds: Arc::new(move |lit| {
                    if lit.variable() == a {
                        match lit.relation() {
                            Relation::Leq => Some(vec![(ai, true, lit.ub_value().into())]),
                            Relation::Gt => Some(vec![(ai, false, lit.not().ub_value().into())]),
                        }
                    } else {
                        None
                    }
                }),
            },
        );
        theory.update_column_metadata(
            bi,
            LpNewColumnMetadata {
                var_ref: b,
                explain_iis_bounds: Arc::new(move |col, upper, bound| {
                    assert_eq!(col, bi);
                    if upper {
                        Some(vec![b.leq(float_as_int_cst(bound.ceil()))])
                    } else {
                        Some(vec![b.geq(float_as_int_cst(bound.floor()))])
                    }
                }),
                update_lp_bounds: Arc::new(move |lit| {
                    if lit.variable() == b {
                        match lit.relation() {
                            Relation::Leq => Some(vec![(bi, true, lit.ub_value().into())]),
                            Relation::Gt => Some(vec![(bi, false, lit.not().ub_value().into())]),
                        }
                    } else {
                        None
                    }
                }),
            },
        );

        let _ = d.set_ub(a, 0, Cause::Decision).unwrap();
        let _ = d.set_ub(b, 0, Cause::Decision).unwrap();

        let expl = match theory.propagate(&mut d) {
            Err(Contradiction::Explanation(expl)) => expl,
            _ => Explanation::new(),
        };

        assert_eq!(expl.literals(), [a.leq(0), b.leq(0)]);
    }
}
