use std::{collections::HashSet, sync::Arc};

use super::{
    PredicateExpr, get_action_effect_exprs, get_global_effect_exprs, iter_action_noneffect_exprs,
    iter_global_noneffect_exprs, try_into_predicate_expr, visit_exprs_recursive_and_apply,
    visit_exprs_recursive_and_check,
};
use crate::{
    Effect, EffectOp, Environment, Expr, ExprId, Fluent, FluentId, Model, Param, RealValue, Sym, TimeRef, Timestamp,
    Type,
};

use itertools::Itertools;

/// Identifies a set of candidate groups of predicates, each of which may be substituted with a state function.
pub fn collect_candidate_substitution_groups(model: &Model) -> Vec<SubstitutionGroup> {
    let mut valid_candidates = vec![];

    // detect groups of 1 predicate
    for (predicate_id, _) in model.env.fluents.iter_with_id() {
        for candidate in PredicateSubstitution::candidates(predicate_id, &model.env) {
            if let Some(group) = SubstitutionGroup::new(&[&candidate], &model.env)
                && group.is_substitutable(model)
            {
                valid_candidates.push(group);
                break;
            }
        }
    }

    let candidates = model
        .env
        .fluents
        .iter_with_id()
        .filter(|&(predicate_id, _)| !is_fluent_static(predicate_id, model))
        .flat_map(|(predicate_id, _)| PredicateSubstitution::candidates(predicate_id, &model.env))
        .collect::<Vec<_>>();

    // detect groups of 2 predicates
    for (i1, c1) in candidates.iter().enumerate() {
        for (i2, c2) in candidates.iter().enumerate() {
            if i1 <= i2 {
                continue;
            }
            if let Some(group) = SubstitutionGroup::new(&[c1, c2], &model.env)
                && group.is_substitutable(model)
            {
                valid_candidates.push(group);
            }
        }
    }
    // detect groups of 3 predicates
    for (i1, c1) in candidates.iter().enumerate() {
        for (i2, c2) in candidates.iter().enumerate() {
            for (i3, c3) in candidates.iter().enumerate() {
                if i1 <= i2 || i2 <= i3 {
                    continue;
                }
                if let Some(group) = SubstitutionGroup::new(&[c1, c2, c3], &model.env)
                    && group.is_substitutable(model)
                {
                    valid_candidates.push(group);
                }
            }
        }
    }

    valid_candidates
}

/// Represents a possible substitution of a (boolean) predicate with a (user-typed) multivalued state function
/// returning one of that predicate's parameters.
///
/// When no such parameter is indicated, the substitution does not change anything and keeps the predicate as it was.
#[derive(Clone)]
pub struct PredicateSubstitution {
    pub predicate_id: FluentId,
    predicate: Arc<Fluent>,
    pub return_param_idx: Option<usize>,
}

impl std::fmt::Debug for PredicateSubstitution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", &self.predicate.name)?;
        let mut first = true;
        for i in 0..self.predicate.parameters.len() {
            if Some(i) != self.return_param_idx {
                if !first {
                    write!(f, ", ")?;
                }
                first = false;
                write!(f, "{i}")?;
            }
        }
        match self.return_param_idx {
            Some(i) => write!(f, ") -> {i}"),
            None => write!(f, ") -> Bool"),
        }
    }
}

impl PredicateSubstitution {
    /// Creates, for the given boolean predicate, a substitution in which the return value is one of its original parameters.
    fn new(predicate_id: FluentId, predicate: &Arc<Fluent>, return_param_idx: usize) -> Self {
        Self {
            predicate_id,
            predicate: predicate.clone(),
            return_param_idx: Some(return_param_idx),
        }
    }
    /// Creates, for the given boolean predicate, a substitution in which the boolean return value is kept (i.e. no changes).
    fn new_not_lifted(predicate_id: FluentId, predicate: &Arc<Fluent>) -> Self {
        Self {
            predicate_id,
            predicate: predicate.clone(),
            return_param_idx: None,
        }
    }

    /// Returns, for the given predicate, a list of potential substitutions.
    pub fn candidates(predicate_id: FluentId, env: &Environment) -> Vec<Self> {
        let predicate = Arc::new(env.fluents.get(predicate_id).clone());

        if predicate.return_type != Type::Bool {
            return vec![];
        }
        let mut candidates: Vec<_> = (0..predicate.parameters.len())
            .rev()
            .map(|i| Self::new(predicate_id, &predicate, i))
            .collect();
        candidates.push(Self::new_not_lifted(predicate_id, &predicate));

        candidates
    }

    pub fn collect_params(&self) -> Vec<(usize, &Type)> {
        self.predicate
            .parameters
            .iter()
            .map(|p| p.tpe())
            .enumerate()
            .filter_map(|(i, tpe)| {
                if Some(i) == self.return_param_idx {
                    None
                } else {
                    Some((i, tpe))
                }
            })
            .collect()
    }

    pub fn get_return_type(&self) -> &Type {
        if let Some(lifted_param_idx) = self.return_param_idx {
            self.predicate.parameters[lifted_param_idx].tpe()
        } else {
            debug_assert!(self.predicate.return_type == Type::Bool);
            &self.predicate.return_type
        }
    }
}

/// A group of predicate substitutions that are syntactically compatible (after potential parameter reorderings)
/// to be applied as one subtitution. Or, in other words, that expressions over these predicates
/// can be subtituted with expressions over one new state function.
pub struct SubstitutionGroup {
    pub substitutions: Vec<PredicateSubstitution>,
    pub reorderings: Vec<ParamsReordering>,
    pub params: Vec<Param>,
    pub return_type: SubstitutionGroupReturnType,
}

#[derive(Debug, Clone)]
pub enum SubstitutionGroupReturnType {
    NewHelperType,
    KnownType(Type),
}

impl std::fmt::Debug for SubstitutionGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in 0..self.substitutions.len() - 1 {
            write!(f, "{:?}, ", self.substitutions[i])?;
        }
        let i = self.substitutions.len() - 1;
        write!(f, "{:?}", self.substitutions[i])?;
        Ok(())
    }
}
impl std::fmt::Display for SubstitutionGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.substitutions.iter().map(|f| &f.predicate.name).join(":").as_str()
        )
    }
}

pub(super) struct ParamsReordering {
    pub permutation: Vec<usize>,
}

impl SubstitutionGroup {
    /// Attempts to create a combination of the given substitutions into a syntactically valid group.
    /// Return None if the substitutions could not be unified.
    fn new(substitutions: &[&PredicateSubstitution], _env: &Environment) -> Option<Self> {
        tracing::trace!("{substitutions:?}");

        // Representative substitution (the one that will be used as basis and for comparisons)
        let repr_sub = substitutions[0];

        if substitutions.iter().any(|sub| sub.predicate.return_type != Type::Bool) {
            tracing::trace!("  not a predicate (non-boolean fluent)");
            return None;
        }
        if substitutions.is_empty() {
            tracing::trace!("  empty group");
            return None;
        }
        if substitutions.len() == 1 && repr_sub.return_param_idx.is_none() {
            tracing::trace!("  singleton group and no lifted return param");
            return None;
        }
        if substitutions.iter().any(|sub| sub.return_param_idx.is_none())
            && substitutions.iter().any(|sub| sub.return_param_idx.is_some())
        {
            tracing::trace!("  mixed group (both with and without lifted return params)");
            return None;
        }
        if substitutions
            .iter()
            .any(|sub| !matches!(sub.get_return_type(), Type::Bool | Type::User(_)))
        {
            tracing::trace!("  non-bool and non-user typed (symbolic) return type");
            return None;
        }

        let repr_params = repr_sub.collect_params();
        let mut reorderings = vec![];

        for sub in substitutions {
            let params = sub.collect_params();

            if repr_params.len() != params.len() {
                tracing::trace!("  incorrect number of parameters");
                return None;
            }
            let mut permutation = Vec::new();
            for (_, expected_type) in &repr_params {
                // we need to find a argument of the fluent with the given type
                let mut found = false;
                for (param_idx, tpe) in &params {
                    if tpe == expected_type && !permutation.contains(param_idx) {
                        permutation.push(*param_idx);
                        found = true;
                        break;
                    }
                }
                if !found {
                    tracing::trace!("  incompatible type of parameters (no {expected_type:?} in {repr_params:?})");
                    return None;
                }
            }
            reorderings.push(ParamsReordering { permutation })
        }

        let return_type = if substitutions.len() == 1 {
            debug_assert!(matches!(repr_sub.get_return_type(), Type::User(_)));
            SubstitutionGroupReturnType::KnownType(repr_sub.get_return_type().clone())
        } else if repr_sub.get_return_type() == &Type::Bool {
            debug_assert!(substitutions.iter().all(|sub| sub.return_param_idx.is_none()));
            debug_assert!(substitutions.iter().all(|sub| sub.get_return_type() == &Type::Bool));
            // A new helper type will be created, with 1 helper object of that type per predicate in the group.
            // This is needed because it could happen that the values required
            // by some expressions on the group's predicates could be impossible to distinguish,
            // notably in the case of groups of 2 or more predicates with no lifted return parameter.
            // It would be enough to use the boolean type in the case of a singleton group, but in the general case.
            // An example of a pathological case is `planning/problems/upf/ipc2004-psr-small-strips/problem.pddl`.
            SubstitutionGroupReturnType::NewHelperType
        } else {
            let return_types: Vec<&Type> = substitutions.iter().map(|sub| sub.get_return_type()).collect();
            debug_assert!(matches!(repr_sub.get_return_type(), Type::User(_)));

            for (i, tpe1) in return_types.iter().enumerate() {
                let Type::User(tpe1) = tpe1 else { unreachable!() };

                for tpe2 in &return_types[i + 1..] {
                    let Type::User(tpe2) = tpe2 else { unreachable!() };

                    let nested = tpe1.is_subtype_of(tpe2) || tpe2.is_subtype_of(tpe1);
                    if !nested && tpe1.overlaps(tpe2) {
                        // Two lifted return parameter types that share values without one being a subtype of the other
                        // cannot be safely unified: a single value in the overlap could originate from any predicate in the group,
                        // so the lifted state variable would no longer let us tell which original predicate was true.
                        // That would silently erase mutex information between the group's predicates.
                        // This is the same issue as above for the boolean case (no lifted return parameters).
                        tracing::trace!("  overlapping non-nested return types");
                        return None;
                    }
                }
            }

            let return_types_to_make_a_union_of = {
                let mut res = Vec::with_capacity(return_types.len());

                for (i, tpe1) in return_types.iter().enumerate() {
                    let Type::User(tpe1) = tpe1 else { unreachable!() };

                    let mut future_duplicate_found = false;
                    for tpe2 in &return_types[i + 1..] {
                        let Type::User(tpe2) = tpe2 else { unreachable!() };

                        if tpe1.to_string() == tpe2.to_string() {
                            future_duplicate_found = true;
                            break;
                        }
                    }
                    if !future_duplicate_found {
                        res.push(tpe1);
                    }
                }
                res
            };
            debug_assert!(!return_types_to_make_a_union_of.is_empty());

            if return_types_to_make_a_union_of.len() == 1 {
                SubstitutionGroupReturnType::KnownType(Type::User(return_types_to_make_a_union_of[0].clone()))
            } else {
                // TODO: let union_type = Type::User(todo!("requires support for 'true' union types"));
                // TODO: SubstitutionGroupReturnType::KnownType(union_type)
                return None; // In the meanwhile...
            }
        };

        Some(Self {
            params: repr_params
                .into_iter()
                .map(|(i, _)| repr_sub.predicate.parameters[i].clone())
                .collect(),
            substitutions: substitutions.iter().map(|&sub| sub.clone()).collect(),
            reorderings,
            return_type,
        })
    }

    /// Returns true if the given predicate is part of the group
    pub fn contains(&self, predicate_id: FluentId) -> bool {
        self.substitutions.iter().any(|sub| sub.predicate_id == predicate_id)
    }

    /// Checks whether the group is substitutable, meaning that of all predicates in the group that may map to a single
    /// ground state variable, only one may be true at any point in time (i.e. they are mutex).
    pub fn is_substitutable(&self, model: &Model) -> bool {
        let _ = tracing::span!(tracing::Level::TRACE, "to-sv", fluent = format!("{self:?}")).entered();

        debug_assert!(
            self.substitutions
                .iter()
                .all(|sub| sub.predicate.return_type == Type::Bool)
        );

        let try_into_simple_args = |predicate_id: FluentId, args: &[ExprId]| -> Option<Vec<SimpleArg>> {
            debug_assert!(self.contains(predicate_id));
            let sub_idx = self
                .substitutions
                .iter()
                .position(|sub| sub.predicate_id == predicate_id)
                .unwrap();

            self.reorderings[sub_idx]
                .permutation
                .iter()
                .map(|&i| match model.env.node(args[i]).expr() {
                    Expr::Real(x) => Some(SimpleArg::Cst(CstArg::Real(*x))),
                    Expr::Bool(x) => Some(SimpleArg::Cst(CstArg::Bool(*x))),
                    Expr::Object(x) => Some(SimpleArg::Cst(CstArg::Object(x.name().clone()))),
                    Expr::Param(x) => Some(SimpleArg::Param(x.name().clone())),
                    _ => None,
                })
                .collect::<Option<_>>()
        };

        // 1. check that globally (outside of actions/tasks):
        //   1.1. there are no negative conditions on any of the group's predicates.
        //        (TODO ? is this requirement liftable when there are "null"/"undefined" values/objects for user types ? TODO)
        //   1.2. check there is at most one positive effect on any of the group's predicates, for the same non-return parameters (outside actions),
        //        and that not all "initial/global" effects on a predicate of the group are set to the default value (i.e. false)
        // 2. check that in actions/tasks:
        //   2.1. there is exactly one positive and negative effect on any of the group's predicates, for the same non-return parameters.
        //   2.2. they (these two effects) cover exactly the same interval or there are no negative conditions on any of the group's predicates.

        // 1.1.

        let closure = |expr_id: ExprId, env: &Environment| {
            match try_into_predicate_expr(expr_id, env) {
                Some(PredicateExpr::Positive(_, predicate_id, args))
                | Some(PredicateExpr::Negative(_, _, predicate_id, args))
                    if self.contains(predicate_id) && try_into_simple_args(predicate_id, &args).is_none() =>
                {
                    // We do not support a predicate of the group to be used in expressions featuring non-simple arguments
                    // (i.e. only support constant or plain parameter symbols, no complex nestings).
                    // We short-circuit and do not consider such groups for substitution.
                    tracing::trace!("non-simple arguments in condition/constraint");
                    return Err(crate::Message::error(""));
                }
                _ => (),
            }
            match try_into_predicate_expr(expr_id, env) {
                Some(PredicateExpr::Negative(_, _, predicate_id, _)) if self.contains(predicate_id) => {
                    tracing::trace!("negative condition/constraint in global exprs");
                    return Err(crate::Message::error(""));
                }
                _ => (),
            }
            Ok(())
        };
        for expr_id in iter_global_noneffect_exprs(model) {
            if visit_exprs_recursive_and_check(expr_id, &closure, &model.env).is_err() {
                return false;
            }
        }

        // 1.2.

        let is_effect_positive_and_coherent = |eff: &Effect, simple_args: &mut HashSet<Vec<SimpleArg>>| -> bool {
            if let EffectOp::Assign(eid) = &eff.effect_expression.operation {
                match model.env.node(*eid).bool() {
                    Ok(true) => {
                        if let Some(args) = try_into_simple_args(
                            eff.effect_expression.state_variable.fluent,
                            &eff.effect_expression.state_variable.arguments,
                        ) {
                            if simple_args.contains(&args) {
                                tracing::trace!(
                                    "more than one positive assignment (for the same non-return parameters) in global effects"
                                );
                                return false;
                            }
                            simple_args.insert(args);
                        }
                    }
                    Ok(false) => {
                        tracing::trace!("negative constant effect");
                        return false;
                    }
                    Err(_) => {
                        tracing::trace!("non-constant effect");
                        return false;
                    }
                }
            } else {
                tracing::trace!("non-assignment effect");
                return false;
            }
            true
        };

        let mut only_defaults = true;
        let mut simple_args = HashSet::new();
        for eff in get_global_effect_exprs(model)
            .iter()
            .filter(|eff| self.contains(eff.effect_expression.state_variable.fluent))
        {
            if try_into_simple_args(
                eff.effect_expression.state_variable.fluent,
                &eff.effect_expression.state_variable.arguments,
            )
            .is_none()
            {
                tracing::trace!("non-simple arguments in effect");
                return false;
            }
            if !is_effect_positive_and_coherent(eff, &mut simple_args) {
                return false;
            }
            only_defaults = false;
        }
        if only_defaults {
            return false;
        }

        // 2.

        for act in model.actions.iter() {
            // preparation for 2.2
            let mut has_negative_condition = false;

            let mut closure = |expr_id: ExprId, env: &Environment| {
                match try_into_predicate_expr(expr_id, env) {
                    Some(PredicateExpr::Positive(_, predicate_id, args))
                    | Some(PredicateExpr::Negative(_, _, predicate_id, args))
                        if self.contains(predicate_id) && try_into_simple_args(predicate_id, &args).is_none() =>
                    {
                        tracing::trace!("non-simple arguments in condition/constraint");
                        return Err(crate::Message::error(""));
                    }
                    _ => (),
                }
                match try_into_predicate_expr(expr_id, env) {
                    Some(PredicateExpr::Negative(_, _, predicate_id, _)) if self.contains(predicate_id) => {
                        has_negative_condition = true;
                    }
                    _ => (),
                }
                Ok(())
            };
            for &expr_id in iter_action_noneffect_exprs(act) {
                if visit_exprs_recursive_and_apply(expr_id, &mut closure, &model.env).is_err() {
                    return false;
                }
            }

            // 2.1 + 2.2.

            for (_, effs) in &act
                .effects
                .iter()
                .filter(|eff| self.contains(eff.effect_expression.state_variable.fluent))
                .chunk_by(|eff| {
                    try_into_simple_args(
                        eff.effect_expression.state_variable.fluent,
                        &eff.effect_expression.state_variable.arguments,
                    )
                })
            {
                let effs: Vec<_> = effs.collect();

                let positives: Vec<_> = effs
                    .iter()
                    .filter(|e| match e.effect_expression.operation {
                        EffectOp::Assign(eid) => matches!(model.env.node(eid).bool(), Ok(true)),
                        _ => false,
                    })
                    .collect();
                let negatives: Vec<_> = effs
                    .iter()
                    .filter(|e| match e.effect_expression.operation {
                        EffectOp::Assign(eid) => matches!(model.env.node(eid).bool(), Ok(false)),
                        _ => false,
                    })
                    .collect();

                // we must have exactly one positive and on negative effect
                if positives.len() != 1 || negatives.len() != 1 || effs.len() != 2 {
                    tracing::trace!(
                        "not exactly one positive and one negative effect (for the same non-return parameters) {act:?} {positives:?} {negatives:?}"
                    );
                    return false;
                }
                let pos = positives[0];
                let neg = negatives[0];

                let pos_args = try_into_simple_args(
                    pos.effect_expression.state_variable.fluent,
                    &pos.effect_expression.state_variable.arguments,
                );
                let neg_args = try_into_simple_args(
                    neg.effect_expression.state_variable.fluent,
                    &neg.effect_expression.state_variable.arguments,
                );
                if pos_args.is_none() || neg_args.is_none() {
                    tracing::trace!("non-simple args");
                    return false;
                }
                if pos_args != neg_args {
                    tracing::trace!("not the same args");
                    return false;
                }

                // Recall that by convention, when a positive and negative effect (over the same non-return parameters)
                // start at the same time, the "tie-breaker" is that the negative is applied before the positive one.
                // ("delete-before-add")

                // We do not support the case where the negative effect can start strictly after the positive one
                if !timing_is_necessarily_before_or_eq(neg.effect_expression.timing, pos.effect_expression.timing) {
                    // (TODO ? is this requirement liftable when there are "null"/"undefined" values/objects for user types ? TODO)
                    tracing::trace!("unrecognized pattern for temporal split");
                    return false;
                }
                if neg.effect_expression.timing != pos.effect_expression.timing {
                    tracing::trace!(
                        "unrecognized pattern for temporal split [for now, only support case with exact same timestamps]"
                    );
                    return false;
                }

                // When there is a negative condition (on the same non-return parameters as the two effects),
                // and the two effects do not start at the same time,
                // there is a risk of the negative condition being required between them.
                // For example, if the negative effect starts before the positive one (but not at the same time),
                // and the negative condition is between them,
                // then we cannot safely lift, as the (lifted) value after the negative effect and before the positive one
                // could only be the "unknown" value, so not a value that is provably different from the one forbidden by the negative condition.
                if has_negative_condition && pos.effect_expression.timing != neg.effect_expression.timing {
                    tracing::trace!("not at same timestamps + negative conditions");
                    return false;
                }
            }
        }

        tracing::trace!("OK");
        // we have passed all the tests, this predicate can be lifted as a state variable
        true
    }
}

pub(super) fn timing_is_necessarily_before_or_eq(t1: Timestamp, t2: Timestamp) -> bool {
    match (t1.reference, t2.reference) {
        (tt1, tt2) if tt1 == tt2 => t1.delay <= t2.delay,
        (TimeRef::ActionStart, TimeRef::ActionEnd) => t1.delay <= t2.delay,
        (TimeRef::ActionEnd, TimeRef::ActionStart) => false,
        _ => unreachable!(),
    }
}

/// Returns true if the fluent is static, meaning that all effects over it only contain
/// constants and are at the temporal origin. This last requirement is nonstandard. (FIXME @arbimo right ?)
///
/// Works for any fluent / state function (not just boolean predicates).
fn is_fluent_static(fluent_id: FluentId, model: &Model) -> bool {
    let effect_is_constant_assignment_at_origin = |eff: &Effect| -> bool {
        if eff
            .effect_expression
            .state_variable
            .arguments
            .iter()
            .any(|&eid| !model.env.node(eid).is_cst())
        {
            return false;
        }
        match eff.effect_expression.operation {
            EffectOp::Assign(value_eid) if !model.env.node(value_eid).is_cst() => return false,
            EffectOp::Assign(_) => (),
            _ => return false,
        }
        if eff.effect_expression.timing.reference != TimeRef::Origin {
            return false;
        }

        true
    };

    if model
        .actions
        .iter()
        .flat_map(get_action_effect_exprs)
        .any(|eff| fluent_id == eff.effect_expression.state_variable.fluent)
    {
        // Detected non-initial effects over fluent.
        // (i.e. inside an action, not at global/top level)
        return false;
    }

    if get_global_effect_exprs(model).iter().any(|eff| {
        fluent_id == eff.effect_expression.state_variable.fluent && !effect_is_constant_assignment_at_origin(eff)
    }) {
        // Detected global/top level effects not using constants or not at temporal origin
        return false;
    }

    true
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(super) enum CstArg {
    Bool(bool),
    Real(RealValue),
    Object(Sym),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum SimpleArg {
    Cst(CstArg),
    Param(Sym),
}
