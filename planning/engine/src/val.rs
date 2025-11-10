use aries::{
    core::Lit,
    model::lang::{FAtom, hreif::BoolExpr},
};
use aries_planning_model::{ExprId, Model, Plan, Res, Sym, TimeRef, Timestamp, errors::Spanned};
use aries_sched::{EffectOp, IntCst, Sched, StateVar, SymAtom, Time, constraints::HasValueAt, symbols::ObjectEncoding};
use itertools::Itertools;

fn types(model: &Model) -> ObjectEncoding {
    let t = &model.env.types;
    let o = &model.env.objects;
    ObjectEncoding::build(
        t.top_user_type().name.to_string(),
        |c| t.subtypes(Sym::from(c.as_str())).map(|st| st.to_string()).collect_vec(),
        |c| o.of_type(c.as_str()).map(|o| o.name().to_string()).collect_vec(),
    )
}

pub fn validate(model: &Model, plan: &Plan) -> Res<bool> {
    let objs = types(model);
    let mut sched = aries_sched::Sched::new(1, objs);
    let global_scope = Scope::global(&sched);

    for x in &model.init {
        let eff = convert_effect(x, false, model, &mut sched, &global_scope)?;
        sched.effects.push(eff);
    }

    match plan {
        Plan::Sequential(operators) => {
            // associate each operator to its position in the sequence `t`, use as a timestamp
            for (t, op) in operators.iter().enumerate() {
                let a = model
                    .actions
                    .get_action(&op.action_ref)
                    .ok_or_else(|| op.action_ref.invalid("canot find corresponding action"))?;
                let mut args = im::OrdMap::new();
                for (param, arg) in a.parameters.iter().zip(op.arguments.iter()) {
                    let arg = sched
                        .objects
                        .object_atom(arg.name().canonical_str())
                        .ok_or_else(|| arg.name().invalid("unknown object"))?;
                    args.insert(&param.name, arg);
                }

                let bindings = Scope {
                    start: Time::from(t as IntCst),
                    end: Time::from(t as IntCst),
                    presence: Lit::TRUE,
                    args,
                };

                for x in &a.effects {
                    let eff = convert_effect(x, true, model, &mut sched, &bindings)?;
                    sched.effects.push(eff);
                }
                for c in &a.conditions {
                    if let Some(tp) = c.interval.as_timestamp() {
                        let c = condition_to_constraint(tp, c.cond, model, &mut sched, &bindings)?;
                        sched.add_boxed_constraint(c);
                    }
                }
            }
        }
    }

    for x in &model.goals {
        assert!(x.universal_quantification.is_empty());
        match x.goal_expression {
            aries_planning_model::SimpleGoal::HoldsDuring(time_interval, expr_id) => {
                if let Some(tp) = time_interval.as_timestamp() {
                    let c = condition_to_constraint(tp, expr_id, model, &mut sched, &global_scope)?;
                    sched.add_boxed_constraint(c);
                } else {
                    todo!()
                }
            }
            _ => todo!(),
        }
    }

    //

    match sched.solve() {
        Some(sol) => {
            sched.print(&sol);
            Ok(true)
        }
        None => panic!("INVALID PLAN"),
    }
}

struct Scope<'a> {
    start: Time,
    end: Time,
    presence: Lit,
    args: im::OrdMap<&'a Sym, SymAtom>,
}
impl<'a> Scope<'a> {
    pub fn global(sched: &Sched) -> Scope<'a> {
        Self {
            start: sched.origin,
            end: sched.horizon,
            presence: Lit::TRUE,
            args: im::OrdMap::new(),
        }
    }
}

fn convert_effect(
    x: &aries_planning_model::Effect,
    transition_time: bool,
    model: &Model,
    sched: &mut Sched,
    bindings: &Scope,
) -> Res<aries_sched::Effect> {
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
    let sv = aries_sched::StateVar {
        fluent: model.env.fluents.get(x.state_variable.fluent).name().to_string(),
        args,
    };
    let op = match x.operation {
        aries_planning_model::EffectOp::Assign(e) => {
            let val = reify_bool(e, model, sched)?;
            EffectOp::Assign(val)
        }
        _ => todo!(),
    };
    let eff = aries_sched::Effect {
        transition_start: t,
        transition_end: if transition_time { t + FAtom::EPSILON } else { t },
        mutex_end: sched.new_timepoint(),
        state_var: sv,
        operation: op,
        prez: bindings.presence,
    };
    Ok(eff)
}

fn reify_timing(t: Timestamp, model: &Model, sched: &mut Sched, binding: &Scope) -> Res<FAtom> {
    let tp = reify_timeref(t.reference, model, sched, binding)?;
    if *t.delay.numer() == 0 { Ok(tp) } else { todo!() }
}
fn reify_timeref(t: TimeRef, _model: &Model, sched: &Sched, binding: &Scope) -> Res<FAtom> {
    match t {
        TimeRef::Origin => Ok(sched.origin),
        TimeRef::Horizon => Ok(sched.horizon),
        TimeRef::ActionStart => Ok(binding.start),
        TimeRef::ActionEnd => Ok(binding.end),
        _ => todo!("{t:?}"),
    }
}

fn reify_sym(e: ExprId, model: &Model, sched: &mut Sched, binding: &Scope) -> Res<SymAtom> {
    let e = model.env.node(e);
    match e.expr() {
        aries_planning_model::Expr::Object(object) => {
            let id = sched
                .objects
                .object_id(object.name().canonical_str())
                .ok_or_else(|| e.invalid("Object has no associated value"))?;
            Ok(SymAtom::from(id))
        }
        aries_planning_model::Expr::Param(param) => binding
            .args
            .get(param.name().canonical_str())
            .copied()
            .ok_or_else(|| param.name().invalid("unknown parameter")),
        _ => todo!(),
    }
}

fn reify_bool(e: ExprId, model: &Model, _sched: &mut Sched) -> Res<bool> {
    let e = model.env.node(e);
    match e.expr() {
        aries_planning_model::Expr::Bool(b) => Ok(*b),
        _ => todo!(),
    }
}

fn condition_to_constraint(
    tp: Timestamp,
    expr: ExprId,
    model: &Model,
    sched: &mut Sched,
    bindings: &Scope,
) -> Res<Box<dyn BoolExpr<Sched>>> {
    let expr = model.env.node(expr);
    match expr.expr() {
        aries_planning_model::Expr::StateVariable(fluent_id, args) => {
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
                value: Lit::TRUE.into(),
                timepoint: reify_timing(tp, model, sched, bindings)?,
                prez: bindings.presence,
            };
            Ok(Box::new(c))
        }
        e => todo!("{e:?}"), // aries_planning_model::Expr::Real(ratio) => todo!(),
                             // aries_planning_model::Expr::Bool(_) => todo!(),
                             // aries_planning_model::Expr::Object(object) => todo!(),
                             // aries_planning_model::Expr::Param(param) => todo!(),
                             // aries_planning_model::Expr::App(fun, small_vec) => todo!(),
                             // aries_planning_model::Expr::Exists(params, expr_id) => todo!(),
                             // aries_planning_model::Expr::Forall(params, expr_id) => todo!(),
                             // aries_planning_model::Expr::Instant(timestamp) => todo!(),
                             // aries_planning_model::Expr::Duration => todo!(),
                             // aries_planning_model::Expr::Makespan => todo!(),
                             // aries_planning_model::Expr::ViolationCount(sym) => todo!(),
    }
}
