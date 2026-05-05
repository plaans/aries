mod types;

use aries::backtrack::{Backtrack, DecLvl, EventIndex, ObsTrailCursor, Trail};
use aries::core::literals::ConjunctionBuilder;
use aries::core::state::{DomainsSnapshot, Explanation, InferenceCause};
use aries::prelude::{Domains, DomainsExt, IntCst, Lit, VarRef};
use aries::reasoners::{Contradiction, ReasonerId, Theory};

use std::collections::HashMap;

pub use types::*;

#[derive(Debug, Clone)]
struct LpEvent {
    pub new_lplit: LpLit,
    pub prev_lplit: LpLit,
    pub cause: LpEventCause,
}
impl LpEvent {
    pub fn new(new_lplit: LpLit, prev_lplit: LpLit, cause: LpEventCause) -> Self {
        Self {
            new_lplit,
            prev_lplit,
            cause,
        }
    }
}
#[derive(Debug, Clone)]
enum LpEventCause {
    MainModel(Lit),
    ReducedCostStrengthtening(Vec<LpLit>),
}

type ModelEvent = aries::core::state::Event;

#[derive(Copy, Clone)]
struct ModelUpdateCause(EventIndex);

impl From<u32> for ModelUpdateCause {
    fn from(enc: u32) -> Self {
        ModelUpdateCause(enc.into())
    }
}
impl From<ModelUpdateCause> for u32 {
    fn from(cause: ModelUpdateCause) -> Self {
        cause.0.into()
    }
}

#[derive(Default, Clone)]
struct Stats {
    pub lpruns: u64,
    pub lpruns_time: std::time::Duration,
}

#[derive(Clone)]
struct LpRelaxOptim {
    pub col: LpCol,
    pub var: VarRef,
    pub sense: LpOptimSense,
}

#[derive(Default, Clone)]
pub struct LpRelaxConfig {
    no_propagation_skips: bool,
}

/// TODO: Improve highs API:
/// - Change RowProblem and try_optimise such that no conversion to ColProblem needs to take place
/// - Avoid having to clone lpprob only to move its allocated vectors(' pointers) to the C API. Tackling this naively could result in dangling pointers.
pub struct LpRelax {
    id: ReasonerId,

    model_events: ObsTrailCursor<ModelEvent>,
    trail: Trail<LpEvent>,

    lpprob: LpProblem,
    lpmodel_cached: LpModel,
    lpoptim: Option<LpRelaxOptim>,

    lit_implications: HashMap<VarRef, LitImplicationsFn>, //RefMap<VarRef, LitImplicationsFn>,
    lplit_implications: HashMap<LpCol, LpLitImplicationsFn>, //RefMap<usize, LpLitImplicationsFn>,

    stats: Stats,
    config: LpRelaxConfig,
}
unsafe impl Send for LpRelax {}
unsafe impl Sync for LpRelax {}

impl Clone for LpRelax {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            model_events: self.model_events.clone(),
            trail: self.trail.clone(),
            lpprob: self.lpprob.clone(),
            lpmodel_cached: new_lpmodel(self.lpprob.clone(), self.get_sense()),
            lpoptim: self.lpoptim.clone(),
            lit_implications: self.lit_implications.clone(),
            lplit_implications: self.lplit_implications.clone(),
            stats: self.stats.clone(),
            config: self.config.clone(),
        }
    }
}
fn new_lpmodel(lpprob: LpProblem, sense: Option<LpOptimSense>) -> LpModel {
    let mut lpmodel = lpprob.try_optimise(sense.unwrap_or(LpOptimSense::Minimise)).unwrap();

    lpmodel.set_sense(sense.unwrap_or(LpOptimSense::Minimise));

    //lpmodel.set_option("time_limit", 2.0); // stop after 2 seconds
    lpmodel.set_option("parallel", "off"); // use 1 core
    lpmodel.set_option("threads", 1); // solve on 1 thread
    lpmodel.set_option("iis_strategy", 0); // https://github.com/ERGO-Code/HiGHS/blob/3be639f037e0001b617c59830d3965f246ab5beb/highs/interfaces/highs_c_api.h#L153

    lpmodel
}
fn cache_lpmodel(lpprob: LpProblem, sense: Option<LpOptimSense>, obj_col: Option<LpCol>, lpmodel_cached: &mut LpModel) {
    lpmodel_cached.overwrite(lpprob).unwrap();

    lpmodel_cached.set_sense(sense.unwrap_or(LpOptimSense::Minimise));

    /*for col in all_cols {
        lpmodel_cached.change_column_cost(col, 0.);
    }*/
    if let Some(obj_col) = obj_col {
        lpmodel_cached.change_column_cost(obj_col, 1.);
    }
    //lpmodel_cached.set_option("time_limit", 2.0); // stop after 2 seconds
    lpmodel_cached.set_option("parallel", "off"); // use 1 core
    lpmodel_cached.set_option("threads", 1); // solve on 1 thread
    lpmodel_cached.set_option("iis_strategy", 0); // https://github.com/ERGO-Code/HiGHS/blob/3be639f037e0001b617c59830d3965f246ab5beb/highs/interfaces/highs_c_api.h#L153
}
impl Default for LpRelax {
    fn default() -> Self {
        let lpprob = LpProblem::default();
        Self {
            id: ReasonerId::Extra(0),
            model_events: ObsTrailCursor::default(),
            trail: Trail::default(),
            lpprob: lpprob.clone(),
            lpoptim: None,
            lpmodel_cached: new_lpmodel(lpprob, None),
            lit_implications: HashMap::default(),
            lplit_implications: HashMap::default(),
            stats: Stats::default(),
            config: LpRelaxConfig::default(),
        }
    }
}
impl LpRelax {
    pub fn with_config(config: LpRelaxConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    fn num_rows(&self) -> usize {
        self.lpprob.num_rows()
    }

    fn num_columns(&self) -> usize {
        self.lpprob.num_cols()
    }
    #[allow(dead_code)]
    fn columns(&self) -> Vec<LpCol> {
        (0..self.lpprob.num_cols()).map(LpCol::from).collect()
    }

    pub fn get_column_lower_bound(&self, col: LpCol) -> IntCst {
        float_as_exact_int_cst(self.lpprob.get_column_bounds(col).0)
    }
    pub fn get_column_upper_bound(&self, col: LpCol) -> IntCst {
        float_as_exact_int_cst(self.lpprob.get_column_bounds(col).1)
    }

    pub fn add_column_01(&mut self) -> LpCol {
        self.add_column(Some(0.), Some(1.))
    }

    pub fn add_column(&mut self, lb: Option<FloatCst>, ub: Option<FloatCst>) -> LpCol {
        assert!(self.trail.current_decision_level() == DecLvl::ROOT);

        let lb = lb.unwrap_or(FloatCst::MIN);
        let ub = ub.unwrap_or(FloatCst::MAX);
        assert!(lb <= ub);

        self.lpprob.add_column(0., lb..ub)
    }

    pub fn tighten_column(&mut self, col: LpCol, lb: Option<FloatCst>, ub: Option<FloatCst>) -> bool {
        assert!(self.trail.current_decision_level() == DecLvl::ROOT);
        let mut res = false;

        let old_lb = self.get_column_lower_bound(col);
        let lb = if let Some(lb) = lb
            && old_lb < float_as_exact_int_cst(lb)
        {
            res = true;
            float_as_exact_int_cst(lb)
        } else {
            old_lb
        };
        let old_ub = self.get_column_upper_bound(col);
        let ub = if let Some(ub) = ub
            && float_as_exact_int_cst(ub) < old_ub
        {
            res = true;
            float_as_exact_int_cst(ub)
        } else {
            old_ub
        };
        assert!(lb <= ub);
        
        self.lpprob.change_column_bounds(col, lb..ub);
        res
    }

    pub fn add_row<ITEM: std::borrow::Borrow<(LpCol, FloatCst)>, I: IntoIterator<Item = ITEM>>(
        &mut self,
        row_coefs: I,
        lb: Option<FloatCst>,
        ub: Option<FloatCst>,
    ) {
        assert!(self.trail.current_decision_level() == DecLvl::ROOT);

        let lb = lb.unwrap_or(FloatCst::MIN);
        let ub = ub.unwrap_or(FloatCst::MAX);
        self.lpprob.add_row(lb..ub, row_coefs)
    }

    pub fn add_objective_column<I: IntoIterator<Item = (LpCol, FloatCst)>>(
        &mut self,
        var: VarRef,
        coefs: I,
        sense: LpOptimSense,
    ) -> LpCol {
        assert!(self.trail.current_decision_level() == DecLvl::ROOT);
        assert!(self.lpoptim.is_none());

        self.lpoptim = Some(LpRelaxOptim {
            col: self.add_column(None::<FloatCst>, None::<FloatCst>),
            var,
            sense,
        });
        let factors = coefs.into_iter().chain([(self.get_objective_column().unwrap(), -1.)]);

        self.add_row(factors, Some(0.), Some(0.));

        self.get_objective_column().unwrap()
    }

    pub fn get_objective_current_bound(&self) -> Option<IntCst> {
        self.get_sense().map(|sense| match sense {
            LpOptimSense::Maximise => self.get_column_upper_bound(self.get_objective_column().unwrap()),
            LpOptimSense::Minimise => self.get_column_lower_bound(self.get_objective_column().unwrap()),
        })
    }
    pub fn get_objective_column(&self) -> Option<LpCol> {
        self.lpoptim.as_ref().map(|lpoptim| lpoptim.col)
    }
    pub fn get_objective_var(&self) -> Option<VarRef> {
        self.lpoptim.as_ref().map(|lpoptim| lpoptim.var)
    }
    pub fn get_sense(&self) -> Option<LpOptimSense> {
        self.lpoptim.as_ref().map(|lpoptim| lpoptim.sense)
    }

    pub fn set_lit_implications(&mut self, var: VarRef, func: LitImplicationsFn) {
        assert!(self.trail.current_decision_level() == DecLvl::ROOT);

        assert!(self.lit_implications.insert(var, func).is_none());
    }
    pub fn set_lplit_implications(&mut self, col: LpCol, func: LpLitImplicationsFn) {
        assert!(self.trail.current_decision_level() == DecLvl::ROOT);

        assert!(self.lplit_implications.insert(col, func).is_none());
    }
    /*pub fn has_lit_implications(&self, var: VarRef) -> bool {
        self.lit_implications.contains_key(&var)
    }
    pub fn has_lplit_implications(&self, col: LpCol) -> bool {
        self.lplit_implications.contains_key(&col)
    }*/
    fn compute_implied_lplits(&self, lit: Lit) -> Option<impl IntoIterator<Item = LpLit> + use<>> {
        self.lit_implications.get(&lit.variable()).and_then(|func| func(lit))
    }
    fn compute_implied_lits(&self, lplit: LpLit) -> Option<impl IntoIterator<Item = Lit> + use<>> {
        self.lplit_implications.get(&lplit.col).and_then(|func| func(lplit))
    }

    fn set_lplit(&mut self, lplit: LpLit, cause: LpEventCause, model: &mut Domains) -> Result<(), Contradiction> {
        if let (Some(lpevent_index), Some(implied_lits)) = self._set_lplit(lplit, cause) {
            for lit in implied_lits {
                if let Err(invalid) = model.set(lit, self.identity().cause(ModelUpdateCause(lpevent_index))) {
                    return Err(Contradiction::InvalidUpdate(invalid));
                }
            }
        }
        Ok(())
    }
    fn _set_lplit(
        &mut self,
        lplit: LpLit,
        cause: LpEventCause,
    ) -> (Option<EventIndex>, Option<impl IntoIterator<Item = Lit> + use<>>) {
        let prev_lplit = match lplit.tpe {
            LpLitType::LB => LpLit::geq(lplit.col, self.get_column_lower_bound(lplit.col)),
            LpLitType::UB => LpLit::leq(lplit.col, self.get_column_upper_bound(lplit.col)),
        };
        let bounds = match lplit.tpe {
            LpLitType::LB => lplit.val..self.get_column_upper_bound(lplit.col),
            LpLitType::UB => self.get_column_lower_bound(lplit.col)..lplit.val,
        };
        if lplit.strictly_entails(prev_lplit) && bounds.start <= bounds.end {
            self.lpprob.change_column_bounds(lplit.col, bounds);

            let cause_is_main_model = matches!(cause, LpEventCause::MainModel(_));
            let lpevent_index = self.trail.push(LpEvent::new(lplit, prev_lplit, cause));

            if !cause_is_main_model {
                (Some(lpevent_index), self.compute_implied_lits(lplit))
            } else {
                (Some(lpevent_index), None)
            }
        } else {
            (None, None)
        }
    }

    fn set_backtrack_point(&mut self) -> DecLvl {
        self.trail.save_state()
    }
    fn undo_to_last_backtrack_point(&mut self) {
        self.trail.restore_last_with(|ev| {
            let bounds = match ev.new_lplit.tpe {
                LpLitType::LB => {
                    ev.prev_lplit.val..float_as_exact_int_cst(self.lpprob.get_column_bounds(ev.new_lplit.col).1)
                }
                LpLitType::UB => {
                    float_as_exact_int_cst(self.lpprob.get_column_bounds(ev.new_lplit.col).0)..ev.prev_lplit.val
                }
            };
            self.lpprob.change_column_bounds(ev.new_lplit.col, bounds);
        });
    }

    fn process_model_events(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        while let Some(model_event) = self.model_events.pop(model.trail()) {
            let lit = model_event.new_literal();

            // Ignore model events that originate from us (this reasoner),
            // as they were already pushed to our (local) trail.
            if let Some(x) = model_event.cause.as_external_inference()
                && x.writer == self.identity()
            {
                continue;
            }
            if let Some(lplits) = self.compute_implied_lplits(lit) {
                for lplit in lplits.into_iter() {
                    self.set_lplit(lplit, LpEventCause::MainModel(lit), model)?;
                }
            }
        }
        Ok(())
    }

    fn try_solve_cached_lpmodel(&mut self) -> Result<Result<highs::Solution, highs::Iis>, highs::HighsStatus> {
        // TODO: Warm-start with the same basis and basic variables as in solution obtained at a previous decision level.
        //       Indeed, it is still likely to be primal feasible.
        //       No need to keep track of it for backtracking (just don't use warm-starting right after backtracking).

        let time = std::time::Instant::now();

        let res = self.lpmodel_cached.try_solve_mut();

        self.stats.lpruns_time += time.elapsed();
        self.stats.lpruns += 1;

        res
    }

    fn check_feasibility(&mut self) -> Result<(), Contradiction> {
        cache_lpmodel(self.lpprob.clone(), self.get_sense(), None, &mut self.lpmodel_cached);

        match self.try_solve_cached_lpmodel() {
            Ok(Err(iis)) => Err(self.build_contradiction(iis)),
            _ => Ok(()),
        }
    }
    fn propagate_reduced_costs_strengthtening(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        let obj_col = self
            .get_objective_column()
            .expect("Reduced costs strengthtening requires an objective.");

        let Some((optim_obj_val, nz_reduced_costs)) = {
            cache_lpmodel(
                self.lpprob.clone(),
                self.get_sense(),
                Some(obj_col),
                &mut self.lpmodel_cached,
            );

            match self.try_solve_cached_lpmodel() {
                Ok(Ok(sol)) => {
                    let opt_obj_val = sol.columns()[obj_col.index()];
                    let nz_reduced_costs = sol
                        .dual_columns()
                        .iter()
                        .enumerate()
                        .filter(|(col, _)| *col != obj_col.index())
                        .filter_map(|(col, &rc)| (rc != 0.).then_some((LpCol::from(col), rc, sol.columns()[col])))
                        .collect::<Vec<_>>();

                    //debug_assert!(nz_reduced_costs.iter().all(|&(col,_, _)| col != obj_col));
                    Ok(Some((opt_obj_val, nz_reduced_costs)))
                }
                Ok(Err(iis)) => Err(self.build_contradiction(iis)),
                Err(_) => Ok(None), // TODO ? Highs Error ?
            }
        }?
        else {
            return Ok(());
        };

        // Skip if the optimal objective computed above is too close to the incumbent (previous best known objective value).
        let obj_incumbent_bound = self.get_objective_current_bound().unwrap();
        let obj_diff = FloatCst::from(obj_incumbent_bound) - optim_obj_val;
        if !self.config.no_propagation_skips && (obj_diff).abs() < 1. {
            return Ok(());
        }

        let reason = {
            let mut reason = vec![];
            for &(col, rc, _) in &nz_reduced_costs {
                if rc > 0. {
                    reason.push(LpLit::geq(col, self.get_column_lower_bound(col)));
                } else if rc < 0. {
                    reason.push(LpLit::leq(col, self.get_column_upper_bound(col)));
                } else {
                    unreachable!();
                }
            }
            match self.get_sense() {
                Some(LpOptimSense::Minimise) => reason.push(LpLit::geq(obj_col, obj_incumbent_bound)),
                Some(LpOptimSense::Maximise) => reason.push(LpLit::leq(obj_col, obj_incumbent_bound)),
                None => unreachable!(),
            };
            reason
        };

        for (col, rc, val) in nz_reduced_costs {
            let lplit = if rc > 0. {
                LpLit::geq(col, float_as_ceil_int_cst(val + obj_diff / rc))
            } else if rc < 0. {
                LpLit::leq(col, float_as_floor_int_cst(val + obj_diff / rc))
            } else {
                unreachable!();
            };
            self.set_lplit(lplit, LpEventCause::ReducedCostStrengthtening(reason.clone()), model)?;
        }
        self.set_lplit(
            match self.get_sense() {
                Some(LpOptimSense::Minimise) => LpLit::geq(obj_col, float_as_ceil_int_cst(optim_obj_val)),
                Some(LpOptimSense::Maximise) => LpLit::leq(obj_col, float_as_floor_int_cst(optim_obj_val)),
                None => unreachable!(),
            },
            LpEventCause::ReducedCostStrengthtening(reason.clone()),
            model,
        )?;
        Ok(())
    }

    fn build_contradiction(&self, iis: LpIis) -> Contradiction {
        let mut conjunction_builder = ConjunctionBuilder::new();

        for &(col, status) in iis.columns() {
            match status {
                highs::HighsIisBoundStatus::Lower => {
                    let lplit = LpLit::geq(col, float_as_exact_int_cst(self.lpprob.get_column_bounds(col).0));
                    if let Some(lits) = self.compute_implied_lits(lplit) {
                        for lit in lits {
                            conjunction_builder.push(lit);
                        }
                    }
                }
                highs::HighsIisBoundStatus::Upper => {
                    let lplit = LpLit::leq(col, float_as_exact_int_cst(self.lpprob.get_column_bounds(col).1));
                    if let Some(lits) = self.compute_implied_lits(lplit) {
                        for lit in lits {
                            conjunction_builder.push(lit);
                        }
                    }
                }
                highs::HighsIisBoundStatus::Boxed => {
                    // i.e. equal, i.e. both Lower and Upper
                    let lplit_lb = LpLit::geq(col, float_as_exact_int_cst(self.lpprob.get_column_bounds(col).0));
                    if let Some(lits) = self.compute_implied_lits(lplit_lb) {
                        for lit in lits {
                            conjunction_builder.push(lit);
                        }
                    }
                    let lplit_ub = LpLit::leq(col, float_as_exact_int_cst(self.lpprob.get_column_bounds(col).1));
                    if let Some(lits) = self.compute_implied_lits(lplit_ub) {
                        for lit in lits {
                            conjunction_builder.push(lit);
                        }
                    }
                }
                highs::HighsIisBoundStatus::Free => (),
                s => panic!("Unknown highs status {s:?}"),
            }
        }
        let mut expl = Explanation::new();
        expl.extend(conjunction_builder.build());
        Contradiction::Explanation(expl)
    }
}

impl Theory for LpRelax {
    fn identity(&self) -> ReasonerId {
        self.id
    }

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        if !self.config.no_propagation_skips
            && (self.num_columns() == 0 || (self.num_rows() == 0 && self.lpoptim.is_none()))
        {
            return Ok(());
        }

        let processed_model_updates = self.model_events.num_pending(model.trail());
        self.process_model_events(model)?;

        if !self.config.no_propagation_skips && processed_model_updates > 0 {
            return Ok(());
        }
        if !self.config.no_propagation_skips
            && self.trail.saved_states.len() > 1
            && self.trail.saved_states[self.trail.num_saved() as usize - 1] == self.trail.trail.len()
        {
            return Ok(());
        }

        if !self.config.no_propagation_skips && self.current_decision_level() > DecLvl::ROOT {
            return Ok(());
        }
        // TODO: allow propagation after a backtrack.

        if self.lpoptim.is_some() {
            self.propagate_reduced_costs_strengthtening(model)
        } else {
            self.check_feasibility()
        }
    }

    fn explain(
        &mut self,
        literal: Lit,
        context: InferenceCause,
        model: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    ) {
        let mut add_to_explanation = |l: Lit| {
            debug_assert!(model.entails(l), "{:?} {:#?}", l, self.trail.trail);
            out_explanation.push(l);
        };
        debug_assert_eq!(context.writer, self.identity());

        let ModelUpdateCause(lpevent_index) = ModelUpdateCause::from(context.payload);

        debug_assert!(
            self.compute_implied_lits(self.trail.get_event(lpevent_index).new_lplit)
                .map(|lits| lits.into_iter().any(|l| l == literal))
                .unwrap_or_default()
        );

        match &self.trail.get_event(lpevent_index).cause {
            LpEventCause::MainModel(model_lit) => add_to_explanation(*model_lit),
            LpEventCause::ReducedCostStrengthtening(reason) => {
                for &lplit in reason {
                    let lits = self.compute_implied_lits(lplit).unwrap();

                    let mut lits_not_empty = false;
                    for lit in lits {
                        lits_not_empty = true;
                        add_to_explanation(lit);
                    }
                    assert!(
                        lits_not_empty,
                        "any lp lit found here must have imply a non-empty conjunction of model lits"
                    );
                }
            }
        }
    }

    fn print_stats(&self) {
        println!("# lp runs: {}", self.stats.lpruns);
        println!("# lp runs time: {:.6} s", self.stats.lpruns_time.as_secs_f64());
        //println!("# time spent changing column bounds: {} s", self.stats.lp_bounds_change_time.as_secs_f64());
        //println!("# time spent computing implied lits: {:.6} s", self.stats.implications_time.as_secs_f64());
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}

impl Backtrack for LpRelax {
    fn save_state(&mut self) -> DecLvl {
        self.set_backtrack_point()
    }
    fn num_saved(&self) -> u32 {
        self.trail.num_saved()
    }
    fn restore_last(&mut self) {
        self.undo_to_last_backtrack_point();
    }
}

#[cfg(test)]
pub mod test {
    use crate::types::*;
    use crate::{LpRelax, LpRelaxConfig};
    use aries::backtrack::Backtrack;
    use aries::core::IntCst;
    use aries::core::state::Cause;
    use aries::core::state::Explanation;
    use aries::prelude::Domains;
    use aries::reasoners::Contradiction;
    use aries::reasoners::Theory;

    #[test]
    fn test_trail_backtrack() {
        let mut model = Domains::new();

        let var2 = model.new_var(0, 10);
        let var3 = model.new_var(0, 10);

        model.add_implication(var2.leq(5), var3.leq(5));

        let mut theory = LpRelax::with_config(LpRelaxConfig {
            no_propagation_skips: true,
        });

        let col2 = theory.add_column(Some(0.), Some(10.));
        let col3 = theory.add_column(Some(0.), Some(10.));

        theory.set_lit_implications(var2, default_lit_implications(var2, col2));
        theory.set_lplit_implications(col2, default_lplit_implications(var2, col2));

        theory.set_lit_implications(var3, default_lit_implications(var3, col3));
        theory.set_lplit_implications(col3, default_lplit_implications(var3, col3));

        let assert_col_bounds = |theory: &mut LpRelax, col: LpCol, col_bounds: (IntCst, IntCst)| {
            assert_eq!(
                (theory.get_column_lower_bound(col), theory.get_column_upper_bound(col)),
                col_bounds
            )
        };

        assert_col_bounds(&mut theory, col2, (0, 10));
        assert_col_bounds(&mut theory, col3, (0, 10));

        model.save_state();
        theory.save_state();
        assert_eq!(model.set(var2.leq(8), Cause::Decision), Ok(true));
        assert!(theory.propagate(&mut model).is_ok());

        assert_col_bounds(&mut theory, col2, (0, 8));
        assert_col_bounds(&mut theory, col3, (0, 10));

        model.save_state();
        theory.save_state();
        assert_eq!(model.set(var3.leq(8), Cause::Decision), Ok(true));
        assert!(theory.propagate(&mut model).is_ok());

        assert_col_bounds(&mut theory, col2, (0, 8));
        assert_col_bounds(&mut theory, col3, (0, 8));

        model.restore_last();
        theory.restore_last();

        assert_col_bounds(&mut theory, col2, (0, 8));
        assert_col_bounds(&mut theory, col3, (0, 10));

        model.save_state();
        theory.save_state();

        assert_eq!(model.set(var2.leq(5), Cause::Decision), Ok(true));
        assert!(theory.propagate(&mut model).is_ok());

        assert_col_bounds(&mut theory, col2, (0, 5));
        assert_col_bounds(&mut theory, col3, (0, 5));

        model.restore_last();
        theory.restore_last();

        assert_col_bounds(&mut theory, col2, (0, 8));
        assert_col_bounds(&mut theory, col3, (0, 10));

        model.restore_last();
        theory.restore_last();

        assert_col_bounds(&mut theory, col2, (0, 10));
        assert_col_bounds(&mut theory, col3, (0, 10));
    }

    #[test]
    fn test_infeas() {
        let mut model = Domains::new();

        let avar = model.new_var(0, 1);
        let bvar = model.new_var(0, 1);

        let mut theory = LpRelax::with_config(LpRelaxConfig {
            no_propagation_skips: true,
        });

        let acol = theory.add_column(Some(0.), Some(1.));
        let bcol = theory.add_column(Some(0.), Some(1.));

        theory.set_lit_implications(avar, default_lit_implications(avar, acol));
        theory.set_lplit_implications(acol, default_lplit_implications(avar, acol));

        theory.set_lit_implications(bvar, default_lit_implications(bvar, bcol));
        theory.set_lplit_implications(bcol, default_lplit_implications(bvar, bcol));

        theory.add_row([(acol, 1.), (bcol, 1.)], Some(1.), None);

        let _ = model.set_ub(avar, 0, Cause::Decision).unwrap();
        let _ = model.set_ub(bvar, 0, Cause::Decision).unwrap();

        let expl = match theory.propagate(&mut model) {
            Err(Contradiction::Explanation(expl)) => expl,
            _ => Explanation::new(),
        };
        assert_eq!(expl.literals(), [avar.leq(0), bvar.leq(0)]);
    }
}
