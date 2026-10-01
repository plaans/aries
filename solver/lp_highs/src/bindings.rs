use aries_solver::core::literals::Watches;
use aries_solver::core::views::Dom;

use idmap::DirectIdMap;

use crate::types::*;

/// A binding from the main model to an LP column: whenever it applies, the column bound(s) it
/// states are pushed to the LP.
///
/// Bindings only ever go in that direction. The LP is a relaxation of the main model, so a column
/// is constrained *by* the model and never the other way around.
///
/// Both forms carry a `scope` and only apply while it is entailed, which is how the column of an
/// optional part of the model is left free rather than constrained while its presence is still
/// unknown. [`AriesLit::TRUE`] means "no scope".
#[derive(Clone, Copy, Debug)]
pub(super) enum Binding {
    /// A *half* binding: the fixed column bound `lp_lit`, applied as soon as `scope` and `trigger`
    /// are both entailed.
    ///
    /// This is the form to prefer. It is dispatched by a watch on each of its two literals, so it
    /// costs nothing until one of them holds, and the cause it reports is exactly those two
    /// literals: the weakest justification of the bound, rather than whatever the domain happened
    /// to be when it fired.
    Fixed {
        scope: AriesLit,
        trigger: AriesLit,
        lp_lit: LpLit,
    },
    /// A *full* binding: the column mirrors the variable's domain, both bounds following it.
    ///
    /// No finite set of watches can express this over a wide domain, so it is re-evaluated on every
    /// event on `var` (or on the scope's variable). Each bound is justified by the variable's
    /// matching bound at that moment, which is stronger than the trigger of an equivalent
    /// [`Binding::Fixed`] would be, so prefer that one whenever the column only reacts to finitely
    /// many thresholds — and note that this form constrains the column from *both* sides.
    Tracking { scope: AriesLit, var: AriesVar, col: LpCol },
}

impl Binding {
    /// The column bounds this binding currently states, each with the two main-model literals that
    /// justify it. Empty when the binding does not apply.
    fn eval(&self, dom: &impl Dom) -> impl Iterator<Item = (LpLit, AriesLit, AriesLit)> {
        let (lower, upper) = match *self {
            Binding::Fixed { scope, trigger, lp_lit } => (
                (dom.entails(scope) && dom.entails(trigger)).then_some((lp_lit, scope, trigger)),
                None,
            ),
            Binding::Tracking { scope, var, col } => {
                if dom.entails(scope) {
                    let (lb, ub) = (dom.lb(var), dom.ub(var));
                    (
                        Some((LpLit::geq(col, int_cst_as_long(lb)), scope, AriesLit::geq(var, lb))),
                        Some((LpLit::leq(col, int_cst_as_long(ub)), scope, AriesLit::leq(var, ub))),
                    )
                } else {
                    (None, None)
                }
            }
        };
        lower.into_iter().chain(upper)
    }
}

/// The bindings of an [`crate::LpRelax`] reasoner, indexed by what can make them apply.
#[derive(Default, Clone)]
pub(super) struct Bindings {
    bindings: Vec<Binding>,
    /// Watches on the literals of the [`Binding::Fixed`] bindings, the watcher being the index of the binding.
    /// A binding watching two literals is woken by either of them, and its evaluation then checks that the other one holds as well.
    watches: Watches<usize>,
    /// Indices of the [`Binding::Tracking`] bindings, by the variable they mirror.
    on_var: DirectIdMap<AriesVarIdx, Vec<usize>>,
    /// Indices of the [`Binding::Tracking`] bindings, by the variable of their scope.
    on_scope_var: DirectIdMap<AriesVarIdx, Vec<usize>>,
}

impl Bindings {
    pub fn add(&mut self, binding: Binding) {
        let index = self.bindings.len();
        self.bindings.push(binding);

        match binding {
            Binding::Fixed { scope, trigger, .. } => {
                assert!(scope != AriesLit::FALSE && trigger != AriesLit::FALSE);
                // A tautological literal is entailed from the start and is never the subject of an
                // event, so watching it would leave the binding unreachable. It is left unwatched,
                // and `eval` checks entailment anyway.
                for lit in [scope, trigger] {
                    if !lit.tautological() {
                        self.watches.add_watch(index, lit);
                    }
                }
            }
            Binding::Tracking { scope, var, .. } => {
                assert!(scope != AriesLit::FALSE);
                assert!(var != AriesVar::ZERO);
                Self::post(&mut self.on_var, var, index);
                if !scope.tautological() {
                    Self::post(&mut self.on_scope_var, scope.variable(), index);
                }
            }
        }
    }

    fn post(map: &mut DirectIdMap<AriesVarIdx, Vec<usize>>, var: AriesVar, index: usize) {
        let var_idx = var.to_u32();
        if !map.contains_key(var_idx) {
            map.insert(var_idx, Default::default());
        }
        map[var_idx].push(index);
    }

    /// The bindings that `lit` becoming entailed may have turned on: the ones watching it, and the
    /// tracking ones on its variable, whether they mirror it or take it as scope.
    ///
    /// A binding may legitimately be yielded without anything having changed; applying a bound that
    /// already holds is a no-op.
    pub fn eval_on(&self, lit: AriesLit, dom: &impl Dom) -> impl Iterator<Item = (LpLit, AriesLit, AriesLit)> {
        let var_idx = lit.variable().to_u32();

        let tracked = self.on_var.get(var_idx).into_iter().flatten().copied();
        let scoped = self.on_scope_var.get(var_idx).into_iter().flatten().copied();

        self.watches
            .watches_on(lit)
            .chain(tracked)
            .chain(scoped)
            .flat_map(move |index| self.bindings[index].eval(dom))
    }

    /// Every binding that currently applies.
    ///
    /// Needed to seed the LP from the current domains: watches only fire on *future* events, so a
    /// binding registered when its trigger is already entailed would otherwise never apply.
    pub fn eval_all(&self, dom: &impl Dom) -> impl Iterator<Item = (LpLit, AriesLit, AriesLit)> {
        self.bindings.iter().flat_map(move |binding| binding.eval(dom))
    }
}
