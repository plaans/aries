use std::collections::{HashMap, HashSet};

use crate::*;

use super::{conjuncts, fluents_modified_by_actions};

/// A simple argument of a state variable: an action parameter or an object.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Term {
    Param(Sym),
    Object(Sym),
}

impl Term {
    /// Converts the expression to a simple term, if possible.
    pub fn of(expr: ExprId, env: &Environment) -> Option<Term> {
        match env.node(expr).expr() {
            Expr::Param(p) => Some(Term::Param(p.name().clone())),
            Expr::Object(o) => Some(Term::Object(o.name().clone())),
            _ => None,
        }
    }

    /// Converts all expressions to simple terms, if possible.
    pub fn all_of(exprs: &[ExprId], env: &Environment) -> Option<Vec<Term>> {
        exprs.iter().map(|&e| Term::of(e, env)).collect()
    }

    /// False if the two terms are guaranteed to denote distinct objects, whatever the parameters' values.
    pub fn may_unify(&self, other: &Term) -> bool {
        match (self, other) {
            (Term::Object(a), Term::Object(b)) => a == b,
            _ => true,
        }
    }
}

/// Computes over-approximations of the objects that the parameters of an action may take.
///
/// It relies on the parameter types and on *static unary predicates* (e.g. `(ball ?b)` in untyped domains):
/// if an action requires `(ball ?b)` and `ball` is static, `?b` can only be one of the objects that are initially balls.
pub struct ParamDomains<'a> {
    model: &'a Model,
    /// For each static unary predicate, the set of objects for which it holds.
    static_unary: HashMap<FluentId, HashSet<Sym>>,
}

impl<'a> ParamDomains<'a> {
    pub fn new(model: &'a Model) -> Self {
        let env = &model.env;
        let modified = fluents_modified_by_actions(model);
        let mut static_unary: HashMap<FluentId, HashSet<Sym>> = env
            .fluents
            .iter_with_id()
            .filter(|(id, f)| f.return_type == Type::Bool && f.parameters.len() == 1 && !modified.contains(id))
            .map(|(id, _)| (id, HashSet::new()))
            .collect();
        for eff in &model.init {
            let fluent = eff.effect_expression.state_variable.fluent;
            let Some(objects) = static_unary.get_mut(&fluent) else {
                continue;
            };
            match init_atom(eff, env) {
                Some(args) => {
                    if let [Term::Object(o)] = args.as_slice() {
                        objects.insert(o.clone());
                    }
                }
                // not a plain initial fact, do not try to interpret this predicate
                None => {
                    static_unary.remove(&fluent);
                }
            }
        }
        Self { model, static_unary }
    }

    /// Returns the objects that may be bound to the parameter of the action.
    /// If `param` is not a parameter of the action, returns all objects of type `tpe`.
    pub fn domain(&self, action: &Action, param: &Sym, tpe: &Type) -> HashSet<Sym> {
        let env = &self.model.env;
        let tpe = action
            .parameters
            .iter()
            .find(|p| p.name() == param)
            .map(|p| p.tpe())
            .unwrap_or(tpe);
        let mut domain: HashSet<Sym> = env
            .objects
            .iter()
            .filter(|o| Type::from(o.tpe()).is_subtype_of(tpe))
            .map(|o| o.name().clone())
            .collect();
        if action.parameters.iter().any(|p| p.name() == param) {
            for cond in &action.conditions {
                for c in conjuncts(cond.cond, env) {
                    if let Expr::StateVariable(fluent, args) = env.node(c).expr()
                        && let Some(objects) = self.static_unary.get(fluent)
                        && Term::of(args[0], env) == Some(Term::Param(param.clone()))
                    {
                        domain.retain(|o| objects.contains(o));
                    }
                }
            }
        }
        domain
    }
}

/// If the effect is a plain initial fact (unconditional `p(o1, ..., on) := true` at the origin), returns its arguments.
pub fn init_atom(eff: &Effect, env: &Environment) -> Option<Vec<Term>> {
    let e = &eff.effect_expression;
    let is_plain = eff.universal_quantification.is_empty()
        && e.condition.is_none()
        && e.timing == Timestamp::ORIGIN
        && matches!(e.operation, EffectOp::Assign(v) if matches!(env.node(v).expr(), Expr::Bool(true)));
    let args = Term::all_of(&e.state_variable.arguments, env)?;
    (is_plain && args.iter().all(|a| matches!(a, Term::Object(_)))).then_some(args)
}
