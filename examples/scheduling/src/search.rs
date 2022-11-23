mod activity;

use crate::problem::{Op, Problem};
use aries_backtrack::{Backtrack, DecLvl};
use aries_collections::id_map::IdMap;
use aries_collections::Next;
use aries_core::literals::{Disjunction, LitSet};
use aries_core::state::Explainer;
use aries_core::*;
use aries_model::extensions::{AssignmentExt, SavedAssignment};
use aries_solver::solver::search::activity::{ActivityBrancher, BranchingParams, Heuristic};
use aries_solver::solver::search::{Decision, SearchControl};
use aries_solver::solver::stats::Stats;
use std::str::FromStr;

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
    VSIDS,
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
            "vsids" => Ok(SearchStrategy::VSIDS),
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

#[derive(Copy, Clone, Debug)]
pub enum Role {
    Optimizer,
    #[allow(dead_code)]
    Closer,
}

#[derive(Clone)]
pub struct SolGuided {
    pb: Problem,
    activity_brancher: ActivityBrancher<Var>,
    role: Role,
    saved: DecLvl,
    conflict_directed: bool,
}

impl SolGuided {
    pub fn new(pb: &Problem, role: Role, allowed_conflicts: u64, increase_conflict: f32) -> Self {
        let params = BranchingParams {
            prefer_min_value: true,
            allowed_conflicts,
            increase_ratio_for_allowed_conflicts: increase_conflict,
        };
        SolGuided {
            pb: pb.clone(),
            activity_brancher: ActivityBrancher::new_with(params, ResourceOrderingFirst),
            role,
            saved: DecLvl::ROOT,
            conflict_directed: false,
        }
    }
}

impl SearchControl<Var> for SolGuided {
    fn next_decision(&mut self, _stats: &Stats, model: &Model) -> Option<Decision> {
        if !self.conflict_directed {
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

    /// Invoked by search when facing a conflict in the search
    fn conflict(&mut self, clause: &Disjunction, model: &Model, explainer: &mut dyn Explainer) {
        self.conflict_directed = true;
        // bump activity of all variables of the clause
        self.activity_brancher.decay_activities();
        let deep_act = false;
        let mut lits = LitSet::with_capacity(clause.len());
        if deep_act {
            // TODO: very inefficient, does not scale
            let mut queue = Vec::from(clause.clone());
            while let Some(l) = queue.pop() {
                if let Some(Var::Prec(_, _, _, _)) = model.shape.labels.get(l.variable()) {
                    lits.insert(l);
                } else if model.entails(!l) {
                    if let Some(causes) = model.state.implying_literals(!l, explainer) {
                        for l in causes {
                            assert!(model.entails(l));
                            if model.state.implying_event(l).is_some() {
                                // not a root event
                                queue.push(!l);
                            }
                        }
                    }
                } else {
                    lits.insert(l);
                    // println!("IGNORED");
                    // this is the asserted literal
                }
            }
        } else {
            for &b in clause.literals() {
                lits.insert(b);
            }
        }
        let mut lits: Vec<_> = lits
            .literals()
            .map(|l| {
                if model.entails(!l) {
                    (Some(model.state.entailing_level(!l)), l)
                } else {
                    (None, l)
                }
            })
            .collect();

        lits.sort(); // sort by level

        // println!();
        for (_lvl, l) in lits {
            // if let Some(lvl) = lvl {
            //     println!(
            //         "     {lvl:?} {l:?}\t {}",
            //         self.activity_brancher.get_activity(l.variable()),
            //     )
            // } else {
            //     println!("     ->  {l:?}\t {}", self.activity_brancher.get_activity(l.variable()),)
            // }
            self.activity_brancher.bump_activity(l.variable(), model);
        }
    }
    fn asserted_after_conflict(&mut self, lit: Lit, model: &Model) {
        self.activity_brancher.asserted_after_conflict(lit, model)
    }

    fn new_assignment_found(&mut self, objective: IntCst, assignment: std::sync::Arc<SavedAssignment>) {
        self.conflict_directed = true; // switch to activity based

        // if we are in LNS mode and the given solution is better than the previous one,
        // set the default value of all variables to the one they have in the solution.
        let is_improvement = self
            .activity_brancher
            .incumbent_cost()
            .map(|prev| objective < prev)
            .unwrap_or(true);
        if is_improvement {
            self.activity_brancher.set_incumbent_cost(objective);
            for (var, val) in assignment.bound_variables() {
                if val == 0 || val == 1 {
                    // we assume that this is a binary variable
                    match self.role {
                        Role::Optimizer => self.activity_brancher.set_default_value(var, val),
                        Role::Closer => self.activity_brancher.set_default_value(var, 1 - val), // take negation of solution
                    }
                }
            }
        }
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

const FDS_DECAY: f64 = 0.95;

impl FDSBrancher {
    pub fn new() -> Self {
        FDSBrancher {
            saved: DecLvl::ROOT,
            vars: Default::default(),
            last: LastChoice::None,
            ratings: Default::default(),
            lvl_avg: vec![],
            last_restart: 0,
            next_restart: 100,
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
        let rating = if lit == pos(var) {
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
                // let rating = 1_f64;
                self.set_rating(dec, rating, lvl);
            }
        }
        self.last = LastChoice::None;
    }
    fn process_conflict(&mut self, _model: &Model) {
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
            self.next_restart = _stats.num_decisions + ((diff as f32) * 1.5_f32) as u64;
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
    fn new_assignment_found(&mut self, _objective_value: IntCst, _assignment: std::sync::Arc<SavedAssignment>) {
        // println!("SOL");
    }

    /// Invoked by search when facing a conflict in the search
    fn conflict(&mut self, _clause: &Disjunction, model: &Model, _: &mut dyn Explainer) {
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
    let make_vsids = |s: &mut Solver| s.set_brancher(activity::ActivityBrancher::new());
    let make_act = |s: &mut Solver| s.set_brancher(ActivityBrancher::new_with_heuristic(ResourceOrderingFirst));
    let make_est = |s: &mut Solver| s.set_brancher(EstBrancher::new(pb));
    let make_fds = |s: &mut Solver| s.set_brancher(FDSBrancher::new());
    let make_sol = |s: &mut Solver| s.set_brancher(SolGuided::new(pb, Role::Optimizer, 100, 1.05));
    // let make_fds = |s: &mut Solver| s.set_brancher(SolGuided::new(pb, Role::Closer, 100, 1.5));
    match strategy {
        SearchStrategy::VSIDS => ParSolver::new(base_solver, 1, |_, s| make_vsids(s)),
        SearchStrategy::Activity => ParSolver::new(base_solver, 1, |_, s| make_act(s)),
        SearchStrategy::Est => ParSolver::new(base_solver, 1, |_, s| make_est(s)),
        SearchStrategy::Fds => ParSolver::new(base_solver, 1, |_, s| make_fds(s)),
        SearchStrategy::Sol => ParSolver::new(base_solver, 1, |_, s| {
            s.set_brancher(SolGuided::new(pb, Role::Optimizer, 100, 1.5))
        }),
        SearchStrategy::Parallel => ParSolver::new(base_solver, 2, |id, s| match id {
            0 => make_sol(s),
            // 1 => make_fds(s),
            1 => make_vsids(s),
            // 2 => make_fds(s),
            _ => unreachable!(),
        }),
    }
}
