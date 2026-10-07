mod bindings;
mod state;
mod types;

use aries_solver::{
    backtrack::{Backtrack, DecLvl, ObsTrailCursor},
    core::{
        literals::ConjunctionBuilder,
        state::{Domains, DomainsSnapshot, Explanation, InferenceCause},
    },
    reasoners::{Contradiction, ReasonerId, Theory},
};

use state::LpState;

pub use types::*;

use crate::{
    bindings::{Binding, Bindings},
    state::HighsOptionValueWrapper,
};

#[derive(Default, Clone)]
pub struct LpStats {
    pub num_feasibility_checks: u64,
    pub feasibility_checks_time: std::time::Duration,
}

#[derive(Clone)]
pub struct LpOptions {
    /// Used to activate/deactivate the propagation of the reasoner
    pub propagation_active: bool,
    /// If true, refined / minimized explanations will be computed.
    pub is_explanation_refined: bool,
}
impl Default for LpOptions {
    fn default() -> Self {
        Self {
            propagation_active: true,
            is_explanation_refined: false,
        }
    }
}
impl LpOptions {
    fn get_highs_options(&self) -> impl Iterator<Item = (impl Into<Vec<u8>>, HighsOptionValueWrapper)> {
        [
            // ("time_limit", HighsOptionValueWrapper::Float(10.0)),
            ("parallel", HighsOptionValueWrapper::Str("off")), // use 1 core
            ("threads", HighsOptionValueWrapper::Int(1)),      // solve on 1 thread
            (
                "iis_strategy",
                HighsOptionValueWrapper::Int(if self.is_explanation_refined { 4 } else { 0 }),
            ), // https://github.com/ERGO-Code/HiGHS/blob/3be639f037e0001b617c59830d3965f246ab5beb/highs/interfaces/highs_c_api.h#L153
        ]
        .into_iter()
    }
}

pub struct Lp {
    id: ReasonerId,

    model_events: ObsTrailCursor<AriesModelEvent>,
    lp_state: LpState,
    bindings: Bindings,

    pub stats: LpStats,
    options: LpOptions,
}
unsafe impl Send for Lp {}
unsafe impl Sync for Lp {}

impl Clone for Lp {
    fn clone(&self) -> Self {
        let options = LpOptions::default();
        Self {
            id: self.id,
            model_events: self.model_events.clone(),
            lp_state: self.lp_state.clone_with_options(options.get_highs_options()),
            bindings: self.bindings.clone(),
            stats: self.stats.clone(),
            options,
        }
    }
}
impl Default for Lp {
    fn default() -> Self {
        let options = LpOptions::default();
        Self {
            id: ReasonerId::Extra(0),
            model_events: Default::default(),
            lp_state: LpState::default_with_options(options.get_highs_options()),
            bindings: Default::default(),
            stats: Default::default(),
            options,
        }
    }
}

impl Lp {
    pub fn with_options(options: LpOptions) -> Self {
        Self {
            options,
            ..Default::default()
        }
    }

    pub fn activate_propagation(&mut self) {
        self.options.propagation_active = true;
    }
    pub fn deactivate_propagation(&mut self) {
        self.options.propagation_active = false;
    }

    pub fn num_rows(&self) -> usize {
        self.lp_state.num_rows()
    }
    pub fn num_columns(&self) -> usize {
        self.lp_state.num_columns()
    }
    pub fn get_column_bounds(&self, col: LpCol) -> (LongCst, LongCst) {
        self.lp_state.get_column_bounds(col)
    }

    /// First: the number of columns of the LP, created with [`Lp::add_column`] or [`Lp::add_columns`]
    /// (i.e. not counting the objective column, see [`Lp::add_objective_column`])
    ///
    /// Second: how many of them have equal lower and upper bounds
    /// (as of the last propagation, which syncs the LP's bounds with the model)
    pub fn column_counts(&self) -> (usize, usize) {
        (self.lp_state.num_counted(), self.lp_state.num_fixed_counted())
    }

    pub fn add_column_01(&mut self) -> LpCol {
        assert!(self.lp_state.trail().trail.is_empty());
        self.add_column((Some(0), Some(1)))
    }
    pub fn add_column(&mut self, bounds: (Option<IntCst>, Option<IntCst>)) -> LpCol {
        assert!(self.lp_state.trail().trail.is_empty());
        let bounds = (bounds.0.map(int_cst_as_long), bounds.1.map(int_cst_as_long));
        let col = self.lp_state.add_column(bounds);
        self.lp_state.mark_counted(col);
        col
    }
    pub fn add_columns(&mut self, bounds: &[(Option<IntCst>, Option<IntCst>)]) -> Vec<LpCol> {
        assert!(self.lp_state.trail().trail.is_empty());
        let bounds = bounds
            .iter()
            .map(|bounds| (bounds.0.map(int_cst_as_long), bounds.1.map(int_cst_as_long)));
        let cols = self.lp_state.add_columns(bounds);
        for &col in &cols {
            self.lp_state.mark_counted(col);
        }
        cols
    }
    pub fn tighten_column(&mut self, col: LpCol, bounds: (Option<IntCst>, Option<IntCst>)) -> bool {
        assert!(self.lp_state.trail().trail.is_empty());
        let bounds = (bounds.0.map(int_cst_as_long), bounds.1.map(int_cst_as_long));
        self.lp_state.tighten_column(col, bounds)
    }
    pub fn change_column(&mut self, col: LpCol, bounds: (Option<IntCst>, Option<IntCst>)) {
        assert!(self.lp_state.trail().trail.is_empty());
        let bounds = (bounds.0.map(int_cst_as_long), bounds.1.map(int_cst_as_long));
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
        rows: impl Iterator<Item = (Option<IntCst>, Option<IntCst>, impl Iterator<Item = (LpCol, IntCst)>)>,
    ) -> Vec<LpRow> {
        assert!(self.lp_state.trail().trail.is_empty());

        self.lp_state.add_rows(rows.map(|(lb, ub, row)| {
            (
                lb.map(int_cst_as_float),
                ub.map(int_cst_as_float),
                row.into_iter().map(|(col, c)| (col, int_cst_as_float(c))),
            )
        }))
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

    /// Applies `lp_lit` as soon as `scope` and `trigger` are both entailed. Either may be [`AriesLit::TRUE`].
    ///
    /// Both are reported as the cause of the bound, so they must *entail* it.
    pub fn half_bind_fixed(&mut self, scope: AriesLit, trigger: AriesLit, lp_lit: LpLit) {
        assert!(self.lp_state.trail().trail.is_empty());
        self.bindings.add(Binding::Fixed { scope, trigger, lp_lit });
    }

    /// Makes `col` mirror the domain of `var` while `scope` is entailed.
    ///
    /// Unlike [`Lp::half_bind_fixed`], it constrains the column from both sides and is
    /// re-evaluated on every event on `var` or on `scope`'s variable.
    pub fn half_bind_tracking(&mut self, scope: AriesLit, var: AriesVar, col: LpCol) {
        assert!(self.lp_state.trail().trail.is_empty());
        self.bindings.add(Binding::Tracking { scope, var, col });
    }

    fn process_model_events(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        while let Some(main_event) = self.model_events.pop(model.trail()) {
            // Ignore model events that originate from us (this reasoner),
            // as they were already pushed to our (local) trail.
            if let Some(x) = main_event.cause.as_external_inference()
                && x.writer == self.identity()
            {
                continue;
            }

            let derived = self
                .bindings
                .eval_on(main_event.new_literal(), model)
                .collect::<Vec<_>>();

            for (lp_lit, scope, main_lit) in derived {
                self.set_lp_lit(
                    lp_lit,
                    BoundCause::Some {
                        scope,
                        trigger: main_lit,
                    },
                )?;
            }
        }
        Ok(())
    }

    /// Tightens a column bound. One crossing the opposite bound is a conflict, explained by the causes of both.
    fn set_lp_lit(&mut self, lp_lit: LpLit, cause: BoundCause) -> Result<(), Contradiction> {
        if let LpBoundUpdate::Emptied(cause) = self.lp_state.set_lp_lit(lp_lit, cause) {
            // The bound it crosses is the one the column holds on the opposite side.
            let opposite = match lp_lit.tpe {
                LpLitType::GEQ => LpLitType::LEQ,
                LpLitType::LEQ => LpLitType::GEQ,
            };
            let mut expl = Explanation::new();
            cause.explain(&mut |l| expl.push(l));
            self.explain_column_bound(lp_lit.col, opposite, &mut |l| expl.push(l));
            Err(Contradiction::Explanation(expl))
        } else {
            Ok(())
        }
    }

    fn check_feasibility(&mut self) -> Result<(), Contradiction> {
        match self.lp_state.solve_or_iis(&mut self.stats) {
            Err(iis) => Err(self.build_contradiction(iis)),
            _ => Ok(()),
        }
    }

    /// Pushes the main-model literals that justify the bound currently held by `col` on `bound`'s side.
    fn explain_column_bound(&self, col: LpCol, bound: LpLitType, out: &mut impl FnMut(AriesLit)) {
        self.lp_state.get_column_bound_cause(col, bound).explain(out);
    }

    fn build_contradiction(&self, iis: LpIis) -> Contradiction {
        let mut conjunction_builder = ConjunctionBuilder::new();
        let mut explained = 0usize;

        let mut explain_col = |col: LpCol, lower: bool, upper: bool| {
            let mut push = |l: AriesLit| {
                explained += 1;
                conjunction_builder.push(l);
            };
            if lower {
                self.explain_column_bound(col, LpLitType::GEQ, &mut push);
            }
            if upper {
                self.explain_column_bound(col, LpLitType::LEQ, &mut push);
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

        // An IIS that named nothing usable (e.g. HiGHS returned none) would leave an empty explanation,
        // i.e. "infeasible whatever was decided". That's only true if no column has moved since the LP
        // was built; otherwise fall back to every literal that moved one.
        if explained == 0 && !self.lp_state.trail().trail.is_empty() {
            for ev in &self.lp_state.trail().trail {
                self.explain_column_bound(ev.col, ev.bound, &mut |l| conjunction_builder.push(l));
            }
        }

        let mut expl = Explanation::new();
        expl.extend(conjunction_builder.build());
        Contradiction::Explanation(expl)
    }
}

impl Theory for Lp {
    fn identity(&self) -> ReasonerId {
        self.id
    }

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        if self.lp_state.trail().trail.is_empty() {
            let derived = self.bindings.eval_all(model).collect::<Vec<_>>();
            for (lp_lit, scope, main_lit) in derived {
                self.set_lp_lit(
                    lp_lit,
                    BoundCause::Some {
                        scope,
                        trigger: main_lit,
                    },
                )?;
            }
        }

        self.process_model_events(model)?;

        if !self.options.propagation_active {
            return Ok(());
        }

        self.check_feasibility()
    }

    /// Should not be called: this reasoner never infers a literal, it only reports contradictions.
    fn explain(
        &mut self,
        _literal: AriesLit,
        _context: InferenceCause,
        _model: &DomainsSnapshot,
        _out_explanation: &mut Explanation,
    ) {
        unreachable!()
    }

    fn print_stats(&self) {
        println!("# feasibility checks: {}", self.stats.num_feasibility_checks);
        println!(
            "# feasibility checks time: {:.6} s",
            self.stats.feasibility_checks_time.as_secs_f64()
        );
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}

impl Backtrack for Lp {
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
    use aries_solver::backtrack::Backtrack;
    use aries_solver::core::state::{Cause, Domains, Explanation};
    use aries_solver::core::views::Term;
    use aries_solver::reasoners::{Contradiction, Theory};

    use crate::types::*;
    use crate::{Lp, LpOptions};

    #[test]
    fn test_trail_backtrack() {
        let mut model = Domains::new();

        let var2 = model.new_var(0, 10);
        let var3 = model.new_var(0, 10);

        model.add_implication(var2.leq(5), var3.leq(5));

        let mut theory = Lp::default();

        let col2 = theory.add_column((Some(0), Some(10)));
        let col3 = theory.add_column((Some(0), Some(10)));

        theory.half_bind_tracking(AriesLit::TRUE, var2.variable(), col2);
        theory.half_bind_tracking(AriesLit::TRUE, var3.variable(), col3);

        let assert_col_bounds = |theory: &mut Lp, col: LpCol, col_bounds: (LongCst, LongCst)| {
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

        let mut theory = Lp::default();

        let acol = theory.add_column((Some(0), Some(1)));
        let bcol = theory.add_column((Some(0), Some(1)));

        theory.half_bind_tracking(AriesLit::TRUE, avar.variable(), acol);
        theory.half_bind_tracking(AriesLit::TRUE, bvar.variable(), bcol);

        theory.add_row([(acol, 1), (bcol, 1)].into_iter(), (Some(1), None));

        let _ = model.set_ub(avar, 0, Cause::Decision).unwrap();
        let _ = model.set_ub(bvar, 0, Cause::Decision).unwrap();

        let expl = match theory.propagate(&mut model) {
            Err(Contradiction::Explanation(expl)) => expl,
            _ => Explanation::new(),
        };
        assert_eq!(expl.literals(), [avar.leq(0), bvar.leq(0)]);
    }

    /// A fixed binding only applies within its scope, and stops applying again after a backtrack.
    #[test]
    fn test_binding_fixed_scope() {
        let mut model = Domains::new();

        let p = model.new_var(0, 1);
        let q = model.new_var(0, 1);
        let scope = model.new_var(0, 1);

        let mut theory = Lp::default();

        let acol = theory.add_column((Some(0), Some(1)));
        let bcol = theory.add_column((Some(0), Some(1)));

        theory.half_bind_fixed(AriesLit::TRUE, p.leq(0), LpLit::leq(acol, 0));
        theory.half_bind_fixed(scope.geq(1), q.leq(0), LpLit::leq(bcol, 0));

        model.save_state();
        theory.save_state();

        assert_eq!(model.set(p.leq(0), Cause::Decision), Ok(true));
        assert_eq!(model.set(q.leq(0), Cause::Decision), Ok(true));
        assert!(theory.propagate(&mut model).is_ok());

        // `q <= 0` holds, but the binding that depends on it is still out of scope
        assert_eq!(theory.get_column_bounds(acol), (0, 0));
        assert_eq!(theory.get_column_bounds(bcol), (0, 1));

        model.save_state();
        theory.save_state();

        assert_eq!(model.set(scope.geq(1), Cause::Decision), Ok(true));
        assert!(theory.propagate(&mut model).is_ok());

        // entering the scope applies it, even though the event was on the scope rather than on `q`
        assert_eq!(theory.get_column_bounds(bcol), (0, 0));

        model.restore_last();
        theory.restore_last();

        assert_eq!(theory.get_column_bounds(acol), (0, 0));
        assert_eq!(theory.get_column_bounds(bcol), (0, 1));
    }

    /// A conflict is explained by the triggers of the bindings involved, not by the bounds that the
    /// domains happen to hold at that point.
    #[test]
    fn test_binding_fixed_explanation() {
        let mut model = Domains::new();

        let p = model.new_var(0, 10);
        let scope = model.new_var(0, 1);

        let mut theory = Lp::default();

        let acol = theory.add_column((Some(0), Some(1)));
        let bcol = theory.add_column((Some(0), Some(1)));

        // both columns are pinned to 0 as soon as `p` is above 4, which the row forbids
        theory.half_bind_fixed(AriesLit::TRUE, p.geq(5), LpLit::leq(acol, 0));
        theory.half_bind_fixed(scope.geq(1), p.geq(5), LpLit::leq(bcol, 0));
        theory.add_row([(acol, 1), (bcol, 1)].into_iter(), (Some(1), None));

        assert_eq!(model.set(scope.geq(1), Cause::Decision), Ok(true));
        assert_eq!(model.set(p.geq(9), Cause::Decision), Ok(true));

        let expl = match theory.propagate(&mut model) {
            Err(Contradiction::Explanation(expl)) => expl,
            _ => Explanation::new(),
        };

        // `p >= 5` is what the bindings asked for; `p >= 9` is what the domain holds
        assert!(expl.literals().contains(&p.geq(5)), "{:?}", expl.literals());
        assert!(!expl.literals().contains(&p.geq(9)), "{:?}", expl.literals());
        assert!(expl.literals().contains(&scope.geq(1)), "{:?}", expl.literals());
    }

    /// Two bindings that empty a column between them conflict when the second is applied,
    /// without the LP ever being solved, and are explained by the triggers of both.
    #[test]
    fn test_crossing_bounds_conflict() {
        let mut model = Domains::new();

        let p = model.new_var(0, 1);
        let q = model.new_var(0, 1);

        let mut theory = Lp::default();
        let col = theory.add_column((Some(0), Some(1)));
        theory.half_bind_fixed(AriesLit::TRUE, p.leq(0), LpLit::leq(col, 0));
        theory.half_bind_fixed(AriesLit::TRUE, q.leq(0), LpLit::geq(col, 1));

        assert_eq!(model.set(p.leq(0), Cause::Decision), Ok(true));
        assert_eq!(model.set(q.leq(0), Cause::Decision), Ok(true));

        let Err(Contradiction::Explanation(expl)) = theory.propagate(&mut model) else {
            panic!("the column cannot be both <= 0 and >= 1")
        };
        assert!(expl.literals().contains(&p.leq(0)), "{:?}", expl.literals());
        assert!(expl.literals().contains(&q.leq(0)), "{:?}", expl.literals());
    }

    /// Backtracking one level restores the bound the column had at that level,
    /// not the one it was created with.
    #[test]
    fn test_bound_restored_to_intermediate_value() {
        let mut model = Domains::new();

        let p = model.new_var(0, 1);
        let q = model.new_var(0, 1);

        let mut theory = Lp::default();
        let col = theory.add_column((Some(0), Some(10)));
        theory.half_bind_fixed(AriesLit::TRUE, p.leq(0), LpLit::leq(col, 6));
        theory.half_bind_fixed(AriesLit::TRUE, q.leq(0), LpLit::leq(col, 3));

        model.save_state();
        theory.save_state();
        assert_eq!(model.set(p.leq(0), Cause::Decision), Ok(true));
        assert!(theory.propagate(&mut model).is_ok());
        assert_eq!(theory.get_column_bounds(col), (0, 6));

        model.save_state();
        theory.save_state();
        assert_eq!(model.set(q.leq(0), Cause::Decision), Ok(true));
        assert!(theory.propagate(&mut model).is_ok());
        assert_eq!(theory.get_column_bounds(col), (0, 3));

        model.restore_last();
        theory.restore_last();
        assert_eq!(theory.get_column_bounds(col), (0, 6));

        model.restore_last();
        theory.restore_last();
        assert_eq!(theory.get_column_bounds(col), (0, 10));
    }

    /// An empty row with a positive lower bound cannot be satisfied on its own,
    /// but is only considered infeasible by HiGHS once the LP has a column.
    /// Without any columns, it reports the problem as feasible, even though the row is inconsistent on its own.
    #[test]
    fn test_empty_row_with_positive_lower_bound() {
        let mut model = Domains::new();

        let mut theory = Lp::default();
        theory.add_row(std::iter::empty::<(LpCol, IntCst)>(), (Some(1), None));
        assert_eq!(theory.num_columns(), 0);
        assert!(theory.propagate(&mut model).is_ok());

        let mut theory = Lp::default();
        theory.add_column((Some(0), Some(1)));
        theory.add_row(std::iter::empty::<(LpCol, IntCst)>(), (Some(1), None));

        let Err(Contradiction::Explanation(expl)) = theory.propagate(&mut model) else {
            panic!("a row demanding `0 >= 1` is infeasible")
        };
        assert!(expl.literals().is_empty(), "{:?}", expl.literals());

        let mut theory = Lp::default();
        let col = theory.add_column((Some(0), Some(1)));
        theory.add_rows(std::iter::once((Some(1), Some(0), std::iter::once((col, 1)))));

        let Err(Contradiction::Explanation(expl)) = theory.propagate(&mut model) else {
            panic!("a row bounded [1, 0] cannot be satisfied")
        };
        assert!(expl.literals().is_empty(), "{:?}", expl.literals());
    }

    /// An LP that is infeasible from the bounds its columns were created with is infeasible whatever the search does, so its explanation is empty.
    #[test]
    fn test_root_infeasibility_is_explained_by_nothing() {
        let mut model = Domains::new();

        let mut theory = Lp::default();
        let acol = theory.add_column((Some(0), Some(0)));
        let bcol = theory.add_column((Some(0), Some(0)));
        theory.add_row([(acol, 1), (bcol, 1)].into_iter(), (Some(1), None));

        let Err(Contradiction::Explanation(expl)) = theory.propagate(&mut model) else {
            panic!("`a + b >= 1` cannot hold with both columns pinned to 0")
        };
        assert!(expl.literals().is_empty(), "{:?}", expl.literals());
    }

    /// After an infeasible solve, backtracking makes the LP feasible again,
    /// and every later propagation really solves the LP instead of reusing the previous status.
    #[test]
    fn test_feasible_again_after_infeasible() {
        let mut model = Domains::new();

        let avar = model.new_var(0, 1);
        let bvar = model.new_var(0, 1);

        let mut theory = Lp::default();

        let acol = theory.add_column((Some(0), Some(1)));
        let bcol = theory.add_column((Some(0), Some(1)));

        theory.half_bind_tracking(AriesLit::TRUE, avar.variable(), acol);
        theory.half_bind_tracking(AriesLit::TRUE, bvar.variable(), bcol);

        theory.add_row([(acol, 1), (bcol, 1)].into_iter(), (Some(1), None));

        // Propagates, and checks that the LP was solved (once) to get the result.
        let propagate = |theory: &mut Lp, model: &mut Domains| {
            let lpruns = theory.stats.num_feasibility_checks;
            let res = theory.propagate(model);
            assert_eq!(theory.stats.num_feasibility_checks, lpruns + 1);
            res
        };

        model.save_state();
        theory.save_state();
        assert_eq!(model.set(avar.leq(0), Cause::Decision), Ok(true));
        assert!(propagate(&mut theory, &mut model).is_ok());

        model.save_state();
        theory.save_state();
        assert_eq!(model.set(bvar.leq(0), Cause::Decision), Ok(true));
        let Err(Contradiction::Explanation(expl)) = propagate(&mut theory, &mut model) else {
            panic!("`a + b >= 1` cannot hold with `a <= 0` and `b <= 0`")
        };
        assert_eq!(expl.literals(), [avar.leq(0), bvar.leq(0)]);

        // `b` can be 1 again
        model.restore_last();
        theory.restore_last();
        assert!(propagate(&mut theory, &mut model).is_ok());

        // and the infeasibility is found again
        model.save_state();
        theory.save_state();
        assert_eq!(model.set(bvar.leq(0), Cause::Decision), Ok(true));
        assert!(propagate(&mut theory, &mut model).is_err());

        model.restore_last();
        theory.restore_last();
        model.restore_last();
        theory.restore_last();
        assert!(propagate(&mut theory, &mut model).is_ok());
    }

    /// The columns are counted as fixed whenever their bounds are equal.
    /// Bounds are synced with backtracking and propagation (even when deactivated).
    #[test]
    fn test_column_counts() {
        let mut model = Domains::new();
        let p = model.new_var(0, 1);
        let q = model.new_var(0, 1);

        let mut theory = Lp::with_options(LpOptions {
            propagation_active: false,
            ..Default::default()
        });
        let acol = theory.add_column((Some(0), Some(1)));
        let bcol = theory.add_column((Some(0), Some(1)));
        let ccol = theory.add_column((Some(0), Some(1)));
        theory.half_bind_fixed(AriesLit::TRUE, p.leq(0), LpLit::leq(acol, 0));
        theory.half_bind_fixed(AriesLit::TRUE, q.geq(1), LpLit::geq(bcol, 1));

        assert_eq!(theory.column_counts(), (3, 0));
        theory.tighten_column(ccol, (None, Some(0)));
        assert_eq!(theory.column_counts(), (3, 1));

        model.save_state();
        theory.save_state();
        assert_eq!(model.set(p.leq(0), Cause::Decision), Ok(true));
        assert!(theory.propagate(&mut model).is_ok());
        assert_eq!(theory.column_counts(), (3, 2));

        model.save_state();
        theory.save_state();
        assert_eq!(model.set(q.geq(1), Cause::Decision), Ok(true));
        assert!(theory.propagate(&mut model).is_ok());
        assert_eq!(theory.column_counts(), (3, 3));
        assert_eq!(theory.stats.num_feasibility_checks, 0);

        model.restore_last();
        theory.restore_last();
        assert_eq!(theory.column_counts(), (3, 2));
        model.restore_last();
        theory.restore_last();
        assert_eq!(theory.column_counts(), (3, 1));

        theory.activate_propagation();
        assert!(theory.propagate(&mut model).is_ok());
        assert_eq!(theory.stats.num_feasibility_checks, 1);
    }
}
