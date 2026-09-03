use smallvec::SmallVec;

use crate::core::Lit;
use crate::core::literals::{LitSet, StableLitSet};
use crate::lang::ModelView;

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
/// The [`ValidityScope::to_conjunction`] method allows transforming a `ValidityScope` into a conjunction
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
    ///  - replacing each presence literal defined as a conjunction of other presence literal by this conjunction
    ///  - removing from the resulting set any literal that is guarded
    ///  - removing any tautological (i.e. always true) literal from the set
    ///
    /// The `ctx` parameters provides the necessary context to determine:
    ///
    ///  - the decomposition of each scope literal
    ///  - which literals are tautological in the model
    ///  - which scope literal are implied by another one
    pub fn to_conjunction<Ctx: ModelView>(&self, ctx: &Ctx) -> StableLitSet {
        let mut set = LitSet::new();
        for l in self.required_presence.literals() {
            // decomposes a conjunctive scope into its components
            // Thus if a scope `p` was defined as `pa & pb`, we would get the [pa, pb]
            // If the scope was not defined as a conjunction, we would get the conjunction with only `p`
            //
            // This allows computing potentially smaller scopes. For instance if `!pa` is in the guards,
            // the resulting scope would be reduced to `pb` which is only visible if we work on the decomposition.
            let decomposed_scope = ctx.decompose_scope(l);
            for l in decomposed_scope {
                if !ctx.statically_entailed(l) {
                    set.insert(l);
                }
            }
        }

        // at this point `set contains a conjunction of literals `p1 & ... & pn`, such that the expression is defined if all are true.
        //
        // `self.guards` contains a disjunction of literals `g1 | g2 | ... | gn` such that the expression is defined if any is true.
        //
        // For any `pi`, if there is a guard `gj` such that `!pi => gj` then `pi` can be removed from the set.
        // Proof: `pi` does not play any role in the definition of the expression :
        //   - if M |= pi`, the required presence become independent of `pi`
        //   - if M |= !pi` the expression is defined unconditionally
        //
        // Thus we can remove from `set` any elemnt `pi` such that !gj => pi` (for any j)

        let mut unecessary_presence_requirements: SmallVec<[Lit; 8]> = Default::default();
        for &g in &self.guards {
            unecessary_presence_requirements.push(!g);
            unecessary_presence_requirements.extend(ctx.statically_implied_by(!g));
        }
        for guard in unecessary_presence_requirements {
            if set.contains(guard) {
                set.remove(guard, |l| ctx.statically_entailed(l))
            }
        }
        set.into_sorted()
    }
}
