//! A number of utility functions for converting from `planx` to `aries-timelines`

pub mod constraints;
pub mod encoding;
pub mod required_values;
pub mod tags;

use aries::{
    core::literals::ConjunctionBuilder,
    model::lang::{
        FAtom,
        expr::{and, eq},
    },
    prelude::*,
    reif::ReifExpr,
};
use itertools::Itertools;
use planx::{ExprId, Message, Model, Res, Sym, TimeRef, Timestamp, errors::Spanned};
use timelines::{
    Effect, EffectOp, Sched, StateVar, SymAtom, TaskId, Time, constraints::HasValueAt, symbols::ObjectEncoding,
};

use crate::encode::{constraints::ConditionConstraint, required_values::RequiredValues};

/// Encode the types and objects in the model
pub fn types(model: &Model) -> ObjectEncoding {
    let t = &model.env.types;
    let o = &model.env.objects;
    ObjectEncoding::build(
        t.top_user_type().name.canonical_str().to_string(),
        |c| {
            t.subtypes(Sym::from(c.as_str()))
                .map(|st| st.canonical_str().to_string())
                .collect_vec()
        },
        |c| {
            o.of_type(c.as_str())
                .map(|o| o.name().canonical_str().to_string())
                .sorted() // sorting is unecessary but may be useful to group together similar objects in the absence of typing
                .collect_vec()
        },
    )
}

/// Scope from convertion function can find the values binded in their environments, (action sart, end, presence, parameters, ...)
pub struct Scope<'a> {
    pub start: Time,
    pub end: Time,
    pub presence: Lit,
    pub args: im::OrdMap<&'a Sym, SymAtom>,
    pub source: Option<TaskId>,
}
impl<'a> Scope<'a> {
    pub fn global(sched: &Sched) -> Scope<'a> {
        Self {
            start: sched.origin,
            end: sched.horizon,
            presence: Lit::TRUE,
            args: im::OrdMap::new(),
            source: None,
        }
    }
}

/// Converts the condition `[tp] expr` to a constraint.
///
/// If the `required_values` parameters is non empty, then the function will update it to reflect the state variable values possibly required by this expresion.
pub fn condition_to_constraint(
    tp: Timestamp,
    expr: ExprId,
    model: &Model,
    sched: &mut Sched,
    bindings: &Scope,
    required_values: Option<&mut RequiredValues>,
) -> Res<ConditionConstraint> {
    let expr = model.env.node(expr);
    let constraint = match expr.expr() {
        planx::Expr::StateVariable(fluent_id, args) => {
            let fluent = model.env.fluents.get(*fluent_id);
            let mut reif_args = Vec::with_capacity(args.len());
            for a in args {
                let a = reify_sym(*a, model, sched, bindings)?;
                reif_args.push(a);
            }
            let state_var = StateVar {
                fluent: fluent.name().to_string(),
                args: reif_args,
            };
            let c = HasValueAt {
                state_var,
                value: IAtom::TRUE,
                timepoint: reify_timing(tp, model, sched, bindings)?,
                prez: bindings.presence,
                source: bindings.source,
            };
            ConditionConstraint::HasValue(c)
        }
        planx::Expr::App(planx::Fun::Not, exprs) if exprs.len() == 1 => {
            // call recursively to obtain a an expression to negate, we pass None for the required_values has the one that will be parsed is not required
            let c = condition_to_constraint(tp, exprs[0], model, sched, bindings, None)?;
            match c {
                ConditionConstraint::HasValue(mut c) => {
                    if let Ok(x) = IntCst::try_from(c.value)
                        && (x == 0 || x == 1)
                    {
                        c.value = (1 - x).into(); // negation : 0 -> 1 and 1 -> 0
                        ConditionConstraint::HasValue(c)
                    } else {
                        return expr.todo("unsupported").failed();
                    }
                }
                ConditionConstraint::Eq(a, b) => ConditionConstraint::Neq(a, b),
                ConditionConstraint::Neq(a, b) => ConditionConstraint::Eq(a, b),
            }
        }
        planx::Expr::App(planx::Fun::Eq, exprs) if exprs.len() == 2 => {
            let e1 = reify_sym(exprs[0], model, sched, bindings)?;
            let e2 = reify_sym(exprs[1], model, sched, bindings)?;
            ConditionConstraint::Eq(e1, e2)
        }
        _ => return Err(expr.todo("not supported")),
    };

    // update the required values if requested by caller
    if let Some(reqs) = required_values {
        match &constraint {
            ConditionConstraint::HasValue(c) => {
                // record that someone required such a value
                let fluent_id = model.env.fluents.get_by_name(&c.state_var.fluent).unwrap();
                reqs.add(fluent_id, c.value_box(&sched.model).as_ref());
            }
            // not on a fluent and thus no need to update the required_values
            ConditionConstraint::Eq(_, _) => {}
            ConditionConstraint::Neq(_, _) => {}
        }
    }
    Ok(constraint)
}

pub fn convert_effect(
    x: &planx::Effect,
    transition_time: bool,
    model: &Model,
    sched: &mut Sched,
    bindings: &Scope,
) -> Res<timelines::Effect> {
    assert!(x.universal_quantification.is_empty());
    let x = &x.effect_expression;
    assert!(x.condition.is_none());
    let t = reify_timing(x.timing, model, sched, bindings)?;
    let args: Vec<SymAtom> = x
        .state_variable
        .arguments
        .iter()
        .map(|&arg| reify_sym(arg, model, sched, bindings))
        .try_collect()?;
    let sv = timelines::StateVar {
        fluent: model.env.fluents.get(x.state_variable.fluent).name().to_string(),
        args,
    };
    let op = match x.operation {
        planx::EffectOp::Assign(e) => {
            let val = reify_value(e, model, sched)?;
            EffectOp::Assign(val)
        }
        _ => todo!(),
    };
    let eff = timelines::Effect {
        transition_start: t,
        transition_end: if transition_time { t + FAtom::EPSILON } else { t },
        mutex_end: sched.new_timepoint(),
        state_var: sv,
        operation: op,
        prez: bindings.presence,
        source: bindings.source,
    };
    Ok(eff)
}

/// Add all required default negative values as effects just before the origin.
pub fn add_closed_world_negative_effects(reqs: &RequiredValues, model: &Model, sched: &mut Sched) {
    // time at which to place the negative effects. This is -1 so that it can be overrided by an initial effect (at 0).
    let t = Time::from(-1);

    // all state variables that may require a `false` value
    // we will only place a negative effect for those state variables.
    let req_state_vars = reqs.state_variables(|v| v == 0);

    for sv in req_state_vars {
        let args: Vec<SymAtom> = sv.params.0.into_iter().map(SymAtom::from).collect_vec();
        let sv = timelines::StateVar {
            fluent: model.env.fluents.get(sv.fluent).name().to_string(),
            args,
        };
        // we manually create the mutex-end since it may have a negative value if canceledd by an initial positive effect
        let mutex_end: Time = sched.model.new_fvar(-1, INT_CST_MAX, sched.time_scale, "_").into();
        let eff = timelines::Effect {
            transition_start: t,
            transition_end: t,
            mutex_end,
            state_var: sv,
            operation: EffectOp::FALSE_ASSIGNMENT,
            prez: Lit::TRUE,
            // no action source as it is part of the problem definition
            source: None,
        };
        sched.add_effect(eff);
    }
}

/// Converts a set of effects *from the same action* into an equivalent set compatible with PDDL set semantics.
///
/// In classical planning and PDDL, action effects are split into a *set* of positive (add) effects and a *set* of negative (delete) effects.
/// The formula to compute the new state is `S \ del_effs U add_effs`.
///
/// This has a few consequences that make it differ from our own semantics of effects, namely:
///  - it is allowed to have the same add/del effect multiple times (they would be unified in the set)
///  - if there is both an add and a delete effect of the same fact, the delete effect is cancelleted on (add-after-delete)
///
/// This methods modifies the set of effects passed to adhere to this semantic. In particular, it will weaken the presence of some of the effects
/// to make it absent if there is another effect overriding it.
pub fn convert_to_pddl_set_semantics(effs: Vec<Effect>, sched: &mut Sched) -> Vec<Effect> {
    let mut with_set_semantics = Vec::with_capacity(effs.len());
    for (eid, e) in effs.iter().enumerate() {
        // helper function to check whether `e` can be overriden by another effect `o` (with index `oid` in the effect list)
        let possible_overriden_by = |oid: usize, o: &Effect| -> bool {
            let cancellable = match (&e.operation, &o.operation) {
                // the delete can be overriden by the add (add-after-delete semantics)
                (&EffectOp::FALSE_ASSIGNMENT, &EffectOp::TRUE_ASSIGNMENT) => true,
                // the two effects are of the same kind, the currend (eid) can be overriden by one appearing earlier in the effect list
                (&EffectOp::TRUE_ASSIGNMENT, &EffectOp::TRUE_ASSIGNMENT) => eid > oid,
                (&EffectOp::FALSE_ASSIGNMENT, &EffectOp::FALSE_ASSIGNMENT) => eid > oid,
                // an add cannot be overriden by a delete
                (&EffectOp::TRUE_ASSIGNMENT, &EffectOp::FALSE_ASSIGNMENT) => false,
                (_, _) => todo!("Not a boolean state variable or non-constant assignment"), // TODO: make it truly unreachable
            };

            cancellable
                // they are on the same fluent
                && e.state_var.fluent == o.state_var.fluent
                // they can be placed at the same timepoit
                && sched
                    .model
                    .var_domain(e.transition_end.num)
                    .overlaps(&sched.model.var_domain(o.transition_end.num)) // TODO: we ignore the denominator here, which may not be correct in temporal planning
                // they arguments are compatible
                && e.state_var
                    .args
                    .iter()
                    .map(|a1| sched.model.var_domain(*a1))
                    .zip_eq(o.state_var.args.iter().map(|a2| sched.model.var_domain(*a2)))
                    .all(|(d1, d2)| d1.overlaps(&d2))
        };

        // Required condition for the current effect to be active
        // initially containing only its presence, but we will had a literal for each other effect that may override it
        let mut active = ConjunctionBuilder::new();
        active.push(e.prez);

        // build a set of effects that *may* override this one (this is supposed to be reasonably fast and avoid modifying the model)
        let possible_overriders = effs
            .iter()
            .enumerate()
            .filter(|(oid, o)| possible_overriden_by(*oid, o))
            .map(|(_, o)| o)
            .collect_vec();

        for overrider in possible_overriders {
            // conjunction of literals that, when all true, means the effect is overriden
            // not that we only iterate on effects that are one the same fluent already
            let mut override_conditions = ConjunctionBuilder::new();

            // overrider must be present
            override_conditions.push(overrider.prez);
            // overrider must be placed at the same time
            override_conditions.push(sched.model.reify(eq(e.transition_start, overrider.transition_start)));
            // overrider must have the same state variable arguments
            for (a1, a2) in e.state_var.args.iter().zip_eq(overrider.state_var.args.iter()) {
                override_conditions.push(sched.model.reify(eq(*a1, *a2)));
            }
            let lits = override_conditions.build();
            let cancelled_by = sched.model.reify(ReifExpr::And(lits.into_lits()));

            // record the overriden possibility into the conditions for the effect activity
            active.push(!cancelled_by);
        }
        let active = active.build();
        let active = sched.model.reify(and(active.to_vec())); // TODO: this is innefficient

        if !active.absurd() {
            let mut eff = e.clone();
            eff.prez = active;
            with_set_semantics.push(eff);
        }
    }
    with_set_semantics
}

pub fn reify_timing(t: Timestamp, model: &Model, sched: &mut Sched, binding: &Scope) -> Res<FAtom> {
    let tp = reify_timeref(t.reference, model, sched, binding)?;
    if *t.delay.numer() == 0 {
        Ok(tp)
    } else {
        Message::todo("unsupported non-zero delay").failed()
    }
}
pub fn reify_timeref(t: TimeRef, _model: &Model, sched: &Sched, binding: &Scope) -> Res<FAtom> {
    match t {
        TimeRef::Origin => Ok(sched.origin),
        TimeRef::Horizon => Ok(sched.horizon),
        TimeRef::ActionStart => Ok(binding.start),
        TimeRef::ActionEnd => Ok(binding.end),
        _ => Message::todo(format!("unsupported timeref {t:?}")).failed(),
    }
}

pub fn reify_sym(e: ExprId, model: &Model, sched: &mut Sched, binding: &Scope) -> Res<SymAtom> {
    let e = model.env.node(e);
    match e.expr() {
        planx::Expr::Object(object) => {
            let id = sched
                .objects
                .object_id(object.name().canonical_str())
                .ok_or_else(|| e.invalid("Object has no associated value"))?;
            Ok(SymAtom::from(id))
        }
        planx::Expr::Param(param) => binding
            .args
            .get(param.name().canonical_str())
            .copied()
            .ok_or_else(|| param.name().invalid("unknown parameter")),
        _ => e.todo("not supported").failed(),
    }
}

pub fn reify_value(e: ExprId, model: &Model, _sched: &mut Sched) -> Res<IntCst> {
    let e = model.env.node(e);
    use planx::Expr::*;
    match e.expr() {
        Bool(true) => Ok(1),
        Bool(false) => Ok(0),
        Real(r) if r.denom() == &1 => {
            if let Ok(i) = IntCst::try_from(*r.numer()) {
                Ok(i)
            } else {
                e.todo(format!("Cannot be converted to an {}", aries::core::INT_TYPE_NAME))
                    .failed()
            }
        }
        _ => e.todo("not supported").failed(),
    }
}
