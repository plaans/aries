use aries_core::literals::StableLitSet;
use aries_core::*;
use std::collections::HashMap;
use std::sync::Arc;

/// A structure to keep track of the conjunctive scopes that have been defined in the problem.
///
/// A conjunctive scope is created when we want to refer to a subset of the problem that exists
/// iff all required scopes are present.
///
/// For instance, the expression `a <= b` is defined iff both `a` and `b` are *present*.
/// It can be said to exist in the conjunctive scope `presence(a) & presence(b)`.  
#[derive(Clone)]
pub struct Scopes {
    conjunctive_scopes: HashMap<Arc<StableLitSet>, Lit>,
    conjunction_of: HashMap<Lit, Arc<StableLitSet>>,
    /// Associates for each scope literal (the key) an optional literal that is true in this scope.
    /// Invariant for any entry `(k, v)` in the map, `k = presence(v)`
    tautologies: HashMap<Lit, Lit>,
}

impl Scopes {
    pub fn new() -> Self {
        let mut s = Self {
            conjunctive_scopes: Default::default(),
            conjunction_of: Default::default(),
            tautologies: Default::default(),
        };
        s.insert(StableLitSet::EMPTY, Lit::TRUE);
        s
    }

    /// IF defined, return the literal reprensenting the given conjunction.
    pub fn get(&self, conjunction: &StableLitSet) -> Option<Lit> {
        self.conjunctive_scopes.get(conjunction).copied()
    }

    /// If it already exists, return the literal that is always true in the given
    /// scope.
    pub fn get_tautology_of_scope(&self, scope: Lit) -> Option<Lit> {
        self.tautologies.get(&scope).copied()
    }

    /// Record the `tautology` literal as the tautology of the given `scope`.
    ///
    /// It should be the case that `scope = presence(tautology)` that `tautology`
    /// is always true when present.
    pub fn set_tautology_of_scope(&mut self, scope: Lit, tautology: Lit) {
        assert!(!self.tautologies.contains_key(&scope));
        self.tautologies.insert(scope, tautology);
    }

    /// Inserts a new equivalence between a conjunctive scope and a literal.
    ///
    /// # Panics
    ///
    /// Panics if the scope was already associated to a literal.
    pub fn insert(&mut self, conjunction: StableLitSet, literal: Lit) {
        debug_assert!(!self.conjunctive_scopes.contains_key(&conjunction));

        let conjunction = Arc::new(conjunction);
        self.conjunction_of
            .entry(literal)
            .or_insert_with(|| conjunction.clone());
        // ideally, we want to have each literal pointing to its smallest conjunctive set (relaxable)
        debug_assert!(self.conjunction_of[&literal].len() <= conjunction.len(), "Sanity check");

        self.conjunctive_scopes.insert(conjunction, literal);
    }

    /// If the literal was defined as a conjunctive scope, returns the set of literals in the conjunction.
    pub fn conjuncts(&self, lit: Lit) -> Option<impl IntoIterator<Item = Lit> + '_> {
        self.conjunction_of.get(&lit).map(|set| set.literals())
    }
}

impl Default for Scopes {
    fn default() -> Self {
        Self::new()
    }
}
