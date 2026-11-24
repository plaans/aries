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
        && e.transition_start == Timestamp::ORIGIN
        && matches!(e.operation, EffectOp::Assign(v) if matches!(env.node(v).expr(), Expr::Bool(true)));
    let args = Term::all_of(&e.state_variable.arguments, env)?;
    (is_plain && args.iter().all(|a| matches!(a, Term::Object(_)))).then_some(args)
}

/// A static binary predicate `f(x₀, x₁)` seen as a partial function from its `input` argument to the other one:
/// each object is related to at most one object (e.g. `(on_board ?instrument ?satellite)`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StaticFunction {
    pub fluent: FluentId,
    /// Position of the input argument (0 or 1), the output being the other one.
    pub input: usize,
}

impl StaticFunction {
    pub fn output(&self) -> usize {
        1 - self.input
    }

    /// If the arguments of an atom of this predicate have the given input, returns its output.
    pub fn apply<T: PartialEq + Clone>(&self, atom_args: &[T], input: &T) -> Option<T> {
        (atom_args[self.input] == *input).then(|| atom_args[self.output()].clone())
    }
}

/// The static binary predicates that are functional in the problem, with their value tables.
pub struct StaticFunctions {
    tables: HashMap<StaticFunction, HashMap<Sym, Sym>>,
}

impl StaticFunctions {
    pub fn new(model: &Model) -> Self {
        let env = &model.env;
        let modified = fluents_modified_by_actions(model);
        let mut facts: HashMap<FluentId, Option<Vec<(Sym, Sym)>>> = env
            .fluents
            .iter_with_id()
            .filter(|(id, f)| f.return_type == Type::Bool && f.parameters.len() == 2 && !modified.contains(id))
            .map(|(id, _)| (id, Some(Vec::new())))
            .collect();
        for eff in &model.init {
            let Some(pairs) = facts.get_mut(&eff.effect_expression.state_variable.fluent) else {
                continue;
            };
            match (init_atom(eff, env).as_deref(), pairs.as_mut()) {
                (Some([Term::Object(a), Term::Object(b)]), Some(pairs)) => pairs.push((a.clone(), b.clone())),
                _ => *pairs = None,
            }
        }
        let mut tables = HashMap::new();
        for (fluent, pairs) in facts {
            let Some(pairs) = pairs else { continue };
            for input in [0, 1] {
                let mut table = HashMap::new();
                let functional = pairs.iter().all(|(a, b)| {
                    let (x, y) = if input == 0 { (a, b) } else { (b, a) };
                    table.insert(x.clone(), y.clone()).is_none_or(|prev| prev == *y)
                });
                if functional {
                    tables.insert(StaticFunction { fluent, input }, table);
                }
            }
        }
        StaticFunctions { tables }
    }

    pub fn is_function(&self, f: &StaticFunction) -> bool {
        self.tables.contains_key(f)
    }

    /// Value of the function for the given object (`None` if undefined).
    pub fn eval(&self, f: &StaticFunction, input: &Sym) -> Option<&Sym> {
        self.tables.get(f)?.get(input)
    }
}
