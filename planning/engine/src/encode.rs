//! A number of utility functions for converting from `planx` to `aries-timelines`

pub mod constraints;
pub mod encoding;
pub mod required_values;
pub mod tags;

use aries::{
    core::literals::ConjunctionBuilder,
    model::lang::{
        BoolExpr, IntExpr,
        expr::{eq, lin_eq},
    },
    prelude::*,
};
use itertools::Itertools;
use planx::{ExprId, Fun, Message, Model, Res, Sym, TimeRef, Timestamp, Type, errors::Spanned};
use smallvec::SmallVec;
use timelines::{
    Effect, EffectOp, FluentParam, FluentsEncoding, IntExp, IntTerm, Sched, StateVar, SymAtom, TaskId, Time,
    boxes::Segment,
    constraints::{HasValueAt, bool2int},
    symbols::ObjectEncoding,
};

use crate::encode::{
    constraints::{ConditionConstraint, ConditionExpression},
    encoding::Encoding,
    required_values::RequiredValues,
};

/// Encode the types and objects in the model
pub fn types(model: &Model) -> ObjectEncoding {
    let t = &model.env.types;
    let o = &model.env.objects;
    ObjectEncoding::build(
        t.top_user_type().name.to_string(),
        |c| t.subtypes(Sym::from(c.as_str())).map(|st| st.to_string()).collect_vec(),
        |c| {
            o.of_type(c.as_str())
                .map(|o| o.name().to_string())
                .sorted() // sorting is unnecessary but may be useful to group together similar objects in the absence of typing
                .collect_vec()
        },
    )
}

pub fn fluents(model: &Model, objects: &ObjectEncoding) -> Res<FluentsEncoding> {
    let mut res = FluentsEncoding::default();

    for f in model.env.fluents.iter() {
        let params = {
            let mut ps = SmallVec::<[FluentParam; 6]>::new();
            for tpe in f.parameters.iter().map(|p| &p.tpe) {
                ps.push(FluentParam {
                    range: type_range(tpe, objects)?,
                    tpe: tpe.to_string(),
                });
            }
            ps
        };
        let r#return = FluentParam {
            range: type_range(&f.return_type, objects)?,
            tpe: f.return_type.to_string(),
        };

        res.add(f.name().to_string(), &params, r#return);
    }
    Ok(res)
}

fn type_range(param_type: &Type, objects: &ObjectEncoding) -> Res<Segment> {
    match param_type {
        Type::Bool => Ok(Segment::new(0, 1)),
        Type::Int(int_interval) => Ok(Segment::new(
            int_interval
                .0
                .map(|x| IntCst::try_from(x).unwrap())
                .unwrap_or(INT_CST_MIN),
            int_interval
                .1
                .map(|x| IntCst::try_from(x).unwrap())
                .unwrap_or(INT_CST_MAX),
        )),
        Type::Real => Ok(Segment::all()),
        Type::User(var_type) => {
            let Some(var_type) = var_type.to_single_type() else {
                return Err(Message::error("Unsupported parameter type (union type)"));
            };
            let range = objects
                .domain_of_type(var_type.name.as_str())
                .ok_or_else(|| var_type.name.invalid("Could not determine the domain of this type."))?;
            Ok(Segment::new(range.first, range.last))
        }
    }
}

/// Scope from convertion function can find the values binded in their environments, (action sart, end, presence, parameters, ...)
#[derive(Debug, Clone)]
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

    pub fn sub_scope(&self, presence: Lit) -> Scope<'a> {
        let mut scope = self.clone();
        scope.presence = presence;
        scope
    }
}

/// Converts the condition `[tp] expr` to a constraint.
///
/// IMPORTANT: the expression may requires some values that should be identified with [`ConditionConstraint::add_required_values`] method.
/// Failure to do so means that some effects may incorrectly be optimized away because they are deemed useless.
pub fn condition_to_constraint(
    tp: Timestamp,
    expr: ExprId,
    model: &Model,
    sched: &mut Sched,
    bindings: &Scope,
    encoding: &mut Encoding,
) -> Res<ConditionConstraint> {
    let expr = model.env.node(expr);
    let timepoint = reify_timing(tp, model, sched, bindings)?;
    let constraint = match expr.expr() {
        planx::Expr::Bool(b) => if *b {
            ConditionExpression::LeqZero(LinSum::zero())
        } else {
            ConditionExpression::LeqZero(LinSum::cst(1))
        }
        .scoped(bindings.presence),
        planx::Expr::StateVariable(fluent_id, args) => {
            let fluent = model.env.fluents.get(*fluent_id);
            let mut reif_args = Vec::with_capacity(args.len());
            for a in args {
                let a = reify_sym(*a, model, sched, bindings, encoding)?;
                reif_args.push(a);
            }
            let state_var = StateVar {
                fluent: fluent.name().to_string(),
                args: reif_args,
            };
            let c = HasValueAt {
                state_var,
                value: IntTerm::TRUE,
                timepoint,
                prez: bindings.presence,
                source: bindings.source,
            };
            ConditionExpression::HasValue(c).scoped(bindings.presence)
        }
        planx::Expr::App(planx::Fun::Not, exprs) if exprs.len() == 1 => {
            // call recursively to obtain a an expression to negate,
            // we do not track the required value there because we will post the negation (and that is the one we want to follow)
            let c = condition_to_constraint(tp, exprs[0], model, sched, bindings, encoding)?;
            !c
        }
        planx::Expr::App(planx::Fun::Eq, exprs) if exprs.len() == 2 => {
            use planx::Expr::*;
            match (model.env.node(exprs[0]).expr(), model.env.node(exprs[1]).expr()) {
                (StateVariable(fluent_id, args), _) => {
                    let fluent = model.env.fluents.get(*fluent_id);
                    let mut reif_args = Vec::with_capacity(args.len());
                    for a in args {
                        let a = reify_sym(*a, model, sched, bindings, encoding)?;
                        reif_args.push(a);
                    }
                    let state_var = StateVar {
                        fluent: fluent.name().to_string(),
                        args: reif_args,
                    };
                    let c = HasValueAt {
                        state_var,
                        value: reify_expression(exprs[1], Some(timepoint), model, sched, bindings, encoding)
                            .map(|arg_expr| flatten_expression(arg_expr, sched, bindings))?,
                        timepoint,
                        prez: bindings.presence,
                        source: bindings.source,
                    };
                    ConditionExpression::HasValue(c).scoped(bindings.presence)
                }
                (_, StateVariable(fluent_id, args)) => {
                    let fluent = model.env.fluents.get(*fluent_id);
                    let mut reif_args = Vec::with_capacity(args.len());
                    for a in args {
                        let a = reify_sym(*a, model, sched, bindings, encoding)?;
                        reif_args.push(a);
                    }
                    let state_var = StateVar {
                        fluent: fluent.name().to_string(),
                        args: reif_args,
                    };
                    let c = HasValueAt {
                        state_var,
                        value: reify_expression(exprs[0], Some(timepoint), model, sched, bindings, encoding)
                            .map(|arg_expr| flatten_expression(arg_expr, sched, bindings))?,
                        timepoint,
                        prez: bindings.presence,
                        source: bindings.source,
                    };
                    ConditionExpression::HasValue(c).scoped(bindings.presence)
                }
                _ => {
                    let e1 = reify_expression(exprs[0], Some(timepoint), model, sched, bindings, encoding)?;
                    let e2 = reify_expression(exprs[1], Some(timepoint), model, sched, bindings, encoding)?;
                    /*{
                        let (e1_lb, e1_ub) = e1.bounds(&sched.model);
                        let (e2_lb, e2_ub) = e2.bounds(&sched.model);
                        sched.model.enforce(e1.clone().leq(e2_ub), [bindings.presence]);
                        sched.model.enforce(e2.clone().geq(e1_lb), [bindings.presence]);
                        sched.model.enforce(e2.clone().leq(e1_ub), [bindings.presence]);
                        sched.model.enforce(e1.clone().geq(e2_lb), [bindings.presence]);
                        // (WARNING / BUG): bindings.presence can cause problems (try visitall with 1 action)
                    }*/
                    ConditionExpression::EqZero(e1 - e2).scoped(bindings.presence)
                }
            }
        }
        planx::Expr::App(planx::Fun::Leq, exprs) if exprs.len() == 2 => {
            // ?TODO? separate pattern when lhs and/or rhs is a state variable ??? (like above)
            let lhs = reify_expression(exprs[0], Some(timepoint), model, sched, bindings, encoding)?;
            let rhs = reify_expression(exprs[1], Some(timepoint), model, sched, bindings, encoding)?;
            /*{
                let lhs_lb = lhs.bounds(&sched.model).0;
                let rhs_ub = rhs.bounds(&sched.model).1;
                sched.model.enforce(lhs.clone().leq(rhs_ub), [bindings.presence]);
                sched.model.enforce(rhs.clone().geq(lhs_lb), [bindings.presence]);
                // (WARNING / BUG): bindings.presence can cause problems (try visitall with 1 action)
            }*/
            ConditionExpression::LeqZero(lhs - rhs).scoped(bindings.presence)
        }
        planx::Expr::App(planx::Fun::Or, exprs) => {
            let mut disjuncts = Vec::with_capacity(exprs.len());
            for &expr in exprs {
                // we create a local scope for each disjunct which is necessary because
                // the constraint must hold even if a disjunct cannot be evaluated (e.g. no value on the state variable it refers to)
                let local_scope = sched.model.new_presence_variable(bindings.presence, "").true_lit();
                let local_bindings = bindings.sub_scope(local_scope);
                let c = condition_to_constraint(tp, expr, model, sched, &local_bindings, encoding)?;
                disjuncts.push(c);
            }
            ConditionExpression::Or(disjuncts).scoped(bindings.presence)
        }
        planx::Expr::App(planx::Fun::And, exprs) => {
            let mut conjuncts = Vec::with_capacity(exprs.len());
            for &expr in exprs {
                // like for disjunction there is a local scope for each conjunct.
                // This is needed be cause the expression can be negated (and become a disjunction) for which idenpendent scopes are necessary
                let local_scope = sched.model.new_presence_variable(bindings.presence, "").true_lit();
                let local_bindings = bindings.sub_scope(local_scope);
                let c = condition_to_constraint(tp, expr, model, sched, &local_bindings, encoding)?;
                conjuncts.push(c);
            }
            ConditionExpression::And(conjuncts).scoped(bindings.presence)
        }
        planx::Expr::App(planx::Fun::Geq, exprs) if exprs.len() == 2 => {
            // ?TODO? separate pattern when lhs and/or rhs is a state variable ??? (like above)
            let lhs = reify_expression(exprs[0], Some(timepoint), model, sched, bindings, encoding)?;
            let rhs = reify_expression(exprs[1], Some(timepoint), model, sched, bindings, encoding)?;
            /*{
                let lhs_ub = lhs.bounds(&sched.model).1;
                let rhs_lb = rhs.bounds(&sched.model).0;
                sched.model.enforce(rhs.clone().leq(lhs_ub), [bindings.presence]);
                sched.model.enforce(lhs.clone().geq(rhs_lb), [bindings.presence]);
                // (WARNING / BUG): bindings.presence can cause problems (try visitall with 1 action)
            }*/
            ConditionExpression::LeqZero(rhs - lhs).scoped(bindings.presence)
        }
        _ => return Err(expr.todo("not supported")),
    };

    // update the required values if requested by caller
    Ok(constraint)
}

/// Converts a [`planx::Effect`] into a [`timelines::Effect`]
///
/// The effect will have a transition time of [`Sched::epsilon`], with the `transition_time_after`
/// parameter controlling whether the transition time is before or after the timepoint indicated in the original effect.
/// Affect at action start/end should hage the transition after (to be available at action.end + epsilon),
/// while initial effects should have their transition time before (to be available at t=0).
pub fn convert_effect(
    effect: &planx::Effect,
    transition_time_after: bool,
    model: &Model,
    sched: &mut Sched,
    bindings: &Scope,
    encoding: &mut Encoding,
) -> Res<timelines::Effect> {
    if !effect.universal_quantification.is_empty() || effect.effect_expression.condition.is_some() {
        return model.env.node(effect).todo("Unsupported").failed();
    }
    let x = &effect.effect_expression;
    let t = reify_timing(x.timing, model, sched, bindings)?;
    let args: Vec<SymAtom> = x
        .state_variable
        .arguments
        .iter()
        .map(|&arg| reify_sym(arg, model, sched, bindings, encoding))
        .try_collect()?;
    let sv = timelines::StateVar {
        fluent: model.env.fluents.get(x.state_variable.fluent).name().to_string(),
        args,
    };
    let op = match x.operation {
        planx::EffectOp::Assign(v) => {
            EffectOp::Assign(reify_expression_to_term(v, Some(t), model, sched, bindings, encoding)?)
        }
        planx::EffectOp::Increase(v) => {
            EffectOp::Step(reify_expression_to_term(v, Some(t), model, sched, bindings, encoding)?)
        }
        planx::EffectOp::Decrease(v) => {
            EffectOp::Step(-reify_expression_to_term(v, Some(t), model, sched, bindings, encoding)?)
        }
    };
    let eff = timelines::Effect {
        transition_start: if transition_time_after { t } else { t - sched.epsilon },
        transition_end: if transition_time_after { t + sched.epsilon } else { t },
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

    // all state variables that may require a `0` value, which encodes `false` for predicates
    // we will only place a negative effect for those state variables.
    let req_state_vars = reqs.state_variables(|v| v == 0);

    for sv in req_state_vars {
        if model.env.fluents.get(sv.fluent).return_type != planx::Type::Bool {
            continue; // this is not a boolean fluent and thus not eligible for the closed world assumption
        }
        let args: Vec<SymAtom> = sv.params.0.into_iter().map(SymAtom::from).collect_vec();
        let sv = timelines::StateVar {
            fluent: model.env.fluents.get(sv.fluent).name().to_string(),
            args,
        };
        // we manually create the mutex-end since it may have a negative value if canceledd by an initial positive effect
        let mutex_end: Time = sched.model.new_ivar(-1, INT_CST_MAX, "_").into();
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
                    .var_domain(e.transition_end)
                    .overlaps(&sched.model.var_domain(o.transition_end))
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
                override_conditions.push(lin_eq(*a1, *a2).reified(&mut sched.model));
            }
            let conjunction = override_conditions.build();
            let cancelled_by = sched.model.reify(conjunction);

            // record the overriden possibility into the conditions for the effect activity
            active.push(!cancelled_by);
        }
        let active = active.build();
        let active = sched.model.reify(active);
        debug_assert_eq!(sched.model.presence_literal(active), Lit::TRUE);

        if !active.absurd() {
            let mut eff = e.clone();
            eff.prez = active;
            with_set_semantics.push(eff);
            // record that the `active` is a subscope of `e.prez`
            // this is useful to allow finer reasoning on the scope of expressions
            // Its absence may also cause our pedantic checks to fail (because they are overly careful)
            sched.model.state.add_implication(active, e.prez);
        }
    }
    with_set_semantics
}

pub fn reify_timing(t: Timestamp, model: &Model, sched: &mut Sched, binding: &Scope) -> Res<Time> {
    let tp = reify_timeref(t.reference, model, sched, binding)?;
    if *t.delay.numer() == 0 {
        Ok(tp)
    } else {
        Message::todo("unsupported non-zero delay").failed()
    }
}
pub fn reify_timeref(t: TimeRef, _model: &Model, sched: &Sched, binding: &Scope) -> Res<Time> {
    match t {
        TimeRef::Origin => Ok(sched.origin),
        TimeRef::Horizon => Ok(sched.horizon),
        TimeRef::ActionStart => Ok(binding.start),
        TimeRef::ActionEnd => Ok(binding.end),
        _ => Message::todo(format!("unsupported timeref {t:?}")).failed(),
    }
}

pub fn reify_sym(
    eid: ExprId,
    model: &Model,
    sched: &mut Sched,
    binding: &Scope,
    encoding: &mut Encoding,
) -> Res<SymAtom> {
    reify_expression(eid, None, model, sched, binding, encoding).map(|e| flatten_expression(e, sched, binding))
}

pub fn reify_constant(
    e: ExprId,
    model: &Model,
    sched: &mut Sched,
    scope: &Scope,
    encoding: &mut Encoding,
) -> Res<IntCst> {
    let reif = reify_expression(e, None, model, sched, scope, encoding)?;
    let reif = flatten_expression(reif, sched, scope);
    let cst = IntCst::try_from(reif).map_err(|_| model.env.node(e).todo("non constant term unsupported"))?;
    Ok(cst)
}

pub fn reify_expression_to_term(
    e: ExprId,
    time: Option<Time>,
    model: &Model,
    sched: &mut Sched,
    scope: &Scope,
    encoding: &mut Encoding,
) -> Res<IntTerm> {
    let reif = reify_expression(e, time, model, sched, scope, encoding)?;
    Ok(flatten_expression(reif, sched, scope))
}

pub fn reify_expression(
    e: ExprId,
    time: Option<Time>,
    model: &Model,
    sched: &mut Sched,
    binding: &Scope,
    encoding: &mut Encoding,
) -> Res<IntExp> {
    let e = model.env.node(e);
    use planx::Expr::*;
    match e.expr() {
        Bool(true) => Ok(1.into()),
        Bool(false) => Ok(0.into()),
        Real(r) if r.denom() == &1 => {
            if let Ok(i) = IntCst::try_from(*r.numer()) {
                Ok(i.into())
            } else {
                e.todo(format!("Cannot be converted to an {}", aries::core::INT_TYPE_NAME))
                    .failed()
            }
        }
        planx::Expr::Object(object) => {
            let id = sched
                .objects
                .object_id(object.name().as_str())
                .ok_or_else(|| e.invalid("Object has no associated value"))?;
            Ok(id.into())
        }
        planx::Expr::Param(param) => binding
            .args
            .get(param.name())
            .copied()
            // .inspect(|id| println!("{:?}-{:?}", id.lower_bound(&sched.model), id.upper_bound(&sched.model)))
            .map(|id| id.into())
            .ok_or_else(|| param.name().invalid("unknown parameter")),
        StateVariable(fluent, args) => {
            let Some(time) = time else {
                return e
                    .invalid("this is a state variable and cannot be interpreted without a temporal context")
                    .failed();
            };
            let fluent = model.env.fluents.get(*fluent);
            let reified_var = sched
                .model
                .new_optional_ivar(INT_CST_MIN, INT_CST_MAX, binding.presence, "");
            let reified_args = args
                .iter()
                .map(|&arg| {
                    reify_expression(arg, Some(time), model, sched, binding, encoding)
                        .map(|arg_expr| flatten_expression(arg_expr, sched, binding))
                })
                .collect::<Res<Vec<IntTerm>>>()?;
            let state_var = StateVar {
                fluent: fluent.name().to_string(),
                args: reified_args,
            };
            let reif = HasValueAt {
                state_var,
                value: reified_var.into(),
                timepoint: time,
                prez: binding.presence,
                source: binding.source,
            };
            let reif = ConditionExpression::HasValue(reif).scoped(binding.presence);
            reif.constraint
                .add_required_values(&mut encoding.required_values, model, &sched);
            sched.add_constraint(reif);
            Ok(reified_var.into())
        }
        planx::Expr::App(Fun::Plus, args) => {
            let mut sum = IntExp::zero();
            for arg in args {
                sum += reify_expression(*arg, time, model, sched, binding, encoding)?;
            }
            Ok(sum)
        }
        planx::Expr::App(Fun::Minus, args) if args.len() == 2 => {
            let mut sum = IntExp::zero();
            sum += reify_expression(args[0], time, model, sched, binding, encoding)?;
            sum -= reify_expression(args[1], time, model, sched, binding, encoding)?;
            Ok(sum)
        }
        planx::Expr::App(Fun::Mul, args) if args.len() == 2 => {
            let a1 = reify_expression(args[0], time, model, sched, binding, encoding)?;
            let a2 = reify_expression(args[1], time, model, sched, binding, encoding)?;
            let expr = if let Ok(cst) = IntCst::try_from(a1.clone()) {
                a2 * cst
            } else if let Ok(cst) = IntCst::try_from(a2.clone()) {
                a1 * cst
            } else {
                return e.todo("non linear expression is not supported").failed();
            };
            Ok(expr)
        }
        planx::Expr::ViolationCount(x) => {
            let sum = if let Some(values) = encoding.preferences.get(x.as_str()) {
                values
                    .iter()
                    .fold(LinSum::zero(), |acc, v| acc + bool2int(!v, &mut sched.model))
            } else {
                LinSum::zero()
            };
            Ok(sum)
        }
        _ => e.todo(format!("not supported [{e}]")).failed(),
    }
}

pub fn flatten_expression(e: IntExp, sched: &mut Sched, binding: &Scope) -> IntTerm {
    if let Ok(term) = IntTerm::try_from(e.clone()) {
        term
    } else {
        e.reify(binding.presence, &mut sched.model)
    }
}
