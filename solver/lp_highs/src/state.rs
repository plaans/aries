use aries_solver::backtrack::{DecLvl, Trail};

use crate::types::*;

#[derive(Debug, Clone)]
struct LpObjective {
    pub col: LpCol,
    pub main_var: AriesVar,
    pub sense: LpObjectiveSense,
}

/// Bounds of a column and the cause justifying each of them.
///
/// The LP model holds the same bounds as floats.
/// This is also where the causes needed by the explanations are recorded.
#[derive(Debug, Clone)]
struct ColBounds {
    lower: LongCst,
    lower_cause: BoundCause,
    upper: LongCst,
    upper_cause: BoundCause,
}

pub(super) struct LpState {
    lp_trail: Trail<LpEvent>,
    col_bounds: Vec<ColBounds>,

    lp_model: LpModel,
    lp_obj: Option<LpObjective>,

    /// Whether each column is counted in `num_fixed_counted` (see [`LpState::mark_counted`])
    is_counted: Vec<bool>,
    /// The number of counted columns
    num_counted: usize,
    /// The number of counted columns whose lower and upper bounds are equal
    num_fixed_counted: usize,
}

#[allow(dead_code)]
pub(super) enum HighsOptionValueWrapper {
    Str(&'static str),
    Int(i32),
    Float(f64),
    Bool(bool),
}

impl LpState {
    pub fn default_with_options(options: impl Iterator<Item = (impl Into<Vec<u8>>, HighsOptionValueWrapper)>) -> Self {
        let mut lp_model = highs::ColProblem::default().optimise(LpObjectiveSense::Minimise);
        for (k, v) in options {
            match v {
                HighsOptionValueWrapper::Str(v) => lp_model.set_option(k, v),
                HighsOptionValueWrapper::Int(v) => lp_model.set_option(k, v),
                HighsOptionValueWrapper::Float(v) => lp_model.set_option(k, v),
                HighsOptionValueWrapper::Bool(v) => lp_model.set_option(k, v),
            }
        }
        Self {
            lp_trail: Default::default(),
            col_bounds: Default::default(),
            lp_model,
            lp_obj: None,
            is_counted: Default::default(),
            num_counted: 0,
            num_fixed_counted: 0,
        }
    }

    pub fn clone_with_options(
        &self,
        options: impl Iterator<Item = (impl Into<Vec<u8>>, HighsOptionValueWrapper)>,
    ) -> Self {
        let mut lp_model = self.lp_model.clone();
        for (k, v) in options {
            match v {
                HighsOptionValueWrapper::Str(v) => lp_model.set_option(k, v),
                HighsOptionValueWrapper::Int(v) => lp_model.set_option(k, v),
                HighsOptionValueWrapper::Float(v) => lp_model.set_option(k, v),
                HighsOptionValueWrapper::Bool(v) => lp_model.set_option(k, v),
            }
        }
        Self {
            lp_trail: self.lp_trail.clone(),
            col_bounds: self.col_bounds.clone(),
            lp_model,
            lp_obj: self.lp_obj.clone(),
            is_counted: self.is_counted.clone(),
            num_counted: self.num_counted,
            num_fixed_counted: self.num_fixed_counted,
        }
    }

    pub fn trail(&self) -> &Trail<LpEvent> {
        &self.lp_trail
    }

    pub fn num_rows(&self) -> usize {
        self.lp_model.num_rows()
    }
    pub fn num_columns(&self) -> usize {
        self.lp_model.num_cols()
    }

    pub fn get_column_bounds(&self, col: LpCol) -> (LongCst, LongCst) {
        let bounds = &self.col_bounds[col.index()];
        (bounds.lower, bounds.upper)
    }

    /// Cause justifying one of the bounds of a column.
    pub fn get_column_bound_cause(&self, col: LpCol, bound: LpLitType) -> BoundCause {
        let bounds = &self.col_bounds[col.index()];
        match bound {
            LpLitType::GEQ => bounds.lower_cause,
            LpLitType::LEQ => bounds.upper_cause,
        }
    }

    /// Marks `col` as counted: it is then included in `num_fixed_counted` whenever its lower and upper bounds are equal
    pub fn mark_counted(&mut self, col: LpCol) {
        if self.is_counted[col.index()] {
            return;
        }
        self.is_counted[col.index()] = true;
        self.num_counted += 1;
        let (lower, upper) = self.get_column_bounds(col);
        if lower == upper {
            self.num_fixed_counted += 1;
        }
    }
    pub fn num_counted(&self) -> usize {
        self.num_counted
    }
    pub fn num_fixed_counted(&self) -> usize {
        self.num_fixed_counted
    }

    /// Keeps `num_fixed_counted` up to date after a change of the bounds of `col`, given whether they were equal before
    fn update_num_fixed_counted(&mut self, col: LpCol, was_fixed: bool) {
        if !self.is_counted[col.index()] {
            return;
        }
        let (lower, upper) = self.get_column_bounds(col);
        match (was_fixed, lower == upper) {
            (false, true) => self.num_fixed_counted += 1,
            (true, false) => self.num_fixed_counted -= 1,
            _ => {}
        }
    }

    /// Mirrors the bounds recorded for a column into the LP model.
    fn update_model_with_column_bounds(&mut self, col: LpCol) {
        let (lower, upper) = self.get_column_bounds(col);
        self.lp_model
            .change_column_bounds(col, long_cst_as_float(lower)..=long_cst_as_float(upper));
    }

    pub fn add_column(&mut self, bounds: (Option<LongCst>, Option<LongCst>)) -> LpCol {
        self.add_columns(std::iter::once(bounds))[0]
    }

    pub fn add_columns(&mut self, bounds: impl Iterator<Item = (Option<LongCst>, Option<LongCst>)>) -> Vec<LpCol> {
        let bounds = bounds
            .map(|(lb, ub)| {
                // An unbounded side is held as the extremum of the type, which converts to a
                // magnitude HiGHS treats as infinite.
                let (lower, upper) = (lb.unwrap_or(LongCst::MIN), ub.unwrap_or(LongCst::MAX));
                assert!(lower <= upper);
                self.col_bounds.push(ColBounds {
                    lower,
                    lower_cause: BoundCause::None,
                    upper,
                    upper_cause: BoundCause::None,
                });
                self.is_counted.push(false);
                long_cst_as_float(lower)..=long_cst_as_float(upper)
            })
            .collect::<Vec<_>>();

        let old_cols_num = self.lp_model.num_cols();
        self.lp_model.add_columns(bounds);

        debug_assert_eq!(self.col_bounds.len(), self.lp_model.num_cols());
        (old_cols_num..self.num_columns()).map(LpCol::from).collect()
    }

    /// Restricts the bounds of a column unconditionally, i.e. with no cause to justify them.
    /// Only meaningful before the search starts. Returns whether either bound was restricted.
    pub fn tighten_column(&mut self, col: LpCol, bounds: (Option<LongCst>, Option<LongCst>)) -> bool {
        debug_assert!(self.lp_trail.trail.is_empty());
        let (old_lower, old_upper) = self.get_column_bounds(col);

        let lower = bounds.0.filter(|lb| *lb > old_lower);
        let upper = bounds.1.filter(|ub| *ub < old_upper);

        let entry = &mut self.col_bounds[col.index()];
        if let Some(lower) = lower {
            entry.lower = lower;
        }
        if let Some(upper) = upper {
            entry.upper = upper;
        }
        assert!(entry.lower <= entry.upper);

        self.update_model_with_column_bounds(col);
        self.update_num_fixed_counted(col, old_lower == old_upper);

        lower.is_some() || upper.is_some()
    }

    /// Overwrites the bounds of a column unconditionally. Only meaningful before the search starts.
    pub fn change_column(&mut self, col: LpCol, bounds: (Option<LongCst>, Option<LongCst>)) {
        debug_assert!(self.lp_trail.trail.is_empty());
        let entry = &mut self.col_bounds[col.index()];
        let was_fixed = entry.lower == entry.upper;
        entry.lower = bounds.0.unwrap_or(LongCst::MIN);
        entry.upper = bounds.1.unwrap_or(LongCst::MAX);
        assert!(entry.lower <= entry.upper);

        self.update_model_with_column_bounds(col);
        self.update_num_fixed_counted(col, was_fixed);
    }

    pub fn add_row(
        &mut self,
        row_coefs: impl Iterator<Item = (LpCol, FloatCst)>,
        bounds: (Option<FloatCst>, Option<FloatCst>),
    ) -> LpRow {
        let lb = bounds.0.unwrap_or(FloatCst::MIN);
        let ub = bounds.1.unwrap_or(FloatCst::MAX);
        debug_assert!(lb <= ub, "row with inconsistent bounds [{lb}, {ub}]");

        if lb > ub {
            // HiGHS segfaults in its IIS routine on a row with `lb > ub` (see `add_rows`).
            // An empty row with a positive lower bound states the same unconditional infeasibility and is handled safely.
            return self.lp_model.add_row(1.0..FloatCst::MAX, std::iter::empty());
        }

        let num_columns = self.num_columns();
        self.lp_model.add_row(
            lb..ub,
            row_coefs.inspect(|(col, _)| debug_assert!(col.index() < num_columns)),
        )
    }

    pub fn add_rows(
        &mut self,
        rows: impl Iterator<
            Item = (
                Option<FloatCst>,
                Option<FloatCst>,
                impl Iterator<Item = (LpCol, FloatCst)>,
            ),
        >,
    ) -> Vec<LpRow> {
        let num_cols = self.num_columns();

        let rows = rows.map(move |(lb, ub, row)| {
            let lb = lb.unwrap_or(FloatCst::MIN);
            let ub = ub.unwrap_or(FloatCst::MAX);

            // HiGHS crashes on a row with `lb > ub`: it reports infeasibility from the bound check before
            // making the matrix column-wise, and its IIS routine then reads out of bounds while building
            // the 0-column IIS. An empty row with a positive lower bound is the same statement
            // ("infeasible regardless of any column") in a shape HiGHS survives.
            let consistent = lb <= ub;
            let bounds = if consistent { lb..=ub } else { 1.0..=FloatCst::MAX };

            // row ends up empty if not `consistent` (everything would be filtered out)
            let row = row
                .inspect(move |(col, _)| debug_assert!(col.index() < num_cols))
                .filter(move |_| consistent);

            (bounds, row)
        });

        let old_num_rows = self.num_rows();
        self.lp_model.add_rows(rows);
        (old_num_rows..self.num_rows()).map(LpRow::from).collect()
    }

    pub fn add_objective_column(
        &mut self,
        main_var: AriesVar,
        coefs: impl Iterator<Item = (LpCol, FloatCst)>,
        sense: LpObjectiveSense,
    ) -> LpCol {
        assert!(self.lp_obj.is_none());

        self.lp_obj = Some(LpObjective {
            col: self.add_column((None, None)),
            main_var,
            sense,
        });
        self.lp_model
            .change_column_cost(self.get_objective_column().unwrap(), 1.);

        let factors = coefs
            .into_iter()
            .chain([(self.get_objective_column().unwrap(), -1.)])
            .collect::<Vec<_>>();

        self.add_row(factors.into_iter(), (Some(0.), Some(0.)));
        self.lp_model.set_sense(sense);

        self.get_objective_column().unwrap()
    }
    pub fn get_objective_column(&self) -> Option<LpCol> {
        self.lp_obj.as_ref().map(|lp_obj| lp_obj.col)
    }
    pub fn get_objective_main_var(&self) -> Option<AriesVar> {
        self.lp_obj.as_ref().map(|lp_obj| lp_obj.main_var)
    }
    pub fn get_objective_sense(&self) -> Option<LpObjectiveSense> {
        self.lp_obj.as_ref().map(|lp_obj| lp_obj.sense)
    }

    /// Restricts one bound of a column, with the cause justifying it, and records what it takes to undo that on backtrack.
    ///
    /// A bound that is already held is left untouched, and one crossing the opposite bound is not applied at all (as that is a contradiction):
    /// the caller is handed the cause back to explain this contradiction using it.
    pub fn set_lp_lit(&mut self, lp_lit: LpLit, cause: BoundCause) -> LpBoundUpdate {
        let col = lp_lit.col;
        let (lower, upper) = self.get_column_bounds(col);

        let held = match lp_lit.tpe {
            LpLitType::GEQ => LpLit::geq(col, lower),
            LpLitType::LEQ => LpLit::leq(col, upper),
        };
        if !lp_lit.strictly_entails(held) {
            return LpBoundUpdate::Unchanged;
        }
        let (new_lower, new_upper) = match lp_lit.tpe {
            LpLitType::GEQ => (lp_lit.val, upper),
            LpLitType::LEQ => (lower, lp_lit.val),
        };
        if new_lower > new_upper {
            return LpBoundUpdate::Emptied(cause);
        }

        let entry = &mut self.col_bounds[col.index()];
        let old_cause = match lp_lit.tpe {
            LpLitType::GEQ => std::mem::replace(&mut entry.lower_cause, cause),
            LpLitType::LEQ => std::mem::replace(&mut entry.upper_cause, cause),
        };
        entry.lower = new_lower;
        entry.upper = new_upper;

        self.lp_trail.push(LpEvent {
            col,
            bound: lp_lit.tpe,
            old_val: held.val,
            old_cause,
        });
        self.update_model_with_column_bounds(col);
        self.update_num_fixed_counted(col, lower == upper);

        LpBoundUpdate::Tightened
    }

    pub fn set_backtrack_point(&mut self) -> DecLvl {
        self.lp_trail.save_state()
    }

    pub fn undo_to_last_backtrack_point(&mut self) {
        let (col_bounds, lp_model, is_counted, num_fixed_counted) = (
            &mut self.col_bounds,
            &mut self.lp_model,
            &self.is_counted,
            &mut self.num_fixed_counted,
        );
        self.lp_trail.restore_last_with(|ev| {
            let entry = &mut col_bounds[ev.col.index()];
            let was_fixed = entry.lower == entry.upper;
            match ev.bound {
                LpLitType::GEQ => {
                    entry.lower = ev.old_val;
                    entry.lower_cause = ev.old_cause;
                }
                LpLitType::LEQ => {
                    entry.upper = ev.old_val;
                    entry.upper_cause = ev.old_cause;
                }
            }
            // (undoing a tightening can only loosen the bounds)
            if is_counted[ev.col.index()] && was_fixed && entry.lower != entry.upper {
                *num_fixed_counted -= 1;
            }
            lp_model.change_column_bounds(ev.col, long_cst_as_float(entry.lower)..=long_cst_as_float(entry.upper));
        });
    }

    pub fn solve_or_iis(&mut self, stats: &mut crate::LpStats) -> Result<LpSolution, LpIis> {
        let time = std::time::Instant::now();

        let res = self.lp_model.solve_or_iis();

        stats.feasibility_checks_time += time.elapsed();
        stats.num_feasibility_checks += 1;

        res
    }
}
