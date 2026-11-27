//! The bindings of the LP's columns to the main model's literals.
//! They are used both to simplify the problem (with what the main model already entails)
//! and by the LP wrappers to actually bind the LP with the solver / main model.

use smallvec::{SmallVec, smallvec};

use aries_solver::core::{IntCst, Lit, Var, views::Term};

use crate::{IntTerm, SchedEncoder};

use super::LpRelaxEncoder;
use super::problem::ColTag;

/// The bound of its column that a binding sets to its value (the other bound being already there).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundKind {
    Lower,
    Upper,
}

/// A half-binding: column `tag` is to be leq/geq (depending on `bound`) to `value` as soon as all of `trigger` are entailed in the main model
pub(crate) struct Binding {
    pub tag: ColTag,
    pub trigger: SmallVec<[Lit; 3]>,
    pub value: IntCst,
    pub bound: BoundKind,
}

impl LpRelaxEncoder {
    /// The bindings of the lifted presence columns and support columns (with known activation literal), and of the term grounding columns.
    pub fn bindings(&self, ctx: &SchedEncoder) -> Vec<Binding> {
        let presence = |lit: Lit| ctx.store.state.presence(lit);
        let mut bindings = vec![];

        // Lifted presences
        for (source, trans_ids) in self.iter_sources() {
            let (tag, lit) = (ColTag::PresenceSource(source, None), self.get_source_prez(source, ctx));
            bind_to_lit(&mut bindings, tag, lit, presence(lit), true);
            for &trans_id in trans_ids {
                let tag = ColTag::PresenceTransition(trans_id, None);
                let lit = self.transitions.get_prez(trans_id, ctx);
                bind_to_lit(&mut bindings, tag, lit, presence(lit), true);
            }
        }

        // Lifted supports
        //
        // With conditions as out-transitions,
        // an effect supports at most one condition in the LP, but possibly several in the main model:
        // so binding active causal links to 1 would be *unsound* when conditions can be out-transitions.
        let bind_active_links = !self.supports.with_condition_out_transitions;
        for &((out_trans_id, in_trans_id), active) in self.supports.unsorted_out() {
            if let Some(lit) = active {
                let tag = ColTag::Support(out_trans_id, in_trans_id, None);
                bind_to_lit(&mut bindings, tag, lit, presence(lit), bind_active_links);
            }
        }

        // Term groundings
        //
        // 0 when the term's variable can't take the value `x` (or is absent),
        // and 1 when it is present and fixed to `x`.
        for (term, value) in self.iter_distinct_term_assignments() {
            let tag = ColTag::TermGround(term, value);
            let var = term.variable();
            debug_assert!(!term.is_cst() && var != Var::ZERO);

            let Some(x) = var_value_of(term, value) else {
                // No value of the variable makes the term equal to `value`.
                bindings.push(Binding {
                    tag,
                    trigger: smallvec![Lit::TRUE],
                    value: 0,
                    bound: BoundKind::Upper,
                });
                continue;
            };
            let var_prez = ctx.store.state.presence(var);

            let excluding_lits = [
                x.checked_add(1).map(|above| var.geq(above)),
                x.checked_sub(1).map(|below| var.leq(below)),
            ];
            for excluding_lit in excluding_lits.into_iter().flatten() {
                bindings.push(Binding {
                    tag,
                    trigger: smallvec![excluding_lit],
                    value: 0,
                    bound: BoundKind::Upper,
                });
            }
            if var_prez != Lit::TRUE {
                bindings.push(Binding {
                    tag,
                    trigger: smallvec![!var_prez],
                    value: 0,
                    bound: BoundKind::Upper,
                });
            }
            bindings.push(Binding {
                tag,
                trigger: smallvec![var_prez, var.geq(x), var.leq(x)],
                value: 1,
                bound: BoundKind::Lower,
            });
        }

        bindings
    }
}

/// Binds the column `tag` to the literal `lit`, in scope `scope`:
/// to 0 when the literal is false or absent, and, if `bind_one` is true, to 1 when it is true and present.
fn bind_to_lit(bindings: &mut Vec<Binding>, tag: ColTag, lit: Lit, scope: Lit, bind_one: bool) {
    bindings.push(Binding {
        tag,
        trigger: smallvec![!lit],
        value: 0,
        bound: BoundKind::Upper,
    });
    if scope != Lit::TRUE {
        bindings.push(Binding {
            tag,
            trigger: smallvec![!scope],
            value: 0,
            bound: BoundKind::Upper,
        });
    }
    if bind_one {
        bindings.push(Binding {
            tag,
            trigger: smallvec![scope, lit],
            value: 1,
            bound: BoundKind::Lower,
        });
    }
}

/// The value `x` of a term's variable that makes the term `a*x + b` equal to `value`, if any.
///
/// A grounding records values of the *term*, while the literals of the main model are on its variable, hence `x = (value - b) / a`.
/// But most of the time `a` is 1 and `b` is 0.
///
/// `None` means strictly that no integer `x` qualifies, and an arithmetic overflow panics instead of returning `None`.
/// Indeed, if an arithmetic overflow returned `None`, we could unsoundly pin a column to 0.
fn var_value_of(term: IntTerm, value: IntCst) -> Option<IntCst> {
    const OVERFLOW_MSG: &str = "overflow while computing the variable value of a term grounding";

    let factor = term.scaled_var.factor;
    debug_assert!(factor != 0, "a non-constant term shouldn't have a 0 factor");

    // The checked rem / div can only trip on `INT_CST_MIN` with `factor == -1`,
    // as division by zero is excluded by the assert above.
    let num = value.checked_sub(term.constant).expect(OVERFLOW_MSG);
    (num.checked_rem(factor).expect(OVERFLOW_MSG) == 0).then(|| num.checked_div(factor).expect(OVERFLOW_MSG))
}
