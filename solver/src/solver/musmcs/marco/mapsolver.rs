use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::backtrack::Backtrack;
use crate::core::{Lit, INT_CST_MAX};
use crate::model::extensions::SavedAssignment;
use crate::model::lang::{expr::or, linear::LinearSum, IAtom};
use crate::model::Model;
use crate::solver::search::combinators::CombinatorExt;
use crate::solver::search::lexical::{Lexical, PreferredValue};
use crate::solver::search::SearchControl;
use crate::solver::Exit;

use itertools::Itertools;

use crate::solver::musmcs::{Mcs, Mus};

type Solver = crate::solver::Solver<u8>;
type SolveFn = dyn Fn(&mut Solver) -> Result<Option<Arc<SavedAssignment>>, Exit>;

/// - "High" bias approaches are more likely to discover
///   UNSAT seeds early (since they will be larger), thus favoring finding MUSes early.
/// - "Low" bias approaches are more likely to discover
///   SAT seeds early (since they will be smaller), thus favoring finding MCSes early.
///
/// For each, we make two methods available: 1. using default / preferred values, and 2. optimizing.
/// For high (low) bias, the former corresponds to
/// the solver first choosing value 1 (0) for the literals' variables,
/// while the latter corresponds to maximizing (minimizing)
/// the literals' cardinality / sum of their variables.
/// Both are functionally equivalent, however the preferred values method is expected to be more performant.
#[derive(Default)]
pub enum MapSolverMode {
    None,
    #[default]
    HighPreferredValues,
    LowPreferredValues,
    HighOptimize,
    LowOptimize,
}

pub(crate) struct MapSolver {
    /// The literals whose powerset makes up the search space of the MARCO,
    /// BUT NOT matching those the main solver (aka "subset solver").
    ///
    /// This is because they are "locally" represented in the map solver, differently from the main solver.
    /// As such, they need to be translated between the main solver and map solver.
    ///
    /// Identical to the values of `literals_translate_in` and keys of `literals_translate_out`.
    literals: BTreeSet<Lit>,
    /// A map from the main solver's representation of literals of interest to the map solver's one.
    ///
    /// Is the reverse of `literals_translate_out`.
    literals_translate_in: BTreeMap<Lit, Lit>,
    /// A map from the map solver's representation of literals of interest to the main solver's one.
    ///
    /// Is the reverse of `literals_translate_in`.
    literals_translate_out: BTreeMap<Lit, Lit>,

    solver: Solver,
    /// The exact solving procedure used to discover new seeds.
    solve_fn: Box<SolveFn>,

    /// Singleton MCSes (registered in `block_down`).
    /// Intended for an optional optimisation for the subset solver.
    ///
    /// NOTE: NOT in the local (map solver) representation !!
    /// Stored directly in the main solver's representation !!
    known_singleton_mcses_out: BTreeSet<Lit>,
}

impl MapSolver {
    pub fn new(literals_out: impl IntoIterator<Item = Lit>, map_solver_mode: MapSolverMode) -> Self {
        let mut solver = Solver::new(Model::new());

        let mut literals_translate_in = BTreeMap::<Lit, Lit>::new();
        let mut literals_translate_out = BTreeMap::<Lit, Lit>::new();

        let literals = literals_out
            .into_iter()
            // Discard all duplicates beforehand (so `new_var` isn't called twice for the same thing...)
            .unique()
            .map(|lit_out| {
                let lit_in = solver.model.state.new_var(0, 1).geq(1);
                debug_assert!(!literals_translate_in.contains_key(&lit_out));
                debug_assert!(!literals_translate_out.contains_key(&lit_in));
                literals_translate_in.insert(lit_out, lit_in);
                literals_translate_out.insert(lit_in, lit_out);
                lit_in
            })
            .collect::<BTreeSet<Lit>>();

        let solve_fn: Box<SolveFn> = match map_solver_mode {
            MapSolverMode::None => Box::new(|s: &mut Solver| s.solve()),
            MapSolverMode::HighPreferredValues => {
                let brancher = Lexical::with_vars(literals.iter().map(|&l| l.variable()), PreferredValue::Max)
                    .clone_to_box()
                    .and_then(solver.brancher.clone_to_box());
                solver.set_brancher_boxed(brancher);

                Box::new(move |s: &mut Solver| s.solve())
            }
            MapSolverMode::LowPreferredValues => {
                let brancher = Lexical::with_vars(literals.iter().map(|&l| l.variable()), PreferredValue::Min)
                    .clone_to_box()
                    .and_then(solver.brancher.clone_to_box());
                solver.set_brancher_boxed(brancher);

                Box::new(move |s: &mut Solver| s.solve())
            }
            MapSolverMode::HighOptimize => {
                let sum = LinearSum::of(literals.iter().map(|&l| IAtom::from(l.variable())).collect_vec());
                let obj = IAtom::from(solver.model.state.new_var(0, INT_CST_MAX));
                solver.model.enforce(sum.geq(obj), []);

                Box::new(move |s: &mut Solver| Ok(s.maximize(obj)?.map(|(_, doms)| doms)))
            }
            MapSolverMode::LowOptimize => {
                let sum = LinearSum::of(literals.iter().map(|&l| IAtom::from(l.variable())).collect_vec());
                let obj = IAtom::from(solver.model.state.new_var(0, INT_CST_MAX));
                solver.model.enforce(sum.leq(obj), []);

                Box::new(move |s: &mut Solver| Ok(s.minimize(obj)?.map(|(_, doms)| doms)))
            }
        };

        Self {
            literals,
            literals_translate_in,
            literals_translate_out,
            solver,
            solve_fn,
            known_singleton_mcses_out: BTreeSet::new(),
        }
    }

    /// Translates a (negated) literal of interest from the main solver representation
    /// into (a negated) one in the map solver representation
    fn trin(&self, lit: Lit) -> Lit {
        self.literals_translate_in.get(&lit).copied().unwrap_or_else(||
            // If `lit` is not known, then `!lit` must be. So take the negation of its translation.
            self.literals_translate_in.get(&lit.not()).unwrap().not())
    }

    /// Translates a (negated) literal of interest from the map solver representation
    /// into (a negated) one in the main solver representation.
    fn trout(&self, lit: Lit) -> Lit {
        self.literals_translate_out.get(&lit).copied().unwrap_or_else(||
            // If `lit` is not known, then `!lit` must be. So take the negation of its translation.
            self.literals_translate_out.get(&lit.not()).unwrap().not())
    }

    /// Singleton MCSes (in main solver representation!).
    /// Could optionally be used by the subset solver for optimization.
    pub fn known_singleton_mcses(&self) -> &BTreeSet<Lit> {
        &self.known_singleton_mcses_out
    }

    /// Literals of interest (in main solver representation!)
    /// currently discovered as implied by the given set of assumptions.
    /// Intended for an optional optimisation for the subset solver.
    ///
    /// Necessarily includes the output of `known_singleton_mcses`.
    pub fn known_implications(&mut self, assumptions: &BTreeSet<Lit>) -> BTreeSet<Lit> {
        // Works by:
        // 1. assuming the given literals,
        // 2. propagating them,
        // 3. scanning for literals (or their negation) that are already entailed
        let mut res = BTreeSet::new();

        self.solver.reset();
        for &lit in assumptions {
            if self.solver.assume(self.trin(lit)).is_err() {
                self.solver.reset();
                return res;
            }
        }
        if self.solver.propagate().is_err() {
            self.solver.reset();
            return res;
        }
        for &lit in &self.literals {
            if assumptions.contains(&self.trout(lit)) || assumptions.contains(&self.trout(lit).not()) {
                continue;
            }
            if self.solver.model.state.entails(lit) {
                res.insert(self.trout(lit));
            } else if self.solver.model.state.entails(lit.not()) {
                res.insert(self.trout(lit.not()));
            }
        }
        self.solver.reset();
        res
    }

    /*/// Returns whether the given assignment is valid.
    // Needed for parallel MARCO.
    pub fn seed_is_unexplored(&mut self, seed: &BTreeSet<Lit>) -> bool {
        self.solver.reset();
        let res = self.solver.solve_with_assumptions(seed.iter().copied().collect_vec()).unwrap().is_ok();
        self.solver.reset();
        res
    }*/

    /// Solve for a valid assignment.
    /// In the MARCO algorithm, it will always result in a new assignment, thanks to `block_down` and `block_up`.
    pub fn find_unexplored_seed(&mut self) -> Result<Option<BTreeSet<Lit>>, Exit> {
        match (self.solve_fn)(&mut self.solver)? {
            Some(best_assignment) => {
                let seed = Some(
                    self.literals
                        .iter()
                        .filter(|&&l| best_assignment.entails(l))
                        .map(|&l| self.trout(l))
                        .collect(),
                );
                self.solver.reset();
                Ok(seed)
            }
            None => {
                self.solver.reset();
                Ok(None)
            }
        }
    }

    /// Mark assignments contained in an MSS (i.e. complement of the given MCS) as forbidden.
    /// In other words, mark them as explored. Seeds further discovered won't contain them.
    pub fn block_down(&mut self, mcs: &Mcs) {
        let translated_mcs = mcs.iter().map(|&l| self.trin(l)).collect_vec();
        if let Ok(&singleton_mcs) = translated_mcs.iter().exactly_one() {
            // May only be needed for optional optimisation
            self.known_singleton_mcses_out.insert(self.trout(singleton_mcs));
        }
        self.solver.enforce(or(translated_mcs), []);
    }

    /// Mark assignments containing the given MUS as forbidden.
    /// In other words, mark them as explored. Seeds further discovered won't contain them.
    pub fn block_up(&mut self, mus: &Mus) {
        let translated_mus_negs = mus.iter().map(|&l| self.trin(l).not()).collect_vec();
        self.solver.enforce(or(translated_mus_negs), []);
    }
}
