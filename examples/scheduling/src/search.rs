use crate::problem::{Op, Problem};
use aries_backtrack::{Backtrack, DecLvl};
use aries_collections::id_map::IdMap;
use aries_collections::Next;
use aries_core::literals::Disjunction;
use aries_core::*;
use aries_model::extensions::{AssignmentExt, SavedAssignment};
use aries_solver::solver::search::activity::{ActivityBrancher, Heuristic};
use aries_solver::solver::search::{Decision, SearchControl};
use aries_solver::solver::stats::Stats;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Var {
    /// Variable representing the makespan (constrained to be after the end of tasks
    Makespan,
    /// Variable representing the start time of (job_number, task_number_in_job)
    Start(u32, u32),
    Prec(u32, u32, u32, u32),
}

impl std::fmt::Display for Var {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Model = aries_model::Model<Var>;
pub type Solver = aries_solver::solver::Solver<Var>;
pub type ParSolver = aries_solver::parallel_solver::ParSolver<Var>;

/// Search strategies that can be added to the solver.
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum SearchStrategy {
    /// Activity based search with solution guidance
    Activity,
    /// Variable selection based on earliest starting time + least slack
    Est,
    /// Failure directed search
    Fds,
    /// Solution guided: first runs Est strategy until an initial solution is found and tehn switches to activity based search
    Sol,
    /// Run both Activity and Est in parallel.
    Parallel,
}
impl FromStr for SearchStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "act" | "activity" => Ok(SearchStrategy::Activity),
            "est" | "earliest-start" => Ok(SearchStrategy::Est),
            "fds" | "failure-directed" => Ok(SearchStrategy::Fds),
            "sol" | "solution-guided" => Ok(SearchStrategy::Sol),
            "par" | "parallel" => Ok(SearchStrategy::Parallel),
            e => Err(format!("Unrecognized option: '{}'", e)),
        }
    }
}

pub struct ResourceOrderingFirst;
impl Heuristic<Var> for ResourceOrderingFirst {
    fn decision_stage(&self, _var: VarRef, label: Option<&Var>, _model: &aries_model::Model<Var>) -> u8 {
        match label {
            Some(&Var::Prec(_, _, _, _)) => 0, // a reification of (a <= b), decide in the first stage
            Some(&Var::Makespan) | Some(&Var::Start(_, _)) => 1, // delay decisions on the temporal variable to the second stage
            _ => 2,
        }
    }
}

// ============= Forward progression ===========

#[derive(Clone)]
pub struct EstBrancher {
    pb: Problem,
    saved: DecLvl,
}

impl EstBrancher {
    pub fn new(pb: &Problem) -> Self {
        EstBrancher {
            pb: pb.clone(),
            saved: DecLvl::ROOT,
        }
    }
}

impl SearchControl<Var> for EstBrancher {
    fn next_decision(&mut self, _stats: &Stats, model: &Model) -> Option<Decision> {
        // among the task with the smallest "earliest starting time (est)" pick the one that has the least slack
        let best = active_tasks(&self.pb, model).min_by_key(|(_var, est, lst)| (*est, *lst));

        // decision is to set the start time to the selected task to the smallest possible value.
        // if no task was selected, it means that they are all instantiated and we have a complete schedule
        best.map(|(var, est, _)| Decision::SetLiteral(Lit::leq(var, est)))
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<Var> + Send> {
        Box::new(self.clone())
    }
}

impl Backtrack for EstBrancher {
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

/// Returns an iterator over all timepoints that not bound yet.
/// Each item in the iterator is a tuple `(var, est, lst)` where:
///  - `var` is the temporal variable
///  - `est` is its lower bound (the earliest start time of the task)
///  - `lst` is its upper bound (the latest start time of the task)
///  - `est < lst`: the start time of the task has not been decided yet.
fn active_tasks<'a>(pb: &'a Problem, model: &'a Model) -> impl Iterator<Item = (VarRef, IntCst, IntCst)> + 'a {
    pb.operations()
        .iter()
        .copied()
        .filter_map(move |Op { job, op_id, .. }| {
            let v = model.shape.get_variable(&Var::Start(job, op_id)).unwrap();
            let (lb, ub) = model.domain_of(v);
            if lb < ub {
                Some((v, lb, ub))
            } else {
                None
            }
        })
}

// ======= Solution guided =========

#[derive(Clone)]
pub struct SolGuided {
    pb: Problem,
    activity_brancher: ActivityBrancher<Var>,
    has_incumbent_solution: bool,
    saved: DecLvl,
}

impl SolGuided {
    pub fn new(pb: &Problem) -> Self {
        SolGuided {
            pb: pb.clone(),
            activity_brancher: ActivityBrancher::new_with_heuristic(ResourceOrderingFirst),
            has_incumbent_solution: false,
            saved: DecLvl::ROOT,
        }
    }
}

impl SearchControl<Var> for SolGuided {
    fn next_decision(&mut self, _stats: &Stats, model: &Model) -> Option<Decision> {
        if !self.has_incumbent_solution {
            // among the task with the smallest "earliest starting time (est)" pick the one that has the least slack
            let best = active_tasks(&self.pb, model).min_by_key(|(_var, est, lst)| (*est, *lst));

            // decision is to set the start time to the selected task to the smallest possible value.
            // if no task was selected, it means that they are all instantiated and we have a complete schedule
            best.map(|(var, est, _)| Decision::SetLiteral(Lit::leq(var, est)))
        } else {
            self.activity_brancher.next_decision(_stats, model)
        }
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<Var> + Send> {
        Box::new(self.clone())
    }

    fn import_vars(&mut self, model: &Model) {
        self.activity_brancher.import_vars(model)
    }

    /// Notifies the search control that a new assignment has been found (either if itself or by an other solver running in parallel).
    fn new_assignment_found(&mut self, objective_value: IntCst, assignment: std::sync::Arc<SavedAssignment>) {
        self.activity_brancher.new_assignment_found(objective_value, assignment);
        self.has_incumbent_solution = true;
    }

    /// Invoked by search when facing a conflict in the search
    fn conflict(&mut self, clause: &Disjunction, model: &Model) {
        self.activity_brancher.conflict(clause, model);
    }
    fn asserted_after_conflict(&mut self, lit: Lit, model: &Model) {
        self.activity_brancher.asserted_after_conflict(lit, model)
    }
}

impl Backtrack for SolGuided {
    fn save_state(&mut self) -> DecLvl {
        self.activity_brancher.save_state();
        self.saved += 1;
        self.saved
    }

    fn num_saved(&self) -> u32 {
        self.saved.to_int()
    }

    fn restore_last(&mut self) {
        self.activity_brancher.restore_last();
        self.saved -= 1;
    }
}

// =============== Failure-directed search ========

#[derive(Clone)]
enum LastChoice {
    None,
    Dec { dec: Lit, num_unbound: usize, lvl: DecLvl },
}

#[derive(Clone)]
pub struct FDSBrancher {
    saved: DecLvl,
    vars: Vec<VarRef>,
    last: LastChoice,
    ratings: IdMap<VarRef, (f64, f64)>,
    lvl_avg: Vec<(f64, usize)>,
    last_restart: u64,
    next_restart: u64,
}

fn pos(v: VarRef) -> Lit {
    v.geq(1)
}
fn neg(v: VarRef) -> Lit {
    !pos(v)
}

const FDS_DECAY: f64 = 0.9;

impl FDSBrancher {
    pub fn new() -> Self {
        FDSBrancher {
            saved: DecLvl::ROOT,
            vars: Default::default(),
            last: LastChoice::None,
            ratings: Default::default(),
            lvl_avg: vec![],
            last_restart: 0,
            next_restart: 200,
        }
    }

    fn num_unbound(&self, model: &Model) -> usize {
        let mut cnt = 0;
        for v in &self.vars {
            if !model.state.is_bound(*v) {
                cnt += 1;
            }
        }
        cnt
    }

    fn into_global_rating(&mut self, local_rating: f64, lvl: DecLvl) -> f64 {
        let lvl = lvl.to_int() as usize;
        while lvl >= self.lvl_avg.len() {
            self.lvl_avg.push((0_f64, 0))
        }
        let (prev, inum) = self.lvl_avg[lvl];
        let num = inum as f64;
        let new = (prev * num + local_rating) / (num + 1_f64);
        self.lvl_avg[lvl] = (new, inum + 1);
        local_rating / new
    }

    fn set_rating(&mut self, lit: Lit, local_rating: f64, lvl: DecLvl) {
        let new_rating = self.into_global_rating(local_rating, lvl);
        let var = lit.variable();
        let (pos_rating, neg_rating) = self.ratings.get_mut(var).expect("Setting rating for unknown var");
        let mut rating = if lit == pos(var) {
            pos_rating
        } else {
            assert!(lit == neg(var));
            neg_rating
        };
        *rating = (FDS_DECAY * *rating) + (1_f64 - FDS_DECAY) * new_rating;
    }

    fn process_dec(&mut self, model: &Model) {
        if let LastChoice::Dec { dec, num_unbound, lvl } = self.last {
            let curr_lvl = model.state.current_decision_level();
            // assert!(curr_lvl == lvl.next() || curr_lvl == lvl); // in case of an asserted lit, we might be at the same level

            let unbound_after = self.num_unbound(model);
            // assert!(num_unbound > unbound_after);
            if (curr_lvl == lvl.next() || curr_lvl == lvl) && num_unbound > unbound_after {
                let bound_by_choice = num_unbound - unbound_after;
                // R = (2^num_unbound_after) / 2^num_unbound
                // R = 1 / (2^bound_by_choice)
                let rating = 1_f64 + (0.5_f64).powf(bound_by_choice as f64);
                self.set_rating(dec, rating, lvl);
            }
        }
        self.last = LastChoice::None;
    }
    fn process_conflict(&mut self, model: &Model) {
        if let LastChoice::Dec { dec, lvl, .. } = self.last {
            self.set_rating(dec, 0_f64, lvl);
        }
        self.last = LastChoice::None;
    }
}

impl SearchControl<Var> for FDSBrancher {
    fn import_vars(&mut self, model: &aries_model::Model<Var>) {
        if !self.vars.is_empty() {
            return;
        }
        for v in model.state.variables() {
            if let Some(Var::Prec(_, _, _, _)) = model.shape.labels.get(v) {
                self.vars.push(v);
                self.ratings.insert(v, (1.0, 1.0));
            }
        }
    }
    fn next_decision(&mut self, _stats: &Stats, model: &Model) -> Option<Decision> {
        if _stats.num_decisions == self.next_restart {
            let diff = self.next_restart - self.last_restart;
            self.last_restart = _stats.num_decisions;
            self.next_restart = _stats.num_decisions + ((diff as f32) * 1.3_f32) as u64;
            // println!("{}", _stats.num_decisions);
            return Some(Decision::Restart);
        }
        self.process_dec(model);
        // println!("unbound: {}", self.num_unbound(model));
        let mut best_lit = None;
        let mut best_score = f64::INFINITY;
        for &v in &self.vars {
            if !model.state.is_bound(v) {
                let (pos_score, neg_score) = self.ratings[v];
                let var_score = pos_score + neg_score;
                if var_score < best_score {
                    best_score = var_score;
                    best_lit = Some(if pos_score < neg_score { pos(v) } else { neg(v) });
                }
            }
        }
        if let Some(dec) = best_lit {
            // println!("{:?} DEC {:?}", model.state.current_decision_level(), dec);
            self.last = LastChoice::Dec {
                dec,
                num_unbound: self.num_unbound(model),
                lvl: model.state.current_decision_level(),
            };
            Some(Decision::SetLiteral(dec))
        } else {
            for v in model.state.variables() {
                if !model.state.is_bound(v) {
                    let dec = Lit::leq(v, model.state.lb(v));
                    // println!("DEC {:?}", dec);
                    return Some(Decision::SetLiteral(dec));
                }
            }
            None
        }
    }

    /// Notifies the search control that a new assignment has been found (either if itself or by an other solver running in parallel).
    fn new_assignment_found(&mut self, objective_value: IntCst, assignment: std::sync::Arc<SavedAssignment>) {
        // println!("SOL");
    }

    /// Invoked by search when facing a conflict in the search
    fn conflict(&mut self, clause: &Disjunction, model: &Model) {
        // println!("\tconflict {:?}", clause);
        self.process_conflict(model);
        self.last = LastChoice::None;
    }
    fn asserted_after_conflict(&mut self, lit: Lit, model: &Model) {
        // println!("{:?} ASS {:?}", model.state.current_decision_level(), lit);

        assert!(matches!(self.last, LastChoice::None));
        // ignore if this is not a variable on which we might choose, ignore
        if self.ratings.contains_key(lit.variable()) {
            self.last = LastChoice::Dec {
                dec: lit,
                num_unbound: self.num_unbound(model),
                lvl: model.state.current_decision_level(),
            };
        }
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<Var> + Send> {
        Box::new(self.clone())
    }
}

impl Backtrack for FDSBrancher {
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

/// Builds a solver for the given strategy.
pub fn get_solver(base: Solver, strategy: SearchStrategy, pb: &Problem) -> ParSolver {
    let base_solver = Box::new(base);
    let make_act = |s: &mut Solver| s.set_brancher(ActivityBrancher::new_with_heuristic(ResourceOrderingFirst));
    let make_est = |s: &mut Solver| s.set_brancher(EstBrancher::new(pb));
    let make_fds = |s: &mut Solver| s.set_brancher(FDSBrancher::new());
    let make_sol = |s: &mut Solver| s.set_brancher(SolGuided::new(pb));
    match strategy {
        SearchStrategy::Activity => ParSolver::new(base_solver, 1, |_, s| make_act(s)),
        SearchStrategy::Est => ParSolver::new(base_solver, 1, |_, s| make_est(s)),
        SearchStrategy::Fds => ParSolver::new(base_solver, 1, |_, s| make_fds(s)),
        SearchStrategy::Sol => ParSolver::new(base_solver, 1, |_, s| make_sol(s)),
        SearchStrategy::Parallel => ParSolver::new(base_solver, 2, |id, s| match id {
            0 => make_act(s),
            1 => make_est(s),
            // 2 => make_fds(s),
            _ => unreachable!(),
        }),
    }
}
