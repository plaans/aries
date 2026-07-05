mod bindings;
mod lplit;
mod types;

use aries_solver::backtrack::{Backtrack, DecLvl, EventIndex, ObsTrailCursor, Trail};
use aries_solver::core::literals::ConjunctionBuilder;
use aries_solver::core::state::{DomainsSnapshot, Explanation, InferenceCause};
use aries_solver::prelude::{Domains, DomainsExt, INT_CST_MAX, INT_CST_MIN, IntCst, Lit, Var};
use aries_solver::reasoners::{Contradiction, ReasonerId, Theory};

use bindings::LpRelaxBindings;
pub use bindings::{LitToLpLitsBindingFn, LpLitToLitsBindingFn};
pub use lplit::*;
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

type ModelEvent = aries_solver::core::state::Event;

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
struct LpRelaxStats {
    pub lpruns: u64,
    pub lpruns_time: std::time::Duration,
}

#[derive(Clone)]
pub struct LpRelaxConfig {
    use_propagation_skips: bool,
}
impl Default for LpRelaxConfig {
    fn default() -> Self {
        Self {
            use_propagation_skips: true,
        }
    }
}

#[derive(Clone)]
struct LpRelaxObjective {
    pub col: LpCol,
    pub var: Var,
    pub sense: LpObjectiveSense,
}

struct LpRelaxState {
    model_events: ObsTrailCursor<ModelEvent>,
    trail: Trail<LpEvent>,

    lpmodel: LpModel,
    lpobjective: Option<LpRelaxObjective>,
}
impl Clone for LpRelaxState {
    fn clone(&self) -> Self {
        let mut lpmodel = self.lpmodel.clone();
        set_lpmodel_options(&mut lpmodel);

        Self {
            model_events: self.model_events.clone(),
            trail: self.trail.clone(),
            lpmodel,
            lpobjective: self.lpobjective.clone(),
        }
    }
}
fn set_lpmodel_options(lpmodel: &mut LpModel) {
    //lpmodel.set_option("time_limit", 2.0); // stop after 2 seconds
    lpmodel.set_option("parallel", "off"); // use 1 core
    lpmodel.set_option("threads", 1); // solve on 1 thread
    lpmodel.set_option("iis_strategy", 0); // https://github.com/ERGO-Code/HiGHS/blob/3be639f037e0001b617c59830d3965f246ab5beb/highs/interfaces/highs_c_api.h#L153
}
impl Default for LpRelaxState {
    fn default() -> Self {
        let mut lpmodel = LpModel::default(LpObjectiveSense::Minimise);
        set_lpmodel_options(&mut lpmodel);

        Self {
            model_events: ObsTrailCursor::default(),
            trail: Trail::default(),
            lpmodel,
            lpobjective: None,
        }
    }
}
impl LpRelaxState {
    fn num_rows(&self) -> usize {
        self.lpmodel.num_rows()
    }
    fn num_columns(&self) -> usize {
        self.lpmodel.num_columns()
    }
    #[allow(dead_code)]
    fn columns(&self) -> Vec<LpCol> {
        (0..self.lpmodel.num_columns()).map(LpCol::from).collect()
    }

    fn get_column_lower_bound(&self, col: LpCol) -> IntCst {
        float_as_exact_int_cst(self.lpmodel.get_column_bounds(col).0)
    }
    fn get_column_upper_bound(&self, col: LpCol) -> IntCst {
        float_as_exact_int_cst(self.lpmodel.get_column_bounds(col).1)
    }
    fn add_column(&mut self, lb: Option<FloatCst>, ub: Option<FloatCst>) -> LpCol {
        assert!(self.trail.current_decision_level() == DecLvl::ROOT);

        let lb = lb.unwrap_or(FloatCst::MIN);
        let ub = ub.unwrap_or(FloatCst::MAX);
        assert!(lb <= ub);

        self.lpmodel.add_column(0., lb..ub, []).unwrap()
    }
    fn add_columns(&mut self, lbs_ubs: &[(Option<FloatCst>, Option<FloatCst>)]) -> Vec<LpCol> {
        assert!(self.trail.current_decision_level() == DecLvl::ROOT);

        let lbs_ubs = Vec::from_iter(
            lbs_ubs
                .iter()
                .map(|(lb, ub)| (lb.unwrap_or(FloatCst::MIN), ub.unwrap_or(FloatCst::MAX))),
        );
        self.lpmodel.add_columns(&lbs_ubs).unwrap()
    }
    fn tighten_column(&mut self, col: LpCol, lb: Option<FloatCst>, ub: Option<FloatCst>) -> bool {
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

        self.lpmodel.change_column_bounds(col, lb..ub);
        res
    }
    fn change_column(&mut self, col: LpCol, lb: Option<FloatCst>, ub: Option<FloatCst>) {
        assert!(self.trail.current_decision_level() == DecLvl::ROOT);
        let lb = if let Some(lb) = lb {
            float_as_exact_int_cst(lb)
        } else {
            INT_CST_MIN
        };
        let ub = if let Some(ub) = ub {
            float_as_exact_int_cst(ub)
        } else {
            INT_CST_MAX
        };
        assert!(lb <= ub);

        self.lpmodel.change_column_bounds(col, lb..ub);
    }
    fn add_row(
        &mut self,
        row_coefs: impl Iterator<Item = (LpCol, FloatCst)>,
        lb: Option<FloatCst>,
        ub: Option<FloatCst>,
    ) -> LpRow {
        assert!(self.trail.current_decision_level() == DecLvl::ROOT);

        let lb = lb.unwrap_or(FloatCst::MIN);
        let ub: f64 = ub.unwrap_or(FloatCst::MAX);
        let num_columns = self.num_columns();
        self.lpmodel
            .add_row(
                lb..ub,
                row_coefs.inspect(|(col, _)| debug_assert!(col.index() < num_columns)),
            )
            .unwrap()
    }
    fn add_rows(
        &mut self,
        rows_coefs: &[Vec<(LpCol, FloatCst)>],
        lbs_ubs: &[(Option<FloatCst>, Option<FloatCst>)],
    ) -> Vec<LpRow> {
        assert!(self.trail.current_decision_level() == DecLvl::ROOT);
        debug_assert!(
            rows_coefs
                .iter()
                .all(|row_coefs| row_coefs.iter().all(|(col, _)| col.index() < self.num_columns()))
        );

        let lbs_ubs: Vec<_> = lbs_ubs
            .iter()
            .map(|(lb, ub)| (lb.unwrap_or(FloatCst::MIN), ub.unwrap_or(FloatCst::MAX)))
            .collect();

        self.lpmodel.add_rows(&lbs_ubs, rows_coefs).unwrap()
    }

    fn add_objective_column(
        &mut self,
        var: Var,
        coefs: impl Iterator<Item = (LpCol, FloatCst)>,
        sense: LpObjectiveSense,
    ) -> LpCol {
        assert!(self.trail.current_decision_level() == DecLvl::ROOT);
        assert!(self.lpobjective.is_none());

        self.lpobjective = Some(LpRelaxObjective {
            col: self.add_column(None::<FloatCst>, None::<FloatCst>),
            var,
            sense,
        });
        self.lpmodel
            .change_column_cost(self.get_objective_column().unwrap(), 1.);

        let factors = coefs
            .into_iter()
            .chain([(self.get_objective_column().unwrap(), -1.)])
            .collect::<Vec<_>>();

        self.add_row(factors.into_iter(), Some(0.), Some(0.));
        self.lpmodel.set_sense(sense);

        self.get_objective_column().unwrap()
    }

    fn get_objective_current_bound(&self) -> Option<IntCst> {
        self.get_objective_sense().map(|sense| match sense {
            LpObjectiveSense::Maximise => self.get_column_upper_bound(self.get_objective_column().unwrap()),
            LpObjectiveSense::Minimise => self.get_column_lower_bound(self.get_objective_column().unwrap()),
        })
    }
    fn get_objective_column(&self) -> Option<LpCol> {
        self.lpobjective.as_ref().map(|lpoptim| lpoptim.col)
    }
    fn get_objective_var(&self) -> Option<Var> {
        self.lpobjective.as_ref().map(|lpoptim| lpoptim.var)
    }
    fn get_objective_sense(&self) -> Option<LpObjectiveSense> {
        self.lpobjective.as_ref().map(|lpoptim| lpoptim.sense)
    }

    fn set_lplit(
        &mut self,
        lplit: LpLit,
        cause: LpEventCause,
        model: &mut Domains,
        identity: ReasonerId,
        bindings: &LpRelaxBindings,
    ) -> Result<(), Contradiction> {
        debug_assert!(identity == ReasonerId::Extra(0));

        let (lp_event_index, implied_lits) = {
            let prev_lplit = match lplit.tpe {
                LpLitType::GEQ => LpLit::geq(lplit.col, self.get_column_lower_bound(lplit.col)),
                LpLitType::LEQ => LpLit::leq(lplit.col, self.get_column_upper_bound(lplit.col)),
            };
            let bounds = match lplit.tpe {
                LpLitType::GEQ => lplit.val..self.get_column_upper_bound(lplit.col),
                LpLitType::LEQ => self.get_column_lower_bound(lplit.col)..lplit.val,
            };
            if lplit.strictly_entails(prev_lplit) && bounds.start <= bounds.end {
                self.lpmodel.change_column_bounds(lplit.col, bounds);

                let cause_is_main_model = matches!(cause, LpEventCause::MainModel(_));
                let lpevent_index = self.trail.push(LpEvent::new(lplit, prev_lplit, cause));

                if !cause_is_main_model {
                    (Some(lpevent_index), bindings.compute_implied_lits(lplit))
                } else {
                    (Some(lpevent_index), None)
                }
            } else {
                (None, None)
            }
        };

        if let (Some(lpevent_index), Some(implied_lits)) = (lp_event_index, implied_lits) {
            for lit in implied_lits {
                if let Err(invalid) = model.set(lit, identity.cause(ModelUpdateCause(lpevent_index))) {
                    return Err(Contradiction::InvalidUpdate(invalid));
                }
            }
        }
        Ok(())
    }
    fn set_backtrack_point(&mut self) -> DecLvl {
        self.trail.save_state()
    }
    fn undo_to_last_backtrack_point(&mut self) {
        self.lpmodel.clear_solver().unwrap();

        self.trail.restore_last_with(|ev| {
            let bounds = match ev.new_lplit.tpe {
                LpLitType::GEQ => {
                    ev.prev_lplit.val..float_as_exact_int_cst(self.lpmodel.get_column_bounds(ev.new_lplit.col).1)
                }
                LpLitType::LEQ => {
                    float_as_exact_int_cst(self.lpmodel.get_column_bounds(ev.new_lplit.col).0)..ev.prev_lplit.val
                }
            };
            self.lpmodel.change_column_bounds(ev.new_lplit.col, bounds);
        });
    }
    fn solve_or_iis(&mut self, stats: &mut LpRelaxStats) -> Result<Result<LpSolution, LpIis>, highs::HighsStatus> {
        let time = std::time::Instant::now();

        let res = self.lpmodel.solve_or_iis();

        if res.is_err() {
            self.lpmodel.clear_solver().unwrap();
        }

        stats.lpruns_time += time.elapsed();
        stats.lpruns += 1;

        res
    }
}

/// TODO: Improve highs API:
/// - Change RowProblem and try_optimise such that no conversion to ColProblem needs to take place
/// - Avoid having to clone lpprob only to move its allocated vectors(' pointers) to the C API. Tackling this naively could result in dangling pointers.
#[derive(Clone)]
pub struct LpRelax {
    id: ReasonerId,

    state: LpRelaxState,
    bindings: LpRelaxBindings,

    //prev_propagation_attempt_trail_info: (DecLvl, u32, bool),
    stats: LpRelaxStats,
    config: LpRelaxConfig,
    // num_propagation_call: usize,
}
unsafe impl Send for LpRelax {}
unsafe impl Sync for LpRelax {}

impl Default for LpRelax {
    fn default() -> Self {
        Self {
            id: ReasonerId::Extra(0),
            state: LpRelaxState::default(),
            bindings: LpRelaxBindings::default(),
            //prev_propagation_attempt_trail_info: (DecLvl::ROOT, 0, false),
            stats: LpRelaxStats::default(),
            config: LpRelaxConfig::default(),
            // num_propagation_call: 0,
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
    pub fn num_rows(&self) -> usize {
        self.state.num_rows()
    }
    pub fn num_columns(&self) -> usize {
        self.state.num_columns()
    }
    pub fn get_column_lower_bound(&self, col: LpCol) -> IntCst {
        self.state.get_column_lower_bound(col)
    }
    pub fn get_column_upper_bound(&self, col: LpCol) -> IntCst {
        self.state.get_column_upper_bound(col)
    }

    pub fn add_column_01(&mut self) -> LpCol {
        self.add_column(Some(0.), Some(1.))
    }

    pub fn add_column(&mut self, lb: Option<FloatCst>, ub: Option<FloatCst>) -> LpCol {
        self.state.add_column(lb, ub)
    }

    pub fn add_columns(&mut self, lbs_ubs: &[(Option<FloatCst>, Option<FloatCst>)]) -> Vec<LpCol> {
        self.state.add_columns(lbs_ubs)
    }

    pub fn tighten_column(&mut self, col: LpCol, lb: Option<FloatCst>, ub: Option<FloatCst>) -> bool {
        self.state.tighten_column(col, lb, ub)
    }
    pub fn change_column(&mut self, col: LpCol, lb: Option<FloatCst>, ub: Option<FloatCst>) {
        self.state.change_column(col, lb, ub)
    }

    pub fn add_row(
        &mut self,
        row_coefs: impl Iterator<Item = (LpCol, FloatCst)>,
        lb: Option<FloatCst>,
        ub: Option<FloatCst>,
    ) -> LpRow {
        self.state.add_row(row_coefs, lb, ub)
    }

    pub fn add_rows(
        &mut self,
        rows_coefs: &[Vec<(LpCol, FloatCst)>],
        lbs_ubs: &[(Option<FloatCst>, Option<FloatCst>)],
    ) -> Vec<LpRow> {
        self.state.add_rows(rows_coefs, lbs_ubs)
    }

    pub fn add_objective_column(
        &mut self,
        var: Var,
        coefs: impl Iterator<Item = (LpCol, FloatCst)>,
        sense: LpObjectiveSense,
    ) -> LpCol {
        self.state.add_objective_column(var, coefs, sense)
    }

    pub fn get_objective_column(&self) -> Option<LpCol> {
        self.state.get_objective_column()
    }
    pub fn get_objective_var(&self) -> Option<Var> {
        self.state.get_objective_var()
    }
    pub fn get_objective_sense(&self) -> Option<LpObjectiveSense> {
        self.state.get_objective_sense()
    }

    pub fn add_var_half_binding(&mut self, var: Var, func: std::sync::Arc<LitToLpLitsBindingFn>) {
        assert!(self.state.trail.current_decision_level() == DecLvl::ROOT);
        self.bindings.add_lit_to_lplits_binding(var, func);
    }
    pub fn add_col_half_binding(&mut self, col: LpCol, func: std::sync::Arc<LpLitToLitsBindingFn>) {
        assert!(self.state.trail.current_decision_level() == DecLvl::ROOT);
        self.bindings.add_lplit_to_lits_binding(col, func);
    }
    pub fn add_var_half_binding_default(&mut self, var: Var, col: LpCol) {
        assert!(self.state.trail.current_decision_level() == DecLvl::ROOT);
        self.bindings.add_lit_to_lplits_binding_default(var, col);
    }
    pub fn add_col_half_binding_default(&mut self, col: LpCol, var: Var) {
        assert!(self.state.trail.current_decision_level() == DecLvl::ROOT);
        self.bindings.add_lplit_to_lits_binding_default(var, col);
    }

    fn process_model_events(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        while let Some(model_event) = self.state.model_events.pop(model.trail()) {
            let lit = model_event.new_literal();

            // Ignore model events that originate from us (this reasoner),
            // as they were already pushed to our (local) trail.
            if let Some(x) = model_event.cause.as_external_inference()
                && x.writer == self.identity()
            {
                continue;
            }
            if let Some(lplits) = self.bindings.compute_implied_lplits(lit) {
                for lplit in lplits {
                    self.state.set_lplit(
                        lplit,
                        LpEventCause::MainModel(lit),
                        model,
                        self.identity(),
                        &self.bindings,
                    )?
                }
            }
        }
        Ok(())
    }

    fn check_feasibility(&mut self) -> Result<(), Contradiction> {
        match self.state.solve_or_iis(&mut self.stats) {
            Ok(Err(iis)) => Err(self.build_contradiction(iis)),
            _ => Ok(()),
        }
    }
    fn propagate_reduced_costs_strengthtening(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        let obj_col = self
            .get_objective_column()
            .expect("Reduced costs strengthtening requires an objective.");

        let Some((optim_obj_val, nz_reduced_costs)) = {
            match self.state.solve_or_iis(&mut self.stats) {
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
        let obj_incumbent_bound = self.state.get_objective_current_bound().unwrap();
        let obj_diff = FloatCst::from(obj_incumbent_bound) - optim_obj_val;
        if self.config.use_propagation_skips && (obj_diff).abs() < 1. {
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
            match self.get_objective_sense() {
                Some(LpObjectiveSense::Minimise) => reason.push(LpLit::geq(obj_col, obj_incumbent_bound)),
                Some(LpObjectiveSense::Maximise) => reason.push(LpLit::leq(obj_col, obj_incumbent_bound)),
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
            self.state.set_lplit(
                lplit,
                LpEventCause::ReducedCostStrengthtening(reason.clone()),
                model,
                self.identity(),
                &self.bindings,
            )?;
        }
        self.state.set_lplit(
            match self.get_objective_sense() {
                Some(LpObjectiveSense::Minimise) => LpLit::geq(obj_col, float_as_ceil_int_cst(optim_obj_val)),
                Some(LpObjectiveSense::Maximise) => LpLit::leq(obj_col, float_as_floor_int_cst(optim_obj_val)),
                None => unreachable!(),
            },
            LpEventCause::ReducedCostStrengthtening(reason.clone()),
            model,
            self.identity(),
            &self.bindings,
        )?;
        Ok(())
    }

    fn build_contradiction(&self, iis: LpIis) -> Contradiction {
        /*for r in iis.rows() {
            println!("{r:?}");
        }*/
        let mut conjunction_builder = ConjunctionBuilder::new();

        for &(col, status) in iis.columns() {
            match status {
                highs::HighsIisBoundStatus::Lower => {
                    let lplit = LpLit::geq(col, self.state.get_column_lower_bound(col));
                    if let Some(lits) = self.bindings.compute_implied_lits(lplit) {
                        for lit in lits {
                            conjunction_builder.push(lit);
                        }
                    }
                }
                highs::HighsIisBoundStatus::Upper => {
                    let lplit = LpLit::leq(col, self.state.get_column_upper_bound(col));
                    if let Some(lits) = self.bindings.compute_implied_lits(lplit) {
                        for lit in lits {
                            conjunction_builder.push(lit);
                        }
                    }
                }
                highs::HighsIisBoundStatus::Boxed => {
                    // i.e. equal, i.e. both Lower and Upper
                    let lplit_lb = LpLit::geq(col, self.state.get_column_lower_bound(col));
                    if let Some(lits) = self.bindings.compute_implied_lits(lplit_lb) {
                        for lit in lits {
                            conjunction_builder.push(lit);
                        }
                    }
                    let lplit_ub = LpLit::leq(col, self.state.get_column_upper_bound(col));
                    if let Some(lits) = self.bindings.compute_implied_lits(lplit_ub) {
                        for lit in lits {
                            conjunction_builder.push(lit);
                        }
                    }
                }
                highs::HighsIisBoundStatus::Free => (),
                s => panic!("Unknown highs status {s:?}"),
            }
        }

        // Supplement with literals (if any) implied by the current bounds of columns marked as MaybeInConflict:
        // their participation in the infeasibility is uncertain, but including their bounds
        // keeps the explanation sound at the cost of potentially adding unnecessary literals.
        for i in 0..self.num_columns() {
            let col = LpCol::from(i);
            if iis.contains_column_maybe(col) {
                let lplit_lb = LpLit::geq(col, self.state.get_column_lower_bound(col));
                if let Some(lits) = self.bindings.compute_implied_lits(lplit_lb) {
                    for lit in lits {
                        conjunction_builder.push(lit);
                    }
                }
                let lplit_ub = LpLit::leq(col, self.state.get_column_upper_bound(col));
                if let Some(lits) = self.bindings.compute_implied_lits(lplit_ub) {
                    for lit in lits {
                        conjunction_builder.push(lit);
                    }
                }
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
        let model_updates_to_process = self.state.model_events.num_pending(model.trail());

        if model_updates_to_process > 0 {
            self.process_model_events(model)?;
        }

        // BROKEN // NOTE: This is a *hack* to prevent solving the LP on the "initial" propagation performed before search.
        // BROKEN //       Indeed, in very simple problems, the LP relaxation (and its solving overhead) might not
        // BROKEN //       even be needed to detect unsatisfiability on the first propagation loop.
        // BROKEN self.num_propagation_call += 1;
        // BROKEN if self.config.use_propagation_skips && self.num_propagation_call <= 2 {
        // BROKEN     return Ok(());
        // BROKEN }
        //
        // TODO: Need to find a way to avoid solving the LP when unsatisfiability
        //       can be proven on the first propagation loop without needing it.

        if model_updates_to_process == 0 || !self.config.use_propagation_skips {
            if self.config.use_propagation_skips
                && (self.num_columns() == 0 || (self.num_rows() == 0 && self.state.lpobjective.is_none()))
            {
                return Ok(());
            }

            /*if !self.config.no_propagation_skips
                && self.state.trail.saved_states.len() > 1
                && self.state.trail.saved_states[self.state.trail.num_saved() as usize - 1] == self.state.trail.trail.len()
            {
                return Ok(());
            }*/

            // TODO: allow propagation after a backtrack.
            if self.config.use_propagation_skips && self.current_decision_level() > DecLvl::new(0) {
                return Ok(());
            }

            if self.state.lpobjective.is_some() {
                return self.propagate_reduced_costs_strengthtening(model);
            } else {
                return self.check_feasibility();
            }
        }
        Ok(())
    }

    fn explain(
        &mut self,
        literal: Lit,
        context: InferenceCause,
        model: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    ) {
        let mut add_to_explanation = |l: Lit| {
            debug_assert!(model.entails(l), "{:?} {:#?}", l, self.state.trail.trail);
            out_explanation.push(l);
        };
        debug_assert_eq!(context.writer, self.identity());

        let ModelUpdateCause(lpevent_index) = ModelUpdateCause::from(context.payload);

        debug_assert!(
            self.bindings
                .compute_implied_lits(self.state.trail.get_event(lpevent_index).new_lplit)
                .map(|lits| lits.into_iter().any(|l| l == literal))
                .unwrap_or_default()
        );

        match &self.state.trail.get_event(lpevent_index).cause {
            LpEventCause::MainModel(model_lit) => add_to_explanation(*model_lit),
            LpEventCause::ReducedCostStrengthtening(reason) => {
                for &lplit in reason {
                    let lits = self.bindings.compute_implied_lits(lplit).unwrap();

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
        self.state.set_backtrack_point()
    }
    fn num_saved(&self) -> u32 {
        self.state.trail.num_saved()
    }
    fn restore_last(&mut self) {
        self.state.undo_to_last_backtrack_point();
    }
}

#[cfg(test)]
pub mod test {
    use crate::LpCol;
    use crate::{LpRelax, LpRelaxConfig};
    use aries_solver::backtrack::Backtrack;
    use aries_solver::core::IntCst;
    use aries_solver::core::state::Cause;
    use aries_solver::core::state::Explanation;
    use aries_solver::prelude::Domains;
    use aries_solver::reasoners::Contradiction;
    use aries_solver::reasoners::Theory;

    #[test]
    fn test_trail_backtrack() {
        let mut model = Domains::new();

        let var2 = model.new_var(0, 10);
        let var3 = model.new_var(0, 10);

        model.add_implication(var2.leq(5), var3.leq(5));

        let mut theory = LpRelax::with_config(LpRelaxConfig {
            use_propagation_skips: false,
        });

        let col2 = theory.add_column(Some(0.), Some(10.));
        let col3 = theory.add_column(Some(0.), Some(10.));

        theory.add_var_half_binding_default(var2, col2);
        theory.add_col_half_binding_default(col2, var2);

        theory.add_var_half_binding_default(var3, col3);
        theory.add_col_half_binding_default(col3, var3);

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
            use_propagation_skips: false,
        });

        let acol = theory.add_column(Some(0.), Some(1.));
        let bcol = theory.add_column(Some(0.), Some(1.));

        theory.add_var_half_binding_default(avar, acol);
        theory.add_col_half_binding_default(acol, avar);

        theory.add_var_half_binding_default(bvar, bcol);
        theory.add_col_half_binding_default(bcol, bvar);

        theory.add_row([(acol, 1.), (bcol, 1.)].into_iter(), Some(1.), None);

        let _ = model.set_ub(avar, 0, Cause::Decision).unwrap();
        let _ = model.set_ub(bvar, 0, Cause::Decision).unwrap();

        let expl = match theory.propagate(&mut model) {
            Err(Contradiction::Explanation(expl)) => expl,
            _ => Explanation::new(),
        };
        assert_eq!(expl.literals(), [avar.leq(0), bvar.leq(0)]);
    }
}
