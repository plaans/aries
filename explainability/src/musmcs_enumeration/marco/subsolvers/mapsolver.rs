use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use aries::backtrack::Backtrack;
use aries::core::{Lit, SignedVar, VarRef, INT_CST_MAX, INT_CST_MIN};
use aries::model::extensions::SavedAssignment;
use aries::model::lang::{expr::or, linear::LinearSum, IAtom};
use aries::model::Model;
use aries::solver::search::{combinators::CombinatorExt, lexical::Lexical, SearchControl};
use aries::solver::Exit;

use itertools::Itertools;

fn signed_var_of_same_sign(var: VarRef, svar: SignedVar) -> SignedVar {
    match svar.is_plus() {
        true => SignedVar::plus(var),
        false => {
            debug_assert!(svar.is_minus());
            SignedVar::minus(var)
        }
    }
}

type Solver = aries::solver::Solver<u8>;
type SolveFn = dyn Fn(&mut Solver) -> Result<Option<Arc<SavedAssignment>>, Exit>;

pub enum MapSolverMode {
    None,
    OptimizeHigh,
    OptimizeLow,
    PreferredValuesHigh,
    PreferredValuesLow,
}

pub struct MapSolver {
    solver: Solver,
    /// Maps signed(!) variables of the literals representing the soft constraints in
    /// the subset solver to unsigned(!) variables in the map solver (this struct).
    ///
    /// Can be seen as the reverse of `vars_translate_out`.
    /// See documentation for `vars_translate_out` for more details.
    vars_translate_in: BTreeMap<SignedVar, VarRef>,
    /// Maps unsigned(!) variables of the literals representing the soft constraints in
    /// the map solver (this struct) to signed(!) variables in the subset solver.
    ///
    /// Can be seen as the reverse of `vars_translate_in`.
    ///
    /// The reason why we don't do a signed-signed or unsigned-unsigned mapping is that it could result
    /// in a bug where some seeds would not be discovered. For example, the seed `{[a <= 2], [a >= 3]}`
    /// would never be discovered in `find_unexplored_seed`, because it is already trivially unsatisfiable.
    /// On the other hand, if we had two variables `a` and `a'`, this bug wouldn't happen.
    ///
    /// It should be noted that we could, theoretically, use the fact that a seed like `{[a <= 2], [a >= 3]}`
    /// would never be discovered to our advantadge. Indeed, being trivially unsatisfiable,
    /// we could inform the subset solver directly of a trivially unsatisfiable seed,
    /// without even needing to do a solve call to discover it.
    /// However, the implementation for this could be messy / complicated, and would stray
    /// too far from the pseudo-code / "standard" of the MARCO algorithm.
    vars_translate_out: BTreeMap<VarRef, SignedVar>,
    /// These literals represent soft constraints in the map solver (this struct).
    /// Their variables are the same as the keys of `vars_translate_out`.
    literals: BTreeSet<Lit>,
    solve_fn: Box<SolveFn>,
}

impl MapSolver {
    pub fn new(literals: impl IntoIterator<Item = Lit>, solving_mode: MapSolverMode) -> Self {
        let mut model = Model::new();

        let mut vars_translate_in = BTreeMap::<SignedVar, VarRef>::new();
        let mut vars_translate_out = BTreeMap::<VarRef, SignedVar>::new();

        // Literals representing the soft constraints
        let literals = literals
            .into_iter()
            .map(|lit| {
                let v: VarRef = *vars_translate_in
                    .entry(lit.svar())
                    .or_insert_with(|| model.state.new_var(INT_CST_MIN, INT_CST_MAX));
                // IMPORTANT NOTE! Do NOT use `or_insert` instead of `or_insert_with` !
                // As this is going to execute `model.state.new_var(..)` (i.e. create a new variable)
                // EVEN if the entry is non-empty ...!
                vars_translate_out.entry(v).or_insert(lit.svar());
                Lit::new(signed_var_of_same_sign(v, lit.svar()), lit.ub_value())
            })
            .collect::<BTreeSet<Lit>>();

        // TODO cardinality-like constraints ? (another of possible liffiton optimizations)

        let mut solver = Solver::new(model);

        // Approaches for finding / solving for unexplored seeds.
        //
        // "High" bias approaches are more likely to discover UNSAT seeds early (since they will be larger), thus favoring finding MUSes early.
        // "Low" bias approaches are more likely to discover SAT seeds early (since they will be smaller), thus favoring finding MCSes early.
        //
        // This can be done by maximizing (high) or minimizing (low) the cardinality / sum of literals' indicator variables.
        // Another functionally equivalent approach - expected to be more performant - is to inform the solver
        // of our default / preferred values for the literals' indicator variables (0 - low, 1 - high).
        let solve_fn: Box<SolveFn> = match solving_mode {
            // Optimize the sum of the literals' indicator variables. (Maximize for high bias)
            MapSolverMode::OptimizeHigh => {
                let sum = LinearSum::of(
                    literals
                        .iter()
                        .map(|&l| {
                            let v = solver.model.state.new_var(0, 1);
                            solver.model.bind(l, v.geq(1));

                            IAtom::from(v)
                        })
                        .collect_vec(),
                );
                let obj = IAtom::from(solver.model.state.new_var(0, INT_CST_MAX));
                solver.model.enforce(sum.geq(obj), []);

                Box::new(move |s: &mut Solver| {
                    let res = s.maximize(obj)?.map(|(_, doms)| doms);
                    s.reset();
                    Ok(res)
                })
            }
            // Optimize the sum of the literals' indicator variables. (Minimize for low bias)
            MapSolverMode::OptimizeLow => {
                let sum = LinearSum::of(
                    literals
                        .iter()
                        .map(|&l| {
                            let v = solver.model.state.new_var(0, 1);
                            solver.model.bind(l, v.geq(1));

                            IAtom::from(v)
                        })
                        .collect_vec(),
                );
                let obj = IAtom::from(solver.model.state.new_var(0, INT_CST_MAX));
                solver.model.enforce(sum.leq(obj), []);

                Box::new(move |s: &mut Solver| {
                    let res = s.minimize(obj)?.map(|(_, doms)| doms);
                    s.reset();
                    Ok(res)
                })
            }
            // Ask the solver to, if possible, set the literals to true (high bias).
            MapSolverMode::PreferredValuesHigh => {
                let brancher = Lexical::with_vars(
                    literals.iter().map(|&l| {
                        let v = solver.model.state.new_var(0, 1);
                        solver.model.bind(l, v.geq(1));
                        v
                    }),
                    aries::solver::search::lexical::PreferredValue::Max,
                )
                .clone_to_box()
                .and_then(solver.brancher.clone_to_box());

                solver.set_brancher_boxed(brancher);

                Box::new(move |s: &mut Solver| {
                    let res = s.solve()?;
                    s.reset();
                    Ok(res)
                })
            }
            // Ask the solver to, if possible, set the literals to false (low bias).
            MapSolverMode::PreferredValuesLow => {
                let brancher = Lexical::with_vars(
                    literals.iter().map(|&l| {
                        let v = solver.model.state.new_var(0, 1);
                        solver.model.bind(l, v.geq(1));
                        v
                    }),
                    aries::solver::search::lexical::PreferredValue::Min,
                )
                .clone_to_box()
                .and_then(solver.brancher.clone_to_box());

                solver.set_brancher_boxed(brancher);

                Box::new(move |s: &mut Solver| {
                    let res = s.solve()?;
                    s.reset();
                    Ok(res)
                })
            }
            // Unoptimised approach, any solutions.
            MapSolverMode::None => Box::new(move |s: &mut Solver| {
                let res = s.solve()?;
                s.reset();
                Ok(res)
            }),
            _ => panic!(),
        };
        //// FIXME: Could adding the following be any useful for performance ?
        // s.model.enforce(or(literals.iter().copied().collect_vec()), []);

        Self {
            solver,
            vars_translate_in,
            vars_translate_out,
            literals,
            solve_fn,
        }
    }

    /// Translates a soft constraint reification literal from its subset solver representation to its map solver representation.
    fn translate_lit_in(&self, literal: Lit) -> Lit {
        Lit::new(
            signed_var_of_same_sign(self.vars_translate_in[&literal.svar()], literal.svar()),
            literal.ub_value(),
        )
    }

    /// Translates a soft constraint reification literal from its map solver representation to its subset solver representation.
    fn translate_lit_out(&self, literal: Lit) -> Lit {
        Lit::new(self.vars_translate_out[&literal.variable()], literal.ub_value())
    }

    pub fn find_unexplored_seed(&mut self) -> Result<Option<BTreeSet<Lit>>, Exit> {
        match (self.solve_fn)(&mut self.solver)? {
            Some(best_assignment) => {
                let seed = Some(
                    self.literals
                        .iter()
                        .filter(|&&l| best_assignment.entails(l))
                        .map(|&l| self.translate_lit_out(l))
                        .collect(),
                );
                Ok(seed)
            }
            None => Ok(None),
        }
    }

    pub fn block_down(&mut self, sat_subset: &BTreeSet<Lit>) {
        let translated_sat_subset = sat_subset.iter().map(|&l| self.translate_lit_in(l)).collect();
        let translated_sat_subset_complement = self.literals.difference(&translated_sat_subset).copied().collect_vec();
        self.solver.enforce(or(translated_sat_subset_complement), []);
    }

    pub fn block_up(&mut self, unsat_subset: &BTreeSet<Lit>) {
        let translated_unsat_subset_negs = unsat_subset.iter().map(|&l| !self.translate_lit_in(l)).collect_vec();
        self.solver.enforce(or(translated_unsat_subset_negs), []);
    }
}
