use crate::backtrack::{Backtrack, DecLvl};
use crate::collections::set::IterableRefSet;
use crate::core::literals::{Disjunction, Lits};
use crate::core::state::*;
use crate::core::views::{Boundable, VarView};
use crate::core::*;
use crate::lang::expr::geq;
use crate::model::extensions::{DisjunctionExt, DomainsExt};
use crate::model::{Constraint, Label, Model};
use crate::prelude::LinTerm;
use crate::reasoners::cp::max::{AtLeastOneGeq, MaxElem};
use crate::reasoners::{Contradiction, ReasonerId, Reasoners};
use crate::reif::{DifferenceExpression, ReifExpr, Reifiable};
use crate::solver::musmcs::MusMcsEnumerator;
use crate::solver::musmcs::marco::{MapSolverMode, Marco, SubsetSolverOptiMode};
use crate::solver::parallel::signals::{InputSignal, InputStream, SolverOutput, Synchro};
use crate::solver::search::{Decision, SearchControl, default_brancher};
use crate::solver::stats::Stats;
use crate::utils::cpu_time::StartCycleCount;
use aries_env_param::EnvParam;
use crossbeam_channel::Sender;
use itertools::Itertools;
use std::fmt::Formatter;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// If true, decisions will be logged to the standard output.
static LOG_DECISIONS: EnvParam<bool> = EnvParam::new("ARIES_LOG_DECISIONS", "false");

/// If true: each time a solution is found, the solver's stats will be printed (in optimization)
static STATS_AT_SOLUTION: EnvParam<bool> = EnvParam::new("ARIES_STATS_AT_SOLUTION", "false");

/// If true, the solver will post redundant constraints of linear inequalities with 3 variables, as dynamic edges in the STN.
/// These are primarily useful for detecting cyclic propagations that would not be caught by independent propagation of linear constraints.
static DYNAMIC_EDGES: EnvParam<bool> = EnvParam::new("ARIES_DYNAMIC_EDGES", "true");

/// Macro that uses the the same syntax as `println!()` but:
///  - only evaluate arguments and print if `LOG_DECISIONS` is true.
///  - prepends the thread id to the line.
macro_rules! log_dec {
    // log_dec!("a {} event", "log")
    ($($arg:tt)+) => {
        if LOG_DECISIONS.get() {
            print!("[{:?}] ", std::thread::current().id());
            println!($($arg)+);
        }
    }
}

/// Represents an absolute search limit
#[derive(Clone, Copy, Debug)]
pub enum SearchLimit {
    /// Absolute time before which the search must be concluded
    Deadline(Instant),
    /// Number of decisions before which search must be concluded
    ///
    /// *Note:* this references the number of decisions in the stats and might not be 0 if the solver was already used
    NumDecisions(u64),
    /// Number of conflicts before which search must be concluded
    ///
    /// *Note:* this references the number of decisions in the stats and might not be 0 if the solver was already used
    NumConflicts(u64),
    /// No limit to search.
    None,
}

impl SearchLimit {
    /// Maximum allowed duration *from now*  that the search is allowed to proceed.
    ///
    /// This is immediately converted into a deadline from the current time.
    pub fn duration(dur: Duration) -> Self {
        Self::Deadline(Instant::now() + dur)
    }
}

/// Result of the `search` method.
enum SearchResult {
    /// A solution was found through search and the solver's assignment is on this solution
    AtSolution,
    /// The solver was made aware of a solution from its input channel.
    ExternalSolution(Solution),
    /// The solver has exhausted its search space.
    Unsat(Conflict),
}
pub type UnsatCore = Explanation;

#[derive(Debug)]
pub enum Exit {
    Interrupted,
}
impl std::fmt::Display for Exit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Solver interrupted.")
    }
}
impl std::error::Error for Exit {}

pub struct Solver<Lbl> {
    pub model: Model<Lbl>,
    /// Index of the next constraint to post in the model.
    next_unposted_constraint: usize,
    pub brancher: Box<dyn SearchControl<Lbl> + Send>,
    pub reasoners: Reasoners,
    /// Current decision level at which the solver is at (corresponding to the number of saved states in the trail)
    /// Note that a `DecLvl` may be either the root (`DecLvl::ROOT`), start with an assumption or start with a decision.
    /// All assumption must be immediately after the root, before any decision.
    decision_level: DecLvl,
    /// Last level that can be treated as assumption (including ROOT).
    /// Invariant: `last_assumption_level <= decision_level`
    /// Invariant: there may be no decisions any level below `last_assumption_level`
    last_assumption_level: DecLvl,
    pub stats: Stats,
    /// A data structure with the various communication channels
    /// needed to receive/send updates and commands.
    sync: Synchro,
}
impl<Lbl: Label> Solver<Lbl> {
    pub fn new(model: Model<Lbl>) -> Solver<Lbl> {
        Solver {
            model,
            next_unposted_constraint: 0,
            brancher: default_brancher(),
            reasoners: Reasoners::new(),
            decision_level: DecLvl::ROOT,
            last_assumption_level: DecLvl::ROOT,
            stats: Default::default(),
            sync: Synchro::new(),
        }
    }

    pub fn set_brancher(&mut self, brancher: impl SearchControl<Lbl> + 'static + Send) {
        self.brancher = Box::new(brancher)
    }

    pub fn set_brancher_boxed(&mut self, brancher: Box<dyn SearchControl<Lbl> + 'static + Send>) {
        self.brancher = brancher
    }

    pub fn input_stream(&self) -> InputStream {
        self.sync.input_stream()
    }

    pub fn set_solver_output(&mut self, output: Sender<SolverOutput>) {
        self.sync.set_output(output);
    }

    pub fn enforce<Expr: Reifiable<Lbl>>(&mut self, bool_expr: Expr, scope: impl IntoIterator<Item = Lit>) {
        assert_eq!(self.decision_level, DecLvl::ROOT);
        self.model.enforce_scoped(bool_expr, scope);
    }
    pub fn enforce_all<Expr: Reifiable<Lbl>>(
        &mut self,
        bools: impl IntoIterator<Item = Expr>,
        scope: impl IntoIterator<Item = Lit> + Clone,
    ) {
        assert_eq!(self.decision_level, DecLvl::ROOT);
        self.model.enforce_all(bools, scope);
    }

    /// Interns the given expression and returns an equivalent literal.
    /// The returned literal is *optional* and defined such that it is
    /// present iff the expression is valid (typically meaning that all
    /// variables involved in the expression are present).
    ///
    /// If the expression was already interned, the handle to the previously inserted
    /// instance will be returned.
    pub fn reify<Expr: Reifiable<Lbl>>(&mut self, expr: Expr) -> Lit {
        self.model.reify(expr)
    }

    /// Returns a new literal that, if set to true, will force the given expression to be true.
    /// This is done by posting a half-reified constraint.
    ///
    /// Important: calling the function twice with the same expression will return the same literal.
    pub fn half_reify<Expr: Reifiable<Lbl>>(&mut self, expr: Expr) -> Lit {
        self.model.half_reify(expr)
    }

    /// Immediately adds the given constraint to the appropriate reasoner.
    /// Returns an error if the model become invalid as a result.
    fn post_constraint(&mut self, constraint: &Constraint) -> Result<(), InvalidUpdate> {
        let (constraint, enabler) = match constraint {
            Constraint::Propagator(user_propagator) => {
                // black-box propagator, there is nothing we can do except posting it to the CP solver
                for prop in user_propagator.get_propagators() {
                    self.reasoners.cp.add_propagator(prop);
                }
                return Ok(());
            }
            Constraint::HalfReified(reif_expr, lit) => (reif_expr, lit),
        };

        // if we are here, it means we have a half-reified constrant `enabler => constraint`

        let enabler = *enabler;
        assert_eq!(self.model.state.current_decision_level(), DecLvl::ROOT);
        let scope = self.model.presence_literal(enabler);
        if self.model.entails(!scope) {
            return Ok(()); // constraint is absent, ignore
        }
        if self.model.entails(!enabler) {
            return Ok(()); // constraint is inactive, ignore
        }
        match constraint {
            &ReifExpr::Lit(lit) => {
                self.add_clause([!enabler, lit], scope)?; // value => lit
                Ok(())
            }
            ReifExpr::MaxDiff(diff) => {
                let rhs = diff.a;
                let rhs_add = diff.ub;
                let lhs = diff.b;
                self.reasoners
                    .diff
                    .add_half_reified_edge(enabler, rhs, lhs, rhs_add, &self.model.state);
                Ok(())
            }
            ReifExpr::Eq(a, b) => {
                let lit = self.reasoners.eq.add_edge(*a, *b, &mut self.model);
                if lit != enabler {
                    self.add_clause([!enabler, lit], scope)?; // value => lit
                }
                Ok(())
            }
            ReifExpr::Neq(a, b) => {
                let lit = !self.reasoners.eq.add_edge(*a, *b, &mut self.model);
                if lit != enabler {
                    self.add_clause([!enabler, lit], scope)?; // value => lit
                }
                Ok(())
            }
            ReifExpr::EqVal(a, b) => {
                let (lb, ub) = self.model.state.bounds(*a);
                let lit = if (lb..=ub).contains(b) {
                    self.reasoners.eq.add_val_edge(*a, *b, &mut self.model)
                } else {
                    Lit::FALSE
                };
                if lit != enabler {
                    self.add_clause([!enabler, lit], scope)?; // value => lit
                }
                Ok(())
            }
            ReifExpr::NeqVal(a, b) => {
                let lit = !self.reasoners.eq.add_val_edge(*a, *b, &mut self.model);
                if lit != enabler {
                    self.add_clause([!enabler, lit], scope)?; // value => lit
                }
                Ok(())
            }
            ReifExpr::Or(disjuncts) => {
                if self.model.entails(enabler) {
                    self.add_clause(disjuncts, scope)
                } else {
                    // l  <=>  (or a b ...)
                    let mut clause = Lits::with_capacity(disjuncts.len() + 1);
                    //  l => (or a b ...)    <=>   (or (not l) a b ...)
                    clause.push(!enabler);
                    clause.extend_from_slice(disjuncts);
                    if let Some(clause) = Disjunction::new_non_tautological(clause) {
                        self.add_clause(clause, scope)?;
                    }
                    Ok(())
                }
            }
            ReifExpr::And(conjuncts) => {
                if self.model.entails(!enabler) {
                    // (and a b ...)
                    for lit in conjuncts {
                        self.add_clause([lit], scope)?;
                    }
                } else {
                    // (l => (and a b ...))
                    // (l => a) and (l => b) ...
                    for lit in conjuncts {
                        self.add_clause([!enabler, lit], scope)?;
                    }
                }
                Ok(())
            }
            ReifExpr::LinearLeq(lin) => {
                let reformulation = if let Some((cst, [])) = lin.extract() {
                    Some(ReifExpr::Lit((cst <= 0).into()))
                } else if let Some((cst, [(f, v)])) = lin.extract() {
                    debug_assert_ne!(f, 0); // should be guaranteed by `LinSum`
                    // cst + f * v <= 0
                    Some(ReifExpr::Lit((f * v).leq(-cst)))
                } else if let Some((cst, [(f1, v1), (f2, v2)])) = lin.extract() {
                    debug_assert_ne!(f1, 0); // should be guaranteed by `LinSum`
                    debug_assert_ne!(f2, 0); // should be guaranteed by `LinSum`
                    if f1 == -f2 {
                        // can be expressed as a differences logic term
                        let (factor, b, a) = if f1 > 0 { (f1, v1, v2) } else { (f2, v2, v1) };
                        debug_assert!(factor > 0);
                        // cst + factor *(b - a) <= 0
                        // (b - a) <= -cst / factor
                        Some(ReifExpr::MaxDiff(DifferenceExpression {
                            b,
                            a,
                            ub: num_integer::div_floor(-cst, factor),
                        }))
                    } else {
                        None
                    }
                } else if let Some((0, [_, _, _])) = lin.extract()
                    && self.model.entails(enabler)
                    && DYNAMIC_EDGES.get()
                {
                    let doms = &self.model.state; // convenient alias
                    // we may be eligible for encoding as a dynamic STN edge
                    // This typically occurs for duration constraints `(start + duration) <= end`
                    //
                    // for all possible ordering of items in the sum, check if it representable as a dynamic STN edge
                    // and if so add it to the STN
                    let permutations = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
                    for [xi, yi, di] in permutations {
                        // we are interested in the form `y - x <= d`
                        // get it from the sum `y + (-x) + (-d) <= 0)
                        let x = -lin.get_var(xi);
                        let y = lin.get_var(yi);
                        let d = -lin.get_var(di);
                        if x.factor != 1 || y.factor != 1 {
                            continue;
                        }
                        if doms.presence(d.var) != doms.presence_literal(enabler) {
                            // presence of the constraint does not match the one of the edge
                            continue;
                        }
                        if !doms.implies(doms.presence(d.var), doms.presence(x.var))
                            || !doms.implies(doms.presence(d.var), doms.presence(y.var))
                        {
                            continue;
                        }
                        // if we get there we are eligible, massage the constraint into the right format and post it
                        let src = x.var;
                        let tgt = y.var;
                        let (ub_var, ub_factor) = if d.factor >= 0 {
                            (SignedVar::plus(d.var), d.factor)
                        } else {
                            (SignedVar::minus(d.var), -d.factor)
                        };
                        // add a dynamic edge to the STN, specifying that `tgt -src <= ub_var * ub_factor`
                        // Each time a new upper bound is inferred on `ub_var` a new edge will temporarily added.
                        self.reasoners.diff.add_dynamic_edge(src, tgt, ub_var, ub_factor, doms)
                    }
                    None // we posted redendant constraint, but this is not an exact reformulation, we need to post the original one as well
                } else {
                    None
                };
                if let Some(reformulated) = reformulation {
                    self.post_constraint(&Constraint::HalfReified(reformulated, enabler))
                } else {
                    self.reasoners
                        .cp
                        .add_half_reif_linear_leq_constraint(lin, enabler, &self.model.state);
                    Ok(())
                }
            }
            ReifExpr::LinearEq(lin) => {
                self.post_constraint(&Constraint::HalfReified(ReifExpr::LinearLeq(lin.clone()), enabler))?;
                self.post_constraint(&Constraint::HalfReified(ReifExpr::LinearLeq(-lin.clone()), enabler))
            }
            ReifExpr::LinearNeq(lin) => {
                let option1 = self.model.half_reify(lin.clone().lt(0));
                let option2 = self.model.half_reify(lin.clone().gt(0));
                self.add_clause([!enabler, option1, option2], scope)?;
                Ok(())
            }
            ReifExpr::Alternative(a) => {
                let prez = |v: Var| self.model.state.presence_literal(v);
                assert!(
                    self.model.entails(enabler),
                    "Unsupported half reified alternative constraints."
                );
                assert_eq!(prez(a.main), prez(enabler.variable()));

                let scope = prez(a.main);
                let presences = a.alternatives.iter().map(|alt| prez(alt.var)).collect_vec();
                // at least one alternative must be present
                self.add_clause(&presences, scope)?;

                // at most one must be present
                for (i, p1) in presences.iter().copied().enumerate() {
                    for &p2 in &presences[i + 1..] {
                        self.add_clause([!p1, !p2], scope)?;
                    }
                }

                for alt in &a.alternatives {
                    let alt_scope = self.model.state.presence_literal(alt.var);
                    debug_assert!(self.model.state.implies(alt_scope, scope));
                    // a.main = alt.var + alt.shift
                    // a.main - alt.var = alt.shift
                    // alt.cst <= a.main - alt.var <= alt.cst
                    // -alt.cst >= alt.var - a.main   &&   a.main - alt.var <= alt.cst
                    let alt_value = self.model.get_tautology_of_scope(alt_scope);
                    self.post_constraint(&Constraint::HalfReified(
                        ReifExpr::MaxDiff(DifferenceExpression::new(alt.var, a.main, -alt.cst)),
                        alt_value,
                    ))?;
                    self.post_constraint(&Constraint::HalfReified(
                        ReifExpr::MaxDiff(DifferenceExpression::new(a.main, alt.var, alt.cst)),
                        alt_value,
                    ))?;
                }

                let prez = |v: Var| self.model.state.presence_literal(v);

                // ub(main) <- max_i { ub(var_i) + cst_i  | prez_i }
                self.reasoners.cp.add_propagator(AtLeastOneGeq {
                    scope,
                    lhs: SignedVar::plus(a.main),
                    elements: a
                        .alternatives
                        .iter()
                        .map(|alt| MaxElem::new(alt.var + alt.cst, prez(alt.var)))
                        .collect_vec(),
                });

                //  lb(main)  <-   min_i {  lb(var_i)  + cst_i | prez_i }
                // -ub(-main) <-   min_i { -ub(-var_i) + cst_i | prez_i }
                // -ub(-main) <- - max_i {  ub(-var_i) + cst_i | prez_i }
                //  ub(-main) <-   max_i {  ub(-var_i) + cst_i | prez_i }
                self.reasoners.cp.add_propagator(AtLeastOneGeq {
                    scope,
                    lhs: SignedVar::minus(a.main),
                    elements: a
                        .alternatives
                        .iter()
                        .map(|alt| MaxElem::new(-alt.var + alt.cst, prez(alt.var)))
                        .collect_vec(),
                });
                Ok(())
            }
            ReifExpr::EqMax(a) => {
                let prez = |v: SignedVar| self.model.state.presence(v);
                assert!(
                    self.model.entails(enabler),
                    "Unsupported half reified eqmax constraints."
                );
                assert_eq!(prez(a.lhs), prez(enabler.variable().into()));

                let scope = prez(a.lhs);
                let presences = a.rhs.iter().map(|alt| prez(alt.var)).collect_vec();
                // at least one alternative must be present
                self.add_clause(&presences, scope)?;

                // POST  forall i    lhs >= rhs[i]   (scope: prez(rhs[i]))
                for item in &a.rhs {
                    let item_scope = self.model.state.presence(item.var);
                    debug_assert!(self.model.state.implies(item_scope, scope));
                    let alt_value = self.model.get_tautology_of_scope(item_scope);
                    // a.lhs >= item.var + item.cst
                    let constraint = geq(a.lhs, item.var + item.cst);
                    self.post_constraint(&Constraint::HalfReified(constraint.into(), alt_value))?;
                }

                let prez = |v: SignedVar| self.model.state.presence(v);

                // POST  OR_i  (prez(rhs[i])  &&  rhs[i] >= lhs)    [scope: prez(lhs)]
                self.reasoners.cp.add_propagator(AtLeastOneGeq {
                    scope,
                    lhs: a.lhs,
                    elements: a
                        .rhs
                        .iter()
                        .map(|elem| MaxElem::new(elem.var + elem.cst, prez(elem.var)))
                        .collect_vec(),
                });

                Ok(())
            }
            ReifExpr::EqVarMulLit(mul) => {
                assert!(
                    self.model.entails(enabler),
                    "Unsupported half reified eqvarmullit constraints."
                );
                self.reasoners.cp.add_eq_var_mul_lit_constraint(mul);
                Ok(())
            }
        }
    }

    /// Adds a disjunctive constraint within the given scope.
    fn add_clause(&mut self, clause: impl Into<Disjunction>, scope: Lit) -> Result<(), InvalidUpdate> {
        // TODO: this is hotspot in the case where the input clause is borrowed (notably when processing an `Or` constraint)
        assert_eq!(self.current_decision_level(), DecLvl::ROOT);
        let mut clause: Disjunction = clause.into();
        // only keep literals that may become true
        clause.retain(|l| !self.model.entails(!l));
        let (propagatable, scope) = self.scoped_disjunction(clause, scope);
        if propagatable.is_empty() {
            return self.model.state.set(!scope, Cause::Encoding).map(|_| ());
        }
        self.reasoners.sat.add_clause_scoped(propagatable.literals(), scope);
        Ok(())
    }

    /// From a disjunction with optional elements, creates a scoped clause that can be safely unit propagated
    /// TODO: this should be unnecessary with the optional-aware propagation of the SAT solver
    pub(in crate::solver) fn scoped_disjunction(
        &self,
        disjuncts: impl Into<Disjunction>,
        scope: Lit,
    ) -> (Disjunction, Lit) {
        let prez = |l: Lit| self.model.presence_literal(l.variable());
        let disjuncts = disjuncts.into();
        if scope == Lit::TRUE {
            return (disjuncts, scope);
        }
        if disjuncts.is_empty() {
            // the disjunction can never be true and thus must be absent
            return (Disjunction::from([!scope]), Lit::TRUE);
        }
        if disjuncts
            .literals()
            .iter()
            .all(|&l| self.model.state.implies(prez(l), scope))
        {
            return (disjuncts, scope); // TODO: couldn't the scope be ommited here?
        }
        let mut disjuncts = disjuncts.into_lits();
        disjuncts.push(!scope);

        (Disjunction::new(disjuncts), Lit::TRUE)
    }

    /// Returns true if all constraints are posted.
    fn all_constraints_posted(&self) -> bool {
        self.next_unposted_constraint == self.model.shape.constraints.len()
    }

    /// Post all constraints of the model that have not been previously posted.
    fn post_constraints(&mut self) -> Result<(), InvalidUpdate> {
        if self.all_constraints_posted() {
            return Ok(()); // fast path that avoids updating metrics
        }
        let start_time = Instant::now();
        let start_cycles = StartCycleCount::now();
        while self.next_unposted_constraint < self.model.shape.constraints.len() {
            let c = &self.model.shape.constraints[self.next_unposted_constraint];
            let c = c.clone(); // we cannot hold a reference because the underlying storage may grow as a result of posting constraints
            self.post_constraint(&c)?;
            self.next_unposted_constraint += 1;
        }
        self.stats.init_time += start_time.elapsed();
        self.stats.init_cycles += start_cycles.elapsed();
        Ok(())
    }

    /// Searches for the first satisfying assignment, returning none if the search
    /// space was exhausted without encountering a solution.
    pub fn solve(&mut self, limit: SearchLimit) -> Result<Option<Solution>, Exit> {
        if self.post_constraints().is_err() {
            return Ok(None);
        }

        match self.search(limit)? {
            SearchResult::AtSolution => Ok(Some(self.model.state.extract_solution())),
            SearchResult::ExternalSolution(s) => Ok(Some(s)),
            SearchResult::Unsat(_) => Ok(None),
        }
    }

    /// Enumerates all possible values for the given variables.
    /// Returns a list of assignments, where each assigment is a vector of values for the variables given as input.
    ///
    /// IMPORTANT: this method will post non-removable clauses to block solutions. So even resetting will not bring
    ///  the solver back to its previous state. The solver should be cloned before calling enumerate if it is
    ///  needed for something else.
    pub fn enumerate(&mut self, variables: &[Var], limit: SearchLimit) -> Result<Vec<Vec<IntCst>>, Exit> {
        let mut valid_assignments = Vec::with_capacity(64);

        // If trivially UNSAT
        if self.post_constraints().is_err() {
            return Ok(valid_assignments);
        }

        let on_new_solution = |domains: &Domains| {
            let assignment = variables.iter().map(|var| domains.lb(*var)).collect();
            valid_assignments.push(assignment);
        };

        self.enumerate_with(variables, on_new_solution, limit)?;
        Ok(valid_assignments)
    }

    /// Enumerates all possible values for the given variables.
    /// Each time a new solution is found the callback is called.
    /// Return `true` if the solver found a solution, `false` otherwise.
    ///
    /// IMPORTANT: this method will post non-removable clauses to block solutions. So even resetting will not bring
    ///  the solver back to its previous state. The solver should be cloned before calling enumerate if it is
    ///  needed for something else.
    pub fn enumerate_with(
        &mut self,
        variables: &[Var],
        mut on_new_solution: impl FnMut(&Domains),
        limit: SearchLimit,
    ) -> Result<bool, Exit> {
        assert_eq!(self.decision_level, DecLvl::ROOT);
        debug_assert!(
            {
                variables
                    .iter()
                    .map(|v| self.model.presence_literal(*v))
                    .filter(|p| *p != Lit::TRUE)
                    .all(|p| variables.contains(&p.variable()))
            },
            "At least one optional variable without its presence variable"
        );

        // Post constraints and exit if trivially UNSAT
        if self.post_constraints().is_err() {
            return Ok(false);
        }
        let mut sat = false;
        loop {
            match self.search(limit)? {
                SearchResult::Unsat(_) => return Ok(sat),
                SearchResult::AtSolution => {
                    // Solution found
                    sat = true;

                    // Record the solution
                    let solution = Arc::new(self.model.state.clone());

                    // Add a clause forbidding it in future solutions
                    let mut clause = Vec::with_capacity(variables.len() * 2);
                    for &v in variables {
                        // forbid any variable present in the solution to all take the same valud
                        // absent variable are ignored (but their presence variable will be considered)
                        if solution.present(v) == Some(true) {
                            let (val, ub) = solution.bounds(v);
                            debug_assert_eq!(val, ub, "The variable is not bound");
                            clause.push(Lit::lt(v, val));
                            clause.push(Lit::gt(v, val));
                        }
                    }

                    on_new_solution(&solution);

                    if let Some(dl) = self.backtrack_level_for_clause(&clause) {
                        self.restore(dl);
                        self.reasoners.sat.add_clause(&clause);
                    } else {
                        return Ok(sat);
                    }
                }
                SearchResult::ExternalSolution(_) => panic!(),
            }
        }
    }

    /// Incremental solving: pushes (assumes and propagates) an assumption literal.
    /// In case of failure (unsatisfiability encountered), returns an unsat core.
    pub fn incremental_push(&mut self, assumption: Lit) -> Result<bool, UnsatCore> {
        if self.last_assumption_level == DecLvl::ROOT {
            match self.propagate_and_backtrack_to_consistent() {
                Ok(()) => (),
                Err(conflict) => {
                    // conflict at root, return empty unsat core
                    debug_assert!(conflict.is_empty());
                    return Err(Explanation::new());
                }
            };
        }
        self.assume_and_propagate(assumption)
    }

    /// Incremental solving: pushes (assumes and propagates) the given assumption literals one by one,
    /// until completion or failure (unsatisfiability encountered). In that case, returns an unsat core,
    /// as well as the provided assumptions that were pushed successfully.
    pub fn incremental_push_all(&mut self, assumptions: Vec<Lit>) -> Result<(), (Vec<Lit>, UnsatCore)> {
        let mut successfully_pushed = vec![];
        for lit in assumptions {
            match self.incremental_push(lit) {
                Ok(_) => successfully_pushed.push(lit),
                Err(unsat_core) => return Err((successfully_pushed, unsat_core)),
            }
        }
        Ok(())
    }

    /// Incremental solving: Removes the last assumption that was pushed and
    /// reverts the solver to the state right before it was pushed.
    pub fn incremental_pop(&mut self) {
        self.reset_search();
        self.restore_last();
    }

    /// Incremental solving: Solves the problem with the assumptions that were pushed.
    /// In case of unsatisfiability, returns an unsat core (composed of these assumptions).
    pub fn incremental_solve(&mut self, limit: SearchLimit) -> Result<Result<Solution, UnsatCore>, Exit> {
        match self.search(limit)? {
            SearchResult::AtSolution => Ok(Ok(self.model.state.extract_solution())),
            SearchResult::ExternalSolution(s) => Ok(Ok(s)),
            SearchResult::Unsat(conflict) => {
                let unsat_core = self
                    .model
                    .state
                    .extract_unsat_core_after_conflict(conflict, &mut self.reasoners);
                Ok(Err(unsat_core))
            }
        }
    }

    /// Solves with the given assumptions.
    /// In case of unsatisfiability, returns an unsat core (composed of these assumptions).
    ///
    /// Invariant: the solver must be at the root decision level (meaning that there must be no prior assumptions on the stack)
    pub fn solve_with_assumptions(
        &mut self,
        assumptions: &[Lit],
        limit: SearchLimit,
    ) -> Result<Result<Solution, UnsatCore>, Exit> {
        // delegate to optimization with a constant objective
        self.minimize_with_assumptions(Var::ZERO, assumptions, limit, |_sol| {})
    }

    /// Returns an iterable datastructure for computing all MUS and MCS.
    ///
    /// - MUS (Minimal Unsatisfiable Subset): a subset of `assumptions` that cannot be true at the same time
    ///   in a solution
    /// - MCS (Minimal Correction Set): a subset of `assumtions` of which at least one must be false
    ///   for the problem to have a solution
    pub fn mus_and_mcs_enumerator(&mut self, assumptions: &[Lit]) -> MusMcsEnumerator<'_, Lbl> {
        Marco::with(
            assumptions.iter().copied(),
            self,
            MapSolverMode::default(),
            SubsetSolverOptiMode::default(),
        )
    }

    /// Searches for a satisfying solution that fulfills the posted assumptions.
    /// The search might start from any node (with or without decisions already taken) and is allowed to undo
    /// any previous decision. However it will maintain all posted assumptions and may only backtrack to the level of the last one
    /// (or ROOT in the absence of assumptions).
    ///
    /// Search will stop when either:
    ///   - the solver is at a solution `Ok(AtSolution)`.
    ///     In this case the solution can be extracted from the current domains.
    ///   - the solver proved unsatisfiability under the current assumption `Ok(Unsat)`.
    ///     In this case, a conflict will be provided from which an UNSAT core can be built.
    ///
    /// The method may return as well when:
    ///   - the solver receives an `Interrupt` message. Result: `Err(Interrupted)`
    ///   - the solver receives an external solution. Result: `Ok(ExternalSolution)`.
    ///     In this case the solver will return the external solution, which is intended to be handled by the caller
    ///     (typically to set up new upper bonds before calling search again).
    ///
    /// Invariant: when exiting, the `search` method will always let the solver in a state where all reasoners are fully propagated.
    /// The only exceptions is redundant clauses received from an external process that may still be pending (but those can be handled in any decision level).
    fn search(&mut self, limit: SearchLimit) -> Result<SearchResult, Exit> {
        assert!(self.all_constraints_posted());
        // make sure brancher has knowledge of all variables.
        self.brancher.import_vars(&self.model);

        let start_time = Instant::now();
        let start_cycles = StartCycleCount::now();
        loop {
            match limit {
                SearchLimit::Deadline(deadline) => {
                    if Instant::now() >= deadline {
                        return Err(Exit::Interrupted);
                    }
                }
                SearchLimit::NumDecisions(max_decs) => {
                    if self.stats.num_decisions >= max_decs {
                        return Err(Exit::Interrupted);
                    }
                }

                SearchLimit::NumConflicts(max_conflicts) => {
                    if self.stats.num_conflicts >= max_conflicts {
                        return Err(Exit::Interrupted);
                    }
                }
                SearchLimit::None => {}
            }
            // first propagate everything to make sure we are in a clean, consistent state
            // note that this method call will have the search backtrack when encountering an inconsistent state
            if let Err(conflict) = self.propagate_and_backtrack_to_consistent() {
                // UNSAT
                self.stats.solve_time += start_time.elapsed();
                self.stats.solve_cycles += start_cycles.elapsed();
                return Ok(SearchResult::Unsat(conflict));
            }

            // in a consistent state, check for any incoming messages that may cause us to exit the search
            let mut requires_new_propagation = false;
            while let Ok(signal) = self.sync.signals.try_recv() {
                match signal {
                    InputSignal::Interrupt => {
                        self.stats.solve_time += start_time.elapsed();
                        self.stats.solve_cycles += start_cycles.elapsed();
                        return Err(Exit::Interrupted);
                    }
                    InputSignal::LearnedClause(cl) => {
                        self.reasoners.sat.add_forgettable_clause(cl.literals());
                        requires_new_propagation = true;
                    }
                    InputSignal::SolutionFound(assignment) => {
                        self.stats.solve_time += start_time.elapsed();
                        self.stats.solve_cycles += start_cycles.elapsed();
                        return Ok(SearchResult::ExternalSolution(assignment));
                    }
                }
            }
            if requires_new_propagation {
                // at least one new redundant clause added, go back directly to propagation
                // to handle it before taking a decision
                continue;
            }
            match self.brancher.next_decision(&self.stats, &self.model) {
                Some(Decision::SetLiteral(lit)) => {
                    // println!("Decision: {}", self.model.fmt(lit));
                    self.decide(lit);
                }
                Some(Decision::Restart) => {
                    self.reset_search();
                    self.stats.add_restart();
                }
                None => {
                    log_dec!("=> SOLUTION");
                    // SAT: consistent + no choices left
                    self.stats.solve_time += start_time.elapsed();
                    self.stats.solve_cycles += start_cycles.elapsed();
                    debug_assert!({
                        self.model.shape.validate(&self.model.state).unwrap();
                        true
                    });
                    return Ok(SearchResult::AtSolution);
                }
            }
        }
    }

    pub fn minimize(
        &mut self,
        objective: impl Into<LinTerm>,
        limit: SearchLimit,
    ) -> Result<Option<(IntCst, Solution)>, Exit> {
        self.minimize_with_callback(objective, |_, _| (), limit)
    }

    pub fn minimize_with_callback(
        &mut self,
        objective: impl Into<LinTerm>,
        on_new_solution: impl FnMut(IntCst, &Solution),
        limit: SearchLimit,
    ) -> Result<Option<(IntCst, Solution)>, Exit> {
        self.optimize_with(objective.into(), true, on_new_solution, limit)
    }

    pub fn maximize(
        &mut self,
        objective: impl Into<LinTerm>,
        limit: SearchLimit,
    ) -> Result<Option<(IntCst, Solution)>, Exit> {
        self.maximize_with_callback(objective, |_, _| (), limit)
    }

    pub fn maximize_with_callback(
        &mut self,
        objective: impl Into<LinTerm>,
        on_new_solution: impl FnMut(IntCst, &Solution),
        limit: SearchLimit,
    ) -> Result<Option<(IntCst, Solution)>, Exit> {
        self.optimize_with(objective.into(), false, on_new_solution, limit)
    }

    fn optimize_with(
        &mut self,
        objective: LinTerm,
        minimize: bool,
        mut on_new_solution: impl FnMut(IntCst, &Solution),
        limit: SearchLimit,
    ) -> Result<Option<(IntCst, Solution)>, Exit> {
        assert_eq!(self.decision_level, DecLvl::ROOT);
        assert_eq!(self.last_assumption_level, DecLvl::ROOT);

        let opt_var = if minimize { objective } else { -objective };
        if let Ok(sol) =
            self.minimize_with_assumptions(opt_var, &[], limit, |sol| on_new_solution(sol.lb(objective), sol))?
        {
            Ok(Some((sol.lb(objective), sol)))
        } else {
            Ok(None)
        }
    }

    /// Solves the minimization problem under the given assumptions.
    /// In case of unsatisfiability, returns an unsat core (composed of these assumptions).
    ///
    /// A callback can be given that will be invoked on each new solution with an improved cost, as soon as it is discovered.
    ///
    /// Invariant: the solver must be at the root decision level (meaning that there must be no prior assumptions/decisions on the stack)
    pub fn minimize_with_assumptions<ObjVar: Boundable<Value = IntCst> + VarView<Value = IntCst>>(
        &mut self,
        objective: ObjVar,
        assumptions: &[Lit],
        limit: SearchLimit,
        mut on_new_solution: impl FnMut(&Solution),
    ) -> Result<Result<Solution, UnsatCore>, Exit> {
        // make sure brancher has knowledge of all variables.
        self.brancher.import_vars(&self.model);

        assert_eq!(self.decision_level, DecLvl::ROOT);

        match self.propagate_and_backtrack_to_consistent() {
            Ok(()) => (),
            Err(conflict) => {
                // conflict at root, return empty unsat core
                debug_assert!(conflict.is_empty());
                return Ok(Err(Explanation::new()));
            }
        };
        for &lit in assumptions {
            if let Err(unsat_core) = self.assume_and_propagate(lit) {
                return Ok(Err(unsat_core));
            }
        }
        // best solution found so far
        let mut best = None;
        loop {
            let sol = match self.search(limit)? {
                SearchResult::AtSolution => {
                    // solver stopped at a solution, this is necessarily an improvement on the best solution found so far
                    // notify other solvers that we have found a new solution
                    let sol = self.model.state.extract_solution();
                    self.sync.notify_solution_found(sol.clone());
                    let objective_value = sol.lb(&objective);
                    if STATS_AT_SOLUTION.get() {
                        println!("*********  New sol: {objective_value} *********");
                        self.print_stats();
                    }
                    on_new_solution(&sol);
                    sol
                }
                SearchResult::ExternalSolution(sol) => sol, // a solution was handed to us by another solver
                SearchResult::Unsat(conflict) => {
                    // exhausted search space under the current quality assumptions
                    // 1) if we have a solution return it
                    // 2) otherwise, the problem is unsatisfiable and we return the corresponding UNSAT core
                    return match best {
                        Some((_obj, best)) => Ok(Ok(best)),
                        None => {
                            let unsat_core = self
                                .model
                                .state
                                .extract_unsat_core_after_conflict(conflict, &mut self.reasoners);
                            Ok(Err(unsat_core))
                        }
                    };
                }
            };

            // determine whether the solution found is an improvement on the previous one (might not be the case if sent by another solver)
            let objective_value = sol.lb(&objective);
            let is_improvement = match best {
                None => true,
                Some((previous_best, _)) => objective_value < previous_best,
            };

            if is_improvement {
                // Notify the brancher that a new solution has been found.
                // This enables the use of LNS-like solution and letting the brancher use the values in the best solution
                // as the preferred ones.
                self.brancher.new_assignment_found(objective_value, &sol);
                self.stats.add_solution(objective_value); // TODO: might consider external solutions

                // save the best solution
                best = Some((objective_value, sol));

                // literal that forces future solutions to improve on this one
                let improvement_literal = objective.leq(objective_value - 1);

                // undo any decision and post a new assumption that the objective is improved
                self.reset_search();
                match self.assume_and_propagate(improvement_literal) {
                    Ok(_) => {}
                    Err(_unsat_core) => return Ok(Ok(best.unwrap().1)), // no way to improve this bound
                }
            }
        }
    }

    /// Hint the brancher about an initial solution
    pub fn warm_start(&mut self, objective_value: IntCst, assignment: &Solution) {
        self.brancher.new_assignment_found(objective_value, assignment);
    }

    pub fn decide(&mut self, decision: Lit) {
        assert!(self.all_constraints_posted());
        self.save_state();
        log_dec!(
            "decision: {:?} -- {:?}     dom:{:?}",
            self.decision_level,
            decision,
            self.model.bounds(decision.variable())
        );
        let res = self.model.state.decide(decision);
        assert_eq!(res, Ok(true), "Decision did not result in a valid modification.");
        self.stats.add_decision(decision)
    }

    /// Posts an assumption on a new decision level and returns an `UnsatCore` if the assumption
    /// is trivially inconsistent with the previous ones (without running any propagation).
    ///
    /// If the assumption is accepted, returns an `Ok(x)` result where `x` is true iff the assumption was not already entailed
    /// (i.e. something changed in the domains).
    pub fn assume(&mut self, assumption: Lit) -> Result<bool, UnsatCore> {
        assert!(self.all_constraints_posted());
        assert_eq!(self.last_assumption_level, self.decision_level);
        debug_assert!(
            self.model.state.decisions().is_empty(),
            "Not allowed to make assumptions after solver already started making decisions (i.e. started solving) !",
        );
        self.save_state();
        self.last_assumption_level = self.decision_level;
        match self.model.state.assume(assumption) {
            Ok(status) => Ok(status),
            Err(invalid_update) => {
                // invalid update, transform it int an unsat core
                Err(self
                    .model
                    .state
                    .extract_unsat_core_after_invalid_assumption(invalid_update, &mut self.reasoners))
            }
        }
    }

    /// Posts an assumptions on a new decision level, run all propagators and returns an `UnsatCore`
    /// if the assumptions turns out to be incompatible with previous ones.
    ///
    /// If the assumption is accepted returns an `Ok(x)` result where is true if the assumption was not already entailed
    /// (i.e. something changed in the domains).
    pub fn assume_and_propagate(&mut self, assumption: Lit) -> Result<bool, UnsatCore> {
        if self.assume(assumption)? {
            // the assumption changed something in the domain
            match self.propagate_and_backtrack_to_consistent() {
                Ok(_) => Ok(true),
                Err(conflict) => Err(self
                    .model
                    .state
                    .extract_unsat_core_after_conflict(conflict, &mut self.reasoners)),
            }
        } else {
            // no changes made by the asssumption
            Ok(false)
        }
    }

    /// Determines the appropriate backtrack level for this clause, i.e., the earliest level at which
    /// the clause is unit.
    ///
    /// If there is more that one violated literal at the latest level, then no literal is asserted
    /// and the bactrack level is set to the ante-last level (might occur with clause sharing).
    ///
    /// If the last level at which the clause is not violated is an assumption level, then the method returns
    /// None, meaning that the problem cannot be made SAT without relaxing the assumptions.
    fn backtrack_level_for_clause(&self, clause: &[Lit]) -> Option<DecLvl> {
        debug_assert_eq!(self.model.state.value_of_clause(clause.iter().copied()), Some(false));
        let last_assumption_dec_lvl = self.last_assumption_level;

        // level of the two latest set element of the clause.
        // Those are set to the last assumption level beyond which we cannot backtrack
        // Note that this may artificially "delay" the propagation of a unit clause to a level
        // beyond the one it became unit. As a result when relaxing assumptions, we may undo the consequences of the propagation
        // even though the clause remains unit. Since this will only occur for learnt clauses it is not a problem
        // for correctness but may result in redundant work.
        let mut max = last_assumption_dec_lvl;
        let mut max_next = last_assumption_dec_lvl;

        for &lit in clause {
            debug_assert!(self.model.state.entails(!lit));
            if let Some(ev) = self.model.state.implying_event(!lit) {
                let dl = self.model.state.trail().decision_level(ev);
                if dl > max {
                    max_next = max;
                    max = dl;
                } else if dl > max_next {
                    max_next = dl;
                }
            }
        }
        debug_assert!(max >= last_assumption_dec_lvl);
        if max == last_assumption_dec_lvl {
            // clause is still violated on the last assumption level
            None
        } else if max == max_next {
            Some(max - 1)
        } else {
            // indicate that we may backtrack to the first level where the clause is unit
            // (but not earlier that the last assumption level)
            debug_assert!(max_next >= last_assumption_dec_lvl);
            Some(max_next)
        }
    }

    /// Integrates a conflicting clause (typically learnt through conflict analysis)
    /// and backtracks to the appropriate level.
    /// As a side effect, the activity of the variables in the clause will be increased.
    /// Returns `false` if the clause is conflicting at the root and thus constitutes a contradiction.
    #[must_use]
    fn add_conflicting_clause_and_backtrack(&mut self, expl: &Conflict) -> bool {
        // // print the clause before analysis
        // println!("conflict ({}) :", expl.literals().len());
        // for &l in expl.literals() {
        //     if !self.model.state.entails(!l) {
        //         print!("  > {}", self.model.fmt(l));
        //     } else {
        //         print!("    {}", self.model.fmt(l));
        //     }
        //     // let prez = self.model.state.presence(l.variable());
        //     // let scope: Vec<Lit> = self
        //     //     .model
        //     //     .state
        //     //     .implications
        //     //     .direct_implications_of(l.variable().geq(1))
        //     //     .collect();
        //     // print!("  / {}   <<<<   ", self.model.fmt(prez));
        //     // print!("  [");
        //     // for prez in scope {
        //     //     print!("  & {}", self.model.fmt(prez));
        //     // }
        //     // println!("]");
        // }
        // println!();
        if let Some(dl) = self.backtrack_level_for_clause(expl.literals()) {
            // inform the brancher that we are in a conflict state
            self.brancher.conflict(expl, &self.model, &mut self.reasoners, dl);
            // backtrack
            self.restore(dl);
            // println!("conflict:");
            // for l in &expl.clause {
            //     println!("  {l:?}  {}  {:?}", self.model.fmt(l), self.model.value_of_literal(l));
            // }
            debug_assert_eq!(self.model.state.value_of_clause(&expl.clause), None);

            if expl.clause.len() == 1 {
                // clauses with a single literal are tautologies and can be given to the dedicated reasoner
                // note: a possible optimization would also be to not backjump to the root (always the case with a such clauses)
                // but instead to the first level where imposing it would not result in a conflict
                self.reasoners.tautologies.add_tautology(expl.clause.literals()[0])
            } else {
                // add clause to sat solver, making sure the asserted literal is set to true
                self.reasoners.sat.add_learnt_clause(expl.clause.literals());
            }

            true
        } else {
            false
        }
    }

    /// Propagate all constraints until reaching a consistent state or proving that there is no such
    /// consistent state (i.e. the problem is UNSAT).
    ///
    /// This will be done by:
    ///  - propagating in the current state
    ///    - return if no conflict was detected
    ///    - otherwise: learn a conflicting clause, backtrack up the decision tree and repeat the process.
    pub fn propagate_and_backtrack_to_consistent(&mut self) -> Result<(), Conflict> {
        loop {
            match self.propagate() {
                Ok(()) => return Ok(()),
                Err(conflict) => {
                    log_dec!(
                        " CONFLICT {:?} (size: {})  >  {}",
                        self.decision_level,
                        conflict.clause.len(),
                        conflict.literals().iter().map(|l| format!("{:?}", l)).format(" | ")
                    );
                    self.sync.notify_learnt(&conflict.clause);
                    if self.add_conflicting_clause_and_backtrack(&conflict) {
                        // we backtracked, loop again to propagate
                    } else {
                        // could not backtrack to a non-conflicting state, UNSAT
                        return Err(conflict);
                    }
                }
            }
        }
    }

    fn lbd(&self, clause: &Conflict, model: &Domains) -> u32 {
        let mut working_lbd_compute = IterableRefSet::new();

        for &l in clause.literals() {
            if !model.entails(!l) {
                // strange case that may occur due to optionals
                let lvl = self.current_decision_level() + 1; // future
                working_lbd_compute.insert(lvl);
            } else {
                let lvl = model.entailing_level(!l);
                if lvl != DecLvl::ROOT {
                    working_lbd_compute.insert(lvl);
                }
            }
        }
        // eprintln!("LBD: {}", working_lbd_compute.len());
        // returns the number of decision levels, and add one to account for the asserted literal
        working_lbd_compute.len() as u32
    }

    /// Fully propagate all constraints until quiescence or a conflict is reached.
    ///
    /// Returns:
    /// - `Ok(())`: if quiescence was reached without finding any conflict
    /// - `Err(clause)`: if a conflict was found. In this case, `clause` is a conflicting cause in the current
    ///   decision level that
    pub fn propagate(&mut self) -> Result<(), Conflict> {
        match self.post_constraints() {
            Ok(()) => {}
            Err(_) => {
                assert_eq!(self.current_decision_level(), DecLvl::ROOT);
                return Err(Conflict::contradiction());
            }
        }
        let global_start = StartCycleCount::now();

        // we might need to do several rounds of propagation to make sur the first inference engines,
        // can react to the deductions of the latest engines.
        loop {
            let num_events_at_start = self.model.state.num_events();

            debug_assert_eq!(
                self.reasoners.writers().iter().next(),
                Some(&ReasonerId::Sat),
                "SAT propagator should propagate first to ensure none of its invariant are violated by others."
            );
            // propagate all theories
            for &i in self.reasoners.writers() {
                let trail_size = self.model.state.trail().len() as u64;
                let theory_propagation_start = StartCycleCount::now();
                self.stats[i].propagation_loops += 1;
                let th = self.reasoners.reasoner_mut(i);

                match th.propagate(&mut self.model.state) {
                    Ok(()) => (),
                    Err(contradiction) => {
                        // counting domain updates must be done immediately, as :w
                        let num_dom_updates = self.model.state.trail().len() as u64 - trail_size;
                        self.stats[i].dom_updates += num_dom_updates;
                        self.stats.num_dom_updates += num_dom_updates;
                        self.brancher.pre_conflict_analysis(&self.model);
                        // contradiction, learn clause and exit
                        let clause = match contradiction {
                            Contradiction::InvalidUpdate(fail) => self
                                .model
                                .state
                                .clause_for_invalid_inferrence(fail, &mut self.reasoners),
                            Contradiction::Explanation(expl) => {
                                self.model.state.refine_explanation(expl, &mut self.reasoners)
                            }
                        };
                        let lbd = self.lbd(&clause, &self.model.state);
                        self.stats
                            .add_conflict(self.current_decision_level(), clause.len(), lbd);
                        self.stats[i].conflicts += 1;
                        self.stats.propagation_time += global_start.elapsed();
                        self.stats[i].propagation_time += theory_propagation_start.elapsed();
                        return Err(clause);
                    }
                }
                self.stats[i].propagation_time += theory_propagation_start.elapsed();
                let num_dom_updates = self.model.state.trail().len() as u64 - trail_size;
                self.stats[i].dom_updates += num_dom_updates;
                self.stats.num_dom_updates += num_dom_updates;
            }

            if num_events_at_start == self.model.state.num_events() {
                // no new events, inferred in this propagation loop, exit.
                break;
            }
        }
        self.stats.propagation_time += global_start.elapsed();
        Ok(())
    }

    pub fn print_stats(&self) {
        println!("{}", self.stats);
        for (i, th) in self.reasoners.theories() {
            println!("====== {i} =====");
            th.print_stats();
        }
    }

    /// Undo any decision that was made.
    /// This results in backtracking to the last assumption level (or to the ROOT if no assumption was made).
    pub fn reset_search(&mut self) {
        self.restore(self.last_assumption_level);
    }
}

impl<Lbl> Backtrack for Solver<Lbl> {
    fn save_state(&mut self) -> DecLvl {
        self.brancher.pre_save_state(&self.model);
        self.decision_level += 1;
        let n = self.decision_level;
        assert_eq!(self.model.save_state(), n);
        assert_eq!(self.brancher.save_state(), n);

        for w in self.reasoners.writers() {
            let th = self.reasoners.reasoner_mut(*w);
            assert_eq!(th.save_state(), n);
        }
        n
    }

    fn num_saved(&self) -> u32 {
        debug_assert!({
            let n = self.decision_level.to_int();
            assert_eq!(self.model.num_saved(), n);
            assert_eq!(self.brancher.num_saved(), n);
            for (_, th) in self.reasoners.theories() {
                assert_eq!(th.num_saved(), n);
            }
            true
        });
        self.decision_level.to_int()
    }

    fn restore_last(&mut self) {
        assert!(self.decision_level > DecLvl::ROOT);
        self.restore(self.decision_level - 1);
    }

    fn restore(&mut self, saved_id: DecLvl) {
        self.decision_level = saved_id;
        if self.last_assumption_level > saved_id {
            self.last_assumption_level = saved_id;
        }
        self.model.restore(saved_id);
        self.brancher.restore(saved_id);
        for w in self.reasoners.writers() {
            let th = self.reasoners.reasoner_mut(*w);
            th.restore(saved_id);
        }
        debug_assert_eq!(self.current_decision_level(), saved_id);
    }
}

impl<Lbl: Label> Clone for Solver<Lbl> {
    fn clone(&self) -> Self {
        Solver {
            model: self.model.clone(),
            next_unposted_constraint: self.next_unposted_constraint,
            brancher: self.brancher.clone_to_box(),
            reasoners: self.reasoners.clone(),
            decision_level: self.decision_level,
            last_assumption_level: self.last_assumption_level,
            stats: self.stats.clone(),
            sync: self.sync.clone(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::core::Lit;
    use crate::core::literals::Disjunction;

    type Model = crate::model::Model<&'static str>;
    type Solver = crate::solver::Solver<&'static str>;

    #[test]
    fn test_scoped_disjunction() {
        let mut m = Model::new();

        let px = m.new_presence_variable(Lit::TRUE, "px").true_lit();
        let x1 = m.new_optional_bvar(px, "x1").true_lit();
        let x2 = m.new_optional_bvar(px, "x2").true_lit();

        let py = m.new_presence_variable(Lit::TRUE, "py").true_lit();
        // let y1 = m.new_optional_bvar(py, "y1").true_lit();
        // let y2 = m.new_optional_bvar(py, "y2").true_lit();

        let pxy = m.get_conjunctive_scope(&[px, py]);
        let xy1 = m.new_optional_bvar(pxy, "xy1").true_lit();
        // let xy2 = m.new_optional_bvar(pxy, "xy2").true_lit();

        let s = &Solver::new(m);

        fn check(
            s: &Solver,
            scope: Lit,
            clause: impl Into<Disjunction>,
            expected: impl Into<Disjunction>,
            expected_scope: Lit,
        ) {
            let clause = clause.into();
            let result = s.scoped_disjunction(clause, scope);
            let expected = expected.into();
            assert_eq!(result, (expected, expected_scope));
        }

        check(s, px, [x1], [x1], px);
        // check(s, T, [!px, x1], [x1]);
        check(s, px, [x1, x2], [x1, x2], px);
        // check(s, T, [!px, x1, x2], [x1, x2]);
        check(s, px, [xy1], [xy1], px); // ??
        check(s, py, [xy1], [xy1], py);
        check(s, pxy, [xy1], [xy1], pxy);
        check(s, pxy, [x1], [!pxy, x1], Lit::TRUE);
        // check(s, T, [!pxy, xy1], [xy1]);
        // check(s, T, [!px, !py, xy1], [xy1]);
        // check(s, T, [!px, !py], [!px, !py]); // !pxy, would be correct as well
    }
}
