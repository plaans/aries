mod bindings;
mod state;
mod types;

use std::sync::Arc;

use aries_solver::{
    backtrack::{Backtrack, DecLvl, ObsTrailCursor},
    core::{
        literals::ConjunctionBuilder,
        state::{Domains, DomainsSnapshot, Explanation, InferenceCause},
        views::Dom,
    },
    reasoners::{Contradiction, ReasonerId, Theory},
};

use bindings::AriesLitToLpLitHalfBindings;
use state::LpRelaxState;

pub use types::*;

use crate::bindings::{AriesLitToLpLitHalfBinding, AriesLitToLpLitHalfBindingFn};

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
pub struct LpRelax {
    id: ReasonerId,

    main_events: ObsTrailCursor<AriesModelEvent>,
    lp_state: LpRelaxState,
    bindings: AriesLitToLpLitHalfBindings,

    stats: LpRelaxStats,
    config: LpRelaxConfig,
}
unsafe impl Send for LpRelax {}
unsafe impl Sync for LpRelax {}

impl Default for LpRelax {
    fn default() -> Self {
        Self {
            id: ReasonerId::Extra(0),
            main_events: Default::default(),
            lp_state: Default::default(),
            bindings: Default::default(),
            stats: Default::default(),
            config: Default::default(),
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
        self.lp_state.num_rows()
    }
    pub fn num_columns(&self) -> usize {
        self.lp_state.num_columns()
    }
    pub fn get_column_bounds(&self, col: LpCol) -> (IntCst, IntCst) {
        let (lb, ub) = self.lp_state.get_column_bounds(col);
        (float_as_exact_int_cst(lb), float_as_exact_int_cst(ub))
    }

    pub fn add_column_01(&mut self) -> LpCol {
        assert!(self.lp_state.trail().trail.is_empty());
        self.add_column((Some(0), Some(1)))
    }
    pub fn add_column(&mut self, bounds: (Option<IntCst>, Option<IntCst>)) -> LpCol {
        assert!(self.lp_state.trail().trail.is_empty());
        let bounds = (bounds.0.map(int_cst_as_float), bounds.1.map(int_cst_as_float));
        self.lp_state.add_column(bounds)
    }
    pub fn add_columns(&mut self, bounds: &[(Option<IntCst>, Option<IntCst>)]) -> Vec<LpCol> {
        assert!(self.lp_state.trail().trail.is_empty());
        let bounds = bounds
            .iter()
            .map(|bounds| (bounds.0.map(int_cst_as_float), bounds.1.map(int_cst_as_float)));
        self.lp_state.add_columns(bounds)
    }
    pub fn tighten_column(&mut self, col: LpCol, bounds: (Option<IntCst>, Option<IntCst>)) -> bool {
        assert!(self.lp_state.trail().trail.is_empty());
        let bounds = (bounds.0.map(int_cst_as_float), bounds.1.map(int_cst_as_float));
        self.lp_state.tighten_column(col, bounds)
    }
    pub fn change_column(&mut self, col: LpCol, bounds: (Option<IntCst>, Option<IntCst>)) {
        assert!(self.lp_state.trail().trail.is_empty());
        let bounds = (bounds.0.map(int_cst_as_float), bounds.1.map(int_cst_as_float));
        self.lp_state.change_column(col, bounds)
    }

    pub fn add_row(
        &mut self,
        row_coefs: impl Iterator<Item = (LpCol, IntCst)>,
        bounds: (Option<IntCst>, Option<IntCst>),
    ) -> LpRow {
        assert!(self.lp_state.trail().trail.is_empty());
        let row_coefs = row_coefs.map(|(col, c)| (col, int_cst_as_float(c)));
        let bounds = (bounds.0.map(int_cst_as_float), bounds.1.map(int_cst_as_float));
        self.lp_state.add_row(row_coefs, bounds)
    }
    pub fn add_rows(
        &mut self,
        rows_coefs: impl Iterator<Item = Vec<(LpCol, IntCst)>>,
        bounds: &[(Option<IntCst>, Option<IntCst>)],
    ) -> Vec<LpRow> {
        assert!(self.lp_state.trail().trail.is_empty());
        let rows_coefs = rows_coefs
            .map(|row_coefs| {
                row_coefs
                    .into_iter()
                    .map(|(col, c)| (col, int_cst_as_float(c)))
                    .collect()
            })
            .collect();
        let bounds = bounds
            .iter()
            .map(|bounds| (bounds.0.map(int_cst_as_float), bounds.1.map(int_cst_as_float)))
            .collect();
        self.lp_state.add_rows(rows_coefs, bounds)
    }

    pub fn add_objective_column(
        &mut self,
        main_var: AriesVar,
        coefs: impl Iterator<Item = (LpCol, FloatCst)>,
        sense: LpObjectiveSense,
    ) -> LpCol {
        assert!(self.lp_state.trail().trail.is_empty());
        self.lp_state.add_objective_column(main_var, coefs, sense)
    }
    pub fn get_objective_column(&self) -> Option<LpCol> {
        self.lp_state.get_objective_column()
    }
    pub fn get_objective_main_var(&self) -> Option<AriesVar> {
        self.lp_state.get_objective_main_var()
    }
    pub fn get_objective_sense(&self) -> Option<LpObjectiveSense> {
        self.lp_state.get_objective_sense()
    }

    pub fn add_binding(
        &mut self,
        scope: AriesLit,
        svar: AriesSignedVar,
        col: LpCol,
        map_fn: Arc<AriesLitToLpLitHalfBindingFn>,
    ) {
        assert!(self.lp_state.trail().trail.is_empty());
        self.bindings
            .add(AriesLitToLpLitHalfBinding::new(scope, svar, col, map_fn));
    }

    fn process_model_events(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        while let Some(main_event) = self.main_events.pop(model.trail()) {
            // Ignore model events that originate from us (this reasoner),
            // as they were already pushed to our (local) trail.
            if let Some(x) = main_event.cause.as_external_inference()
                && x.writer == self.identity()
            {
                continue;
            }

            let svar = main_event.new_literal().svar();
            let derived = self
                .bindings
                .eval_for_var(svar, model)
                .chain(self.bindings.eval_for_scope_var(svar, model))
                .collect::<Vec<_>>();

            for (lp_lit, scope, main_lit) in derived {
                self.set_lp_lit(lp_lit, LpEventCause::Binding { scope, main: main_lit })?;
            }
        }
        Ok(())
    }

    /// Tightens a column bound. One crossing the opposite bound is a conflict, explained by the causes of both.
    fn set_lp_lit(&mut self, lp_lit: LpLit, cause: LpEventCause) -> Result<(), Contradiction> {
        if let LpBoundUpdate::Emptied(cause) = self.lp_state.set_lp_lit(lp_lit, cause) {
            let bounds = self.lp_state.get_column_bounds(lp_lit.col);
            let opposite = match lp_lit.tpe {
                LpLitType::GEQ => LpLit::leq(lp_lit.col, float_as_exact_int_cst(bounds.1)),
                LpLitType::LEQ => LpLit::geq(lp_lit.col, float_as_exact_int_cst(bounds.0)),
            };
            let now = self.lp_state.trail().trail.len();
            let mut expl = Explanation::new();
            self.explain_cause(&cause, now, &mut |l| expl.push(l));
            self.explain_lp_lit(opposite, now, &mut |l| expl.push(l));
            Err(Contradiction::Explanation(expl))
        } else {
            Ok(())
        }
    }

    fn check_feasibility(&mut self) -> Result<(), Contradiction> {
        match self.lp_state.solve_or_iis(&mut self.stats) {
            Ok(Err(iis)) => Err(self.build_contradiction(iis)),
            _ => Ok(()),
        }
    }

    /*fn propagate_reduced_costs_strengthtening(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        TODO OR REMOVE
        Ok(())
    }*/

    fn explain_cause(&self, cause: &LpEventCause, _len: usize, out: &mut impl FnMut(AriesLit)) {
        match cause {
            LpEventCause::Binding { scope, main } => {
                if !(scope.tautological()) {
                    out(*scope);
                }
                if !(main.tautological()) {
                    out(*main);
                }
            } // LpEventCause::ReducedCostStrengthtening(reason) => {
              //     for &r in reason {
              //         self.explain_lp_lit(r, len, out);
              //     }
              // }
        }
    }

    /// Pushes the main-model literals that justify `lp_lit`, using only the trail's first `len` events.
    fn explain_lp_lit(&self, lp_lit: LpLit, len: usize, out: &mut impl FnMut(AriesLit)) {
        if let Some(ev) = self.lp_state.establishing_event(lp_lit, len) {
            let before = u32::from(ev) as usize;
            self.explain_cause(&self.lp_state.trail().get_event(ev).cause, before, out);
        }
    }

    fn build_contradiction(&self, iis: LpIis) -> Contradiction {
        let now = self.lp_state.trail().trail.len();
        let mut conjunction_builder = ConjunctionBuilder::new();
        let mut explained = 0usize;

        let mut explain_col = |col: LpCol, lower: bool, upper: bool| {
            let mut push = |l: AriesLit| {
                explained += 1;
                conjunction_builder.push(l);
            };
            let (lb, ub) = self.lp_state.get_column_bounds(col);
            if lower {
                self.explain_lp_lit(LpLit::geq(col, float_as_exact_int_cst(lb)), now, &mut push);
            }
            if upper {
                self.explain_lp_lit(LpLit::leq(col, float_as_exact_int_cst(ub)), now, &mut push);
            }
        };

        for &(col, status) in iis.columns() {
            match status {
                highs::HighsIisBoundStatus::Lower => explain_col(col, true, false),
                highs::HighsIisBoundStatus::Upper => explain_col(col, false, true),
                highs::HighsIisBoundStatus::Boxed => explain_col(col, true, true),
                highs::HighsIisBoundStatus::Free => (),
                s => panic!("Unknown highs status {s:?}"),
            }
        }

        // Columns whose participation is uncertain: those never tightened contribute nothing.
        for i in 0..self.num_columns() {
            let col = LpCol::from(i);
            if iis.contains_column_maybe(col) {
                explain_col(col, true, true);
            }
        }

        // An IIS that named nothing usable (e.g. HiGHS returned none) would leave an empty explanation,
        // i.e. "infeasible whatever was decided". That's only true if no column has moved since the LP
        // was built; otherwise fall back to every literal that moved one.
        if explained == 0 && !self.lp_state.trail().trail.is_empty() {
            for ev in &self.lp_state.trail().trail {
                self.explain_cause(&ev.cause, now, &mut |l| conjunction_builder.push(l));
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
        if self.lp_state.trail().trail.is_empty() {
            let derived = self.bindings.eval_all(model).collect::<Vec<_>>();
            for (lp_lit, scope, main_lit) in derived {
                self.set_lp_lit(lp_lit, LpEventCause::Binding { scope, main: main_lit })?;
            }
        }

        let model_updates_to_process = self.main_events.num_pending(model.trail());
        self.process_model_events(model)?;

        if !self.config.use_propagation_skips || model_updates_to_process == 0 {
            if self.config.use_propagation_skips
                && (self.num_columns() == 0 || (self.num_rows() == 0 && self.get_objective_column().is_none()))
            {
                return Ok(());
            }

            // FIXME TODO: allow propagation after a backtrack (if it was relatively long ?)
            if self.stats.lpruns > 0 {
                return Ok(());
            }

            if !self.config.use_propagation_skips
                || model
                    .assumptions_sealed_at()
                    .is_some_and(|lvl| lvl >= self.current_decision_level())
            {
                println!(
                    "|-[LPRELAX]- Solving LP at decision level {:?} (num events: {:?}) with HiGHS",
                    model.current_decision_level(),
                    model.num_events()
                );
                if self.lp_state.get_objective_column().is_some() {
                    // return self.propagate_reduced_costs_strengthtening(model);
                } else {
                    return self.check_feasibility();
                }
            }
        }
        Ok(())
    }

    fn explain(
        &mut self,
        literal: AriesLit,
        context: InferenceCause,
        model: &DomainsSnapshot,
        out_explanation: &mut Explanation,
    ) {
        debug_assert_eq!(context.writer, self.identity());

        let lp_event_index: aries_solver::backtrack::EventIndex = context.payload.into();

        let lp_lit = self.lp_state.trail().get_event(lp_event_index).new_lp_lit;

        self.explain_lp_lit(lp_lit, u32::from(lp_event_index) as usize + 1, &mut |l| {
            debug_assert!(model.entails(l), "{:?} {:#?}", l, self.lp_state.trail().trail);
            out_explanation.push(l)
        });

        debug_assert!(out_explanation.literals().contains(&literal));
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
        self.lp_state.set_backtrack_point()
    }
    fn num_saved(&self) -> u32 {
        self.lp_state.trail().num_saved()
    }
    fn restore_last(&mut self) {
        self.lp_state.undo_to_last_backtrack_point();
    }
}

#[cfg(test)]
pub mod test {
    use std::sync::Arc;

    use aries_solver::backtrack::Backtrack;
    use aries_solver::core::state::{Cause, Domains, Explanation};
    use aries_solver::core::views::Term;
    use aries_solver::reasoners::{Contradiction, Theory};

    use crate::types::*;
    use crate::{LpRelax, LpRelaxConfig};

    #[test]
    fn test_trail_backtrack() {
        let mut model = Domains::new();

        let var2 = model.new_var(0, 10);
        let var3 = model.new_var(0, 10);

        model.add_implication(var2.leq(5), var3.leq(5));

        let mut theory = LpRelax::with_config(LpRelaxConfig {
            use_propagation_skips: false,
        });

        let col2 = theory.add_column((Some(0), Some(10)));
        let col3 = theory.add_column((Some(0), Some(10)));

        theory.add_binding(
            AriesLit::TRUE,
            AriesSignedVar::plus(var2.variable()),
            LpCol::from(col2),
            Arc::new(|v| Some((LpLitType::LEQ, v))),
        );
        theory.add_binding(
            AriesLit::TRUE,
            AriesSignedVar::minus(var2.variable()),
            LpCol::from(col2),
            Arc::new(|v| Some((LpLitType::GEQ, v))),
        );
        theory.add_binding(
            AriesLit::TRUE,
            AriesSignedVar::plus(var3.variable()),
            LpCol::from(col3),
            Arc::new(|v| Some((LpLitType::LEQ, v))),
        );
        theory.add_binding(
            AriesLit::TRUE,
            AriesSignedVar::minus(var3.variable()),
            LpCol::from(col3),
            Arc::new(|v| Some((LpLitType::GEQ, v))),
        );

        let assert_col_bounds = |theory: &mut LpRelax, col: LpCol, col_bounds: (IntCst, IntCst)| {
            assert_eq!(theory.get_column_bounds(col), col_bounds)
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

        let acol = theory.add_column((Some(0), Some(1)));
        let bcol = theory.add_column((Some(0), Some(1)));

        theory.add_binding(
            AriesLit::TRUE,
            AriesSignedVar::plus(avar.variable()),
            LpCol::from(acol),
            Arc::new(|v| Some((LpLitType::LEQ, v))),
        );
        theory.add_binding(
            AriesLit::TRUE,
            AriesSignedVar::minus(avar.variable()),
            LpCol::from(acol),
            Arc::new(|v| Some((LpLitType::GEQ, v))),
        );
        theory.add_binding(
            AriesLit::TRUE,
            AriesSignedVar::plus(bvar.variable()),
            LpCol::from(bcol),
            Arc::new(|v| Some((LpLitType::LEQ, v))),
        );
        theory.add_binding(
            AriesLit::TRUE,
            AriesSignedVar::minus(bvar.variable()),
            LpCol::from(bcol),
            Arc::new(|v| Some((LpLitType::GEQ, v))),
        );

        theory.add_row([(acol, 1), (bcol, 1)].into_iter(), (Some(1), None));

        let _ = model.set_ub(avar, 0, Cause::Decision).unwrap();
        let _ = model.set_ub(bvar, 0, Cause::Decision).unwrap();

        let expl = match theory.propagate(&mut model) {
            Err(Contradiction::Explanation(expl)) => expl,
            _ => Explanation::new(),
        };
        assert_eq!(expl.literals(), [avar.leq(0), bvar.leq(0)]);
    }
}
