use aries_core::literals::{LitSet, StableLitSet};
use aries_core::Lit;

/// Represents the scope in which a given expression is valid.
///
/// It is composed of:
/// - `required_presence`: the set of presence literals of all variables that appear in the expression
/// - `guards`: a set of literals such that if one of them is true, the expression is defined.  
///
/// # Example (no guards)
///
/// Consider the expression `a <= b`, where `a` and `b` are optional variable.
/// The value of the expression can only be computed when both `a` and `b` are present,
/// that is when `presence(a) & presence(b)` holds.
///
/// For this example, the scope should be:
/// `ValidityScope = { required_presence: [presence(a), presence(b)], guards = [] }`
///
/// # Example (with guards)
///
/// In some cases however, it might be the case that an expression is valid even when some of
/// its parts are not.
/// For instance, consider the expression `!presence(a) | (a <= 2)`.
/// If `!presence(a)` holds, then the expression evaluates to true regardless of the value of
/// of `(a <= 2)` and in particular when it is undefined due the absence of `a`.
///
/// For this example, the scope should be:
/// `ValidityScope = { required_presence: [presence(a)], guards = [!presence(a)] }`
///
/// # Flattening to a conjunction of literals
///
/// The [`ValidityScope::flatten`] method allows transforming a `ValidityScope` into a conjunction
/// of literals that must hold for the expression to be defined.
#[derive(Debug)]
pub struct ValidityScope {
    required_presence: LitSet,
    guards: Vec<Lit>,
}

impl ValidityScope {
    pub fn new(required: impl IntoIterator<Item = Lit>, guards: impl IntoIterator<Item = Lit>) -> Self {
        Self {
            required_presence: required.into(),
            guards: guards.into_iter().filter(|&l| l != Lit::FALSE).collect(),
        }
    }

    /// Flatten the scope into a conjunction of literals.
    ///
    /// This involves:
    ///  - replacing all presence literal defined as a conjunction of other presence literal by them
    ///  - removing from the resulting set any literal that is guarded
    ///  - removing any tautological (i.e. always true) literal from the set
    ///
    /// # Parameters
    ///
    /// - `flattened`: if the given literal `l` is defined as a conjunction of literals `[l1, l2, ... ln]`,
    ///   it must return a iterator over them. Returns `None` otherwise.
    /// - `tautology`: Returns true if the given literal always holds.   
    pub fn to_conjunction<Lits: IntoIterator<Item = Lit>>(
        &self,
        flattened: impl Fn(Lit) -> Option<Lits>,
        tautology: impl Fn(Lit) -> bool,
    ) -> StableLitSet {
        let mut set = LitSet::new();
        for l in self.required_presence.literals() {
            if let Some(flat) = flattened(l) {
                for l in flat {
                    if !tautology(l) {
                        set.insert(l);
                    }
                }
            } else if !tautology(l) {
                set.insert(l)
            }
        }
        for &guard in &self.guards {
            set.remove(!guard, &tautology)
        }
        set.into_sorted()
    }
}
