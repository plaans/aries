use aries_solver::backtrack::{DecLvl, EventIndex, Trail};

use crate::types::*;

#[derive(Debug, Clone)]
struct LpObjective {
    pub col: LpCol,
    pub main_var: AriesVar,
    pub sense: LpObjectiveSense,
}

pub(super) struct LpRelaxState {
    lp_trail: Trail<LpEvent>,
    lp_bound_events: Vec<[Option<EventIndex>; 2]>,

    lp_model: LpModel,
    lp_obj: Option<LpObjective>,
}

impl Clone for LpRelaxState {
    fn clone(&self) -> Self {
        let mut lp_model = self.lp_model.clone();
        set_lp_model_options(&mut lp_model);

        Self {
            lp_trail: self.lp_trail.clone(),
            lp_bound_events: self.lp_bound_events.clone(),
            lp_model,
            lp_obj: self.lp_obj.clone(),
        }
    }
}
impl Default for LpRelaxState {
    fn default() -> Self {
        let mut lp_model = LpModel::default(LpObjectiveSense::Minimise);
        set_lp_model_options(&mut lp_model);

        Self {
            lp_trail: Default::default(),
            lp_bound_events: Default::default(),
            lp_model,
            lp_obj: None,
        }
    }
}
fn set_lp_model_options(lp_model: &mut LpModel) {
    //lp_model.set_option("time_limit", 2.0); // stop after 2 seconds
    lp_model.set_option("parallel", "off"); // use 1 core
    lp_model.set_option("threads", 1); // solve on 1 thread
    lp_model.set_option("iis_strategy", 0); // https://github.com/ERGO-Code/HiGHS/blob/3be639f037e0001b617c59830d3965f246ab5beb/highs/interfaces/highs_c_api.h#L153
}

impl LpRelaxState {
    pub fn trail(&self) -> &Trail<LpEvent> {
        &self.lp_trail
    }

    pub fn num_rows(&self) -> usize {
        self.lp_model.num_rows()
    }
    pub fn num_columns(&self) -> usize {
        self.lp_model.num_columns()
    }
    #[allow(dead_code)]
    pub fn columns(&self) -> Vec<LpCol> {
        (0..self.lp_model.num_columns()).map(LpCol::from).collect()
    }

    pub fn get_column_bounds(&self, col: LpCol) -> (FloatCst, FloatCst) {
        self.lp_model.get_column_bounds(col)
    }
    // fn get_column_lower_bound(&self, col: LpCol) -> IntCst {
    //     float_as_exact_int_cst(self.lp_model.get_column_bounds(col).0)
    // }
    // fn get_column_upper_bound(&self, col: LpCol) -> IntCst {
    //     float_as_exact_int_cst(self.lp_model.get_column_bounds(col).1)
    // }

    pub fn add_column(&mut self, bounds: (Option<FloatCst>, Option<FloatCst>)) -> LpCol {
        let lb = bounds.0.unwrap_or(FloatCst::MIN);
        let ub = bounds.1.unwrap_or(FloatCst::MAX);
        assert!(lb <= ub);

        self.lp_model.add_column(0., lb..ub, []).unwrap()
    }

    pub fn add_columns(&mut self, bounds: impl Iterator<Item = (Option<FloatCst>, Option<FloatCst>)>) -> Vec<LpCol> {
        let bounds = bounds
            .map(|(lb, ub)| (lb.unwrap_or(FloatCst::MIN), ub.unwrap_or(FloatCst::MAX)))
            .collect::<Vec<_>>();

        self.lp_model.add_columns(&bounds).unwrap()
    }

    pub fn tighten_column(&mut self, col: LpCol, bounds: (Option<FloatCst>, Option<FloatCst>)) -> bool {
        let mut tightened = false;

        let (old_lb, old_ub) = self.get_column_bounds(col);

        let lb = if let Some(lb) = bounds.0
            && old_lb < lb
        {
            tightened = true;
            lb
        } else {
            old_lb
        };

        let ub = if let Some(ub) = bounds.1
            && ub < old_ub
        {
            tightened = true;
            ub
        } else {
            old_ub
        };
        assert!(lb <= ub);

        self.lp_model.change_column_bounds(col, lb..=ub);

        tightened
    }

    pub fn change_column(&mut self, col: LpCol, bounds: (Option<FloatCst>, Option<FloatCst>)) {
        let lb = bounds.0.unwrap_or(FloatCst::MIN);
        let ub = bounds.1.unwrap_or(FloatCst::MAX);
        assert!(lb <= ub);

        self.lp_model.change_column_bounds(col, lb..=ub);
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
            return self.lp_model.add_row(1.0..FloatCst::MAX, std::iter::empty()).unwrap();
        }

        let num_columns = self.num_columns();
        self.lp_model
            .add_row(
                lb..ub,
                row_coefs.inspect(|(col, _)| debug_assert!(col.index() < num_columns)),
            )
            .unwrap()
    }

    pub fn add_rows(
        &mut self,
        rows_coefs: Vec<Vec<(LpCol, FloatCst)>>,
        bounds: Vec<(Option<FloatCst>, Option<FloatCst>)>,
    ) -> Vec<LpRow> {
        debug_assert_eq!(rows_coefs.len(), bounds.len());
        debug_assert!(
            rows_coefs
                .iter()
                .all(|row_coefs| row_coefs.iter().all(|(col, _)| col.index() < self.num_columns()))
        );

        let bounds = bounds
            .iter()
            .map(|(lb, ub)| (lb.unwrap_or(FloatCst::MIN), ub.unwrap_or(FloatCst::MAX)))
            .collect::<Vec<_>>();

        // HiGHS crashes on a row with `lb > ub`: it reports infeasibility from the bound check before
        // making the matrix column-wise, and its IIS routine then reads out of bounds while building
        // the 0-column IIS. An empty row with a positive lower bound is the same statement
        // ("infeasible regardless of any column") in a shape HiGHS survives.
        debug_assert!(bounds.iter().all(|&(lb, ub)| lb <= ub), "row with inconsistent bounds");
        if bounds.iter().any(|&(lb, ub)| lb > ub) {
            let mut coefs: Vec<&[(LpCol, FloatCst)]> = Vec::with_capacity(rows_coefs.len());
            let mut bds: Vec<(FloatCst, FloatCst)> = Vec::with_capacity(bounds.len());
            for (row_coefs, &(lb, ub)) in rows_coefs.iter().zip(&bounds) {
                if lb > ub {
                    coefs.push(&[]);
                    bds.push((1., FloatCst::MAX));
                } else {
                    coefs.push(row_coefs.as_slice());
                    bds.push((lb, ub));
                }
            }
            return self.lp_model.add_rows(&bds, &coefs).unwrap();
        }

        self.lp_model.add_rows(&bounds, &rows_coefs).unwrap()
    }

    pub fn add_objective_column(
        &mut self,
        main_var: AriesVar,
        coefs: impl Iterator<Item = (LpCol, FloatCst)>,
        sense: LpObjectiveSense,
    ) -> LpCol {
        assert!(self.lp_obj.is_none());

        self.lp_obj = Some(LpObjective {
            col: self.add_column((None::<FloatCst>, None::<FloatCst>)),
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
    // pub fn get_objective_current_bound(&self) -> Option<FloatCst> {
    //     let obj_col = self.get_objective_column().unwrap();
    //
    //     self.get_objective_sense().map(|sense| {
    //         let bounds = self.get_column_bounds(obj_col);
    //         match sense {
    //             LpObjectiveSense::Maximise => bounds.1,
    //             LpObjectiveSense::Minimise => bounds.0,
    //         }
    //     })
    // }
    pub fn get_objective_column(&self) -> Option<LpCol> {
        self.lp_obj.as_ref().map(|lp_obj| lp_obj.col)
    }
    pub fn get_objective_main_var(&self) -> Option<AriesVar> {
        self.lp_obj.as_ref().map(|lp_obj| lp_obj.main_var)
    }
    pub fn get_objective_sense(&self) -> Option<LpObjectiveSense> {
        self.lp_obj.as_ref().map(|lp_obj| lp_obj.sense)
    }

    /*fn set_lp_lit(
        &mut self,
        lp_lit: LpLit,
        cause: LpEventCause,
        model: &mut Domains,
        identity: aries_solver::reasoners::ReasonerId,
        bindings: &LpLitImplicationBindings,
    ) -> Result<(), Contradiction> {
        debug_assert!(identity == ReasonerId::Extra(0));

        let (lp_event_index, implied_lits) = {
            let prev_lp_lit = match lp_lit.tpe {
                LpLitType::GEQ => LpLit::geq(lp_lit.col, self.get_column_lower_bound(lp_lit.col)),
                LpLitType::LEQ => LpLit::leq(lp_lit.col, self.get_column_upper_bound(lp_lit.col)),
            };
            let bounds = match lp_lit.tpe {
                LpLitType::GEQ => {
                    int_cst_as_float(lp_lit.val)..=int_cst_as_float(self.get_column_upper_bound(lp_lit.col))
                }
                LpLitType::LEQ => {
                    int_cst_as_float(self.get_column_lower_bound(lp_lit.col))..=int_cst_as_float(lp_lit.val)
                }
            };
            if lp_lit.strictly_entails(prev_lp_lit) && bounds.start() <= bounds.end() {
                self.lp_model.change_column_bounds(lp_lit.col, bounds);

                let cause_is_main_model = matches!(cause, LpEventCause::MainModel(_));
                let lpevent_index = self.trail.push(LpEvent::new(lp_lit, cause, prev_lp_lit));

                if !cause_is_main_model {
                    (Some(lpevent_index), bindings.compute_implied_lits(lp_lit))
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
    }*/

    pub fn set_lp_lit(&mut self, lp_lit: LpLit, cause: LpEventCause) -> LpBoundUpdate {
        let col = lp_lit.col;

        let (lb, ub) = {
            let bounds = self.get_column_bounds(col);
            (float_as_exact_int_cst(bounds.0), float_as_exact_int_cst(bounds.1))
        };
        let (prev_lp_lit, new_lb, new_ub) = match lp_lit.tpe {
            LpLitType::GEQ => (LpLit::geq(col, lb), lp_lit.val, ub),
            LpLitType::LEQ => (LpLit::leq(col, ub), lb, lp_lit.val),
        };

        if !lp_lit.strictly_entails(prev_lp_lit) {
            return LpBoundUpdate::Unchanged;
        }
        if new_lb > new_ub {
            return LpBoundUpdate::Emptied(cause);
        }

        self.lp_model
            .change_column_bounds(col, int_cst_as_float(new_lb)..=int_cst_as_float(new_ub));

        if self.lp_bound_events.len() <= col.index() {
            self.lp_bound_events.resize(col.index() + 1, [None, None]);
        }
        let prev_event = self.lp_bound_events[col.index()][lp_lit.tpe.side()];
        let ev = self.lp_trail.push(LpEvent {
            new_lp_lit: lp_lit,
            prev_lp_lit,
            cause,
            prev_event,
        });
        self.lp_bound_events[col.index()][lp_lit.tpe.side()] = Some(ev);

        LpBoundUpdate::Tightened
    }

    /// The first event, among the trail's first `len` ones, from which `lp_lit` holds.
    /// `None` when it held before any event (i.e. initial bound).
    pub fn establishing_event(&self, lp_lit: LpLit, len: usize) -> Option<EventIndex> {
        let mut established = None;
        let mut next = self
            .lp_bound_events
            .get(lp_lit.col.index())
            .and_then(|b| b[lp_lit.tpe.side()]);
        while let Some(i) = next {
            let ev = self.lp_trail.get_event(i);
            if (u32::from(i) as usize) < len {
                if !ev.new_lp_lit.entails(lp_lit) {
                    break; // going back, bounds only get weaker
                }
                established = Some(i);
            }
            next = ev.prev_event;
        }
        established
    }

    pub fn set_backtrack_point(&mut self) -> DecLvl {
        self.lp_trail.save_state()
    }

    pub fn undo_to_last_backtrack_point(&mut self) {
        self.lp_model.clear_solver().unwrap();

        self.lp_trail.restore_last_with(|ev| {
            let bounds = match ev.new_lp_lit.tpe {
                LpLitType::GEQ => {
                    int_cst_as_float(ev.prev_lp_lit.val)..=self.lp_model.get_column_bounds(ev.new_lp_lit.col).1
                }
                LpLitType::LEQ => {
                    self.lp_model.get_column_bounds(ev.new_lp_lit.col).0..=int_cst_as_float(ev.prev_lp_lit.val)
                }
            };
            self.lp_model.change_column_bounds(ev.new_lp_lit.col, bounds);
            self.lp_bound_events[ev.new_lp_lit.col.index()][ev.new_lp_lit.tpe.side()] = ev.prev_event;
        });
    }

    pub fn solve_or_iis(
        &mut self,
        stats: &mut crate::LpRelaxStats,
    ) -> Result<Result<LpSolution, LpIis>, highs::HighsStatus> {
        let time = std::time::Instant::now();

        let res = self.lp_model.solve_or_iis();

        if res.is_err() {
            self.lp_model.clear_solver().unwrap();
        }

        stats.lpruns_time += time.elapsed();
        stats.lpruns += 1;

        res
    }
}
