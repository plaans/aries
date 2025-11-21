pub mod lifted_plan;
pub mod potential_effects;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    sync::Arc,
    time::Instant,
};

use aries::{
    core::Lit,
    model::lang::{
        FAtom,
        hreif::{BoolExpr, Store},
    },
};
use aries_sched::{
    ConstraintID, EffectOp, Sched, StateVar, SymAtom, Time, constraints::HasValueAt, explain::ExplainableSolver,
    symbols::ObjectEncoding,
};
use derive_more::derive::Display;
use itertools::Itertools;
use planx::{ExprId, FluentId, Message, Model, Param, Res, Sym, TimeRef, Timestamp, errors::Spanned};

use crate::{
    ctags::{ActionCondition, ActionEffect, CTag, PotentialEffect, Repair},
    repair::{lifted_plan::LiftedPlan, potential_effects::PotentialEffects},
};

#[derive(clap::Args, Debug, Clone)]
pub struct RepairOptions {
    #[arg(long, default_value = "smallest")]
    mode: RepairMode,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, Display)]
pub enum RepairMode {
    Smallest,
    All,
}

#[derive(Display)]
#[display("{mode} {status:>12} {runtime:>10?} (runtime ms)  {encoding_time:>8?} (enctime ms)")]
pub struct RepairReport {
    status: RepairStatus,
    mode: RepairMode,
    runtime: u128,
    encoding_time: u128,
}
#[derive(Display)]
pub enum RepairStatus {
    #[display("VALID  ")]
    ValidPlan,
    #[display("SMCS({_0})")]
    SmallestFound(usize),
}

pub fn domain_repair(model: &Model, plan: &LiftedPlan, options: &RepairOptions) -> Res<RepairReport> {
    let start = Instant::now();
    let mut solver = encode_dom_repair(model, plan)?;
    let encoding_time = start.elapsed().as_millis();

    if solver.check_satisfiability() {
        println!("Plan is valid.");
        return Ok(RepairReport {
            status: RepairStatus::ValidPlan,
            mode: options.mode,
            runtime: start.elapsed().as_millis(),
            encoding_time,
        });
    }
    // TODO: quick SAT check
    println!("INVALID PLAN !");

    let report = match options.mode {
        RepairMode::Smallest => {
            let repair_set = solver.find_smallest_mcs().expect("problem detected as unrepairable");
            let msg = format_culprit_set(Message::error("Smallest MCS"), &repair_set, model);
            println!("\n\n{msg}");
            RepairReport {
                status: RepairStatus::SmallestFound(repair_set.len()),
                mode: options.mode,
                runtime: start.elapsed().as_millis(),
                encoding_time,
            }
        }
        RepairMode::All => {
            let mut mus_count = 0;
            let mut mcs_count = 0;
            let mut mcs_smallest = usize::MAX;
            for musmcs in solver.explain_unsat() {
                let (mut msg, culprits) = match musmcs {
                    aries::solver::musmcs::MusMcs::Mus(elems) => {
                        mus_count += 1;
                        (Message::error("MUS"), elems)
                    }
                    aries::solver::musmcs::MusMcs::Mcs(elems) => {
                        mcs_count += 1;
                        mcs_smallest = mcs_smallest.min(elems.len());
                        (Message::warning("MCS"), elems)
                    }
                };
                msg = format_culprit_set(msg, &culprits, model);
                println!("\n{msg}\n");
                println!("#MUS: {mus_count}\n#MCS: {mcs_count}\nSmallest: {mcs_smallest}");
            }
            RepairReport {
                status: RepairStatus::SmallestFound(mcs_smallest),
                mode: options.mode,
                runtime: start.elapsed().as_millis(),
                encoding_time,
            }
        }
    };
    Ok(report)
}

fn encode_dom_repair(model: &Model, plan: &LiftedPlan) -> Res<ExplainableSolver<Repair>> {
    // for each constraint we may which to relax, stores a CTag (constraint tag) so that we can later decide if it should be relaxed.
    let mut constraints_tags: BTreeMap<ConstraintID, CTag> = Default::default();

    // build encoding of all objects: associates each object to a int value and each type to a range of values
    let objs = types(model);
    let mut sched = aries_sched::Sched::new(1, objs);

    let global_scope = Scope::global(&sched);

    let plan_variables: BTreeMap<&Sym, SymAtom> = plan
        .variables
        .iter()
        .map(|(var_name, var_type)| {
            let type_bounds = sched
                .objects
                .domain_of_type(var_type.name.canonical_str())
                .ok_or_else(|| var_type.name.invalid("Could not determine the domain of this type."))?;
            let var: SymAtom = sched
                .model
                .new_ivar(type_bounds.first, type_bounds.last, var_name.canonical_str())
                .into();
            Ok::<_, Message>((var_name, var))
        })
        .try_collect()?;

    // compute the set of all effects that may be added to each action template
    let potential_effects = Arc::new(PotentialEffects::compute(model, || sched.model.new_literal(Lit::TRUE)));

    // for each potential effect, add a (soft constraint) that it is absent
    for (a, pot_effs) in &potential_effects.effs {
        for (eff_id, pot_eff) in pot_effs.iter().enumerate() {
            // create a constraint disabling the effect, and tag so that we can mark it as soft
            let cid = sched.add_constraint(!pot_eff.2);
            constraints_tags.insert(
                cid,
                CTag::DisablePotentialEffect(PotentialEffect {
                    action_id: a.clone(),
                    effect_id: eff_id,
                    all_effects: potential_effects.clone(),
                }),
            );
        }
    }

    // effect enabler: for each effect in the action template, assoicate it with a literal which if true will force all instanciation of the effec to be present

    // store the enabling literal of each effect, so that we can later reuse it to make the effect optional
    let mut effect_enablers: BTreeMap<ActionEffect, Lit> = BTreeMap::new();
    for a in model.actions.iter() {
        for (eff_id, eff) in a.effects.iter().enumerate() {
            let aeff = ActionEffect {
                action: a.name.clone(),
                effect_id: eff_id,
            };
            let enabler = match &eff.effect_expression.operation {
                planx::EffectOp::Assign(expr_id) => {
                    // hacky way to determine if the effect is positive (will only work for classical planning)
                    let positive_effect = reify_bool(*expr_id, model, &mut sched)?;
                    if positive_effect {
                        // positive effect cannot be removed
                        Lit::TRUE
                    } else {
                        // negative effect create a new literal as enabler
                        sched.model.new_literal(Lit::TRUE)
                    }
                }
                _ => todo!(), // numeric effects
            };
            // record the enabler and place a (soft) constraint forcing it to true
            effect_enablers.insert(aeff.clone(), enabler);
            let cid = sched.add_constraint(enabler);
            constraints_tags.insert(cid, CTag::EnforceEffect(aeff));
        }
    }

    // enforce all elemts of the initial state as effects
    // NOTE: we assume no negative preconditions and do not add a the negative effect for the closed world assumption.
    for x in &model.init {
        let eff = convert_effect(x, false, model, &mut sched, &global_scope)?;
        sched.add_effect(eff);
    }

    for (op_id, op) in plan.operations.iter().enumerate() {
        // correspinding action in the model
        let a = model
            .actions
            .get_action(&op.action_ref)
            .ok_or_else(|| op.action_ref.invalid("cannot find corresponding action"))?;

        // building a scope object so that downstream methods can find the value to replace the actions params/start/end/prez with
        let mut args = im::OrdMap::new();
        for (param, arg) in a.parameters.iter().zip(op.arguments.iter()) {
            let arg = match arg {
                // ground parameter, get the corresponding object constant
                lifted_plan::OperationArg::Ground(object) => sched
                    .objects
                    .object_atom(object.name().canonical_str())
                    .ok_or_else(|| object.name().invalid("unknown object"))?,
                // variable parameter, retrieve the variable we created for it
                lifted_plan::OperationArg::Variable { name } => plan_variables[name],
            };
            args.insert(&param.name, arg);
        }

        let bindings = Scope {
            start: Time::from(op.start), // start time is the index of the action in the plan
            end: Time::from(op.start + op.duration),
            presence: Lit::TRUE, // action is necessarily present
            args,
        };

        // add an effect to the scheduling problem for each effect in the action template
        // the presence of the effect is controlled by the global enabler of the effect in the template
        for (eff_id, x) in a.effects.iter().enumerate() {
            let aeff = ActionEffect {
                action: a.name.clone(),
                effect_id: eff_id,
            };
            let mut eff = convert_effect(x, true, model, &mut sched, &bindings)?;
            // replace the effect presence by its enabler
            assert_eq!(eff.prez, Lit::TRUE);
            eff.prez = effect_enablers[&aeff];
            sched.add_effect(eff);
        }

        // for each potential effect, add it as well (it will be assumed absent by default due to the global constraint)
        for (fid, params, enabler) in potential_effects.for_action(&a.name) {
            let eff = create_potential_effect(*fid, params.as_slice(), *enabler, model, &mut sched, &bindings)?;
            sched.add_effect(eff);
        }

        // for each condition, create a constraint stating it should hold. The constraint is tagged so we can later deactivate
        for (cond_id, c) in a.conditions.iter().enumerate() {
            if let Some(tp) = c.interval.as_timestamp() {
                let c = condition_to_constraint(tp, c.cond, model, &mut sched, &bindings)?;
                let cid = sched.add_boxed_constraint(c);
                constraints_tags.insert(
                    cid,
                    CTag::Support {
                        operator_id: op_id,
                        cond: ActionCondition {
                            action: a.name.clone(),
                            condition_id: cond_id,
                        },
                    },
                );
            }
        }
    }

    // for each goal, add a constraint stating it must hold (the constriant is tagged but not relaxed for domain repair)
    for (gid, x) in model.goals.iter().enumerate() {
        assert!(x.universal_quantification.is_empty());
        match x.goal_expression {
            planx::SimpleGoal::HoldsDuring(time_interval, expr_id) => {
                if let Some(tp) = time_interval.as_timestamp() {
                    let c = condition_to_constraint(tp, expr_id, model, &mut sched, &global_scope)?;
                    let cid = sched.add_boxed_constraint(c);
                    constraints_tags.insert(cid, CTag::EnforceGoal(gid));
                } else {
                    todo!()
                }
            }
            _ => todo!(),
        }
    }
    let constraint_to_repair = |cid: ConstraintID| match constraints_tags.get(&cid) {
        Some(ctag) => ctag.to_repair(),
        _ => None,
    };
    Ok(sched.explainable_solver(constraint_to_repair))
}

/// Encode the types and objects in the model
fn types(model: &Model) -> ObjectEncoding {
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
                .collect_vec()
        },
    )
}

/// Extends a base bessage to display all culprits in it.
fn format_culprit_set(mut msg: Message, culprits: &BTreeSet<Repair>, model: &Model) -> Message {
    for repair in culprits {
        match repair {
            Repair::RmCond(cond) => {
                println!("   cond: {}/{}", cond.action, cond.condition_id);
                let annot = model
                    .env
                    .node(model.actions.get_action(&cond.action).unwrap().conditions[cond.condition_id].cond)
                    .info(format!("to remove (action: {})", cond.action));
                msg = msg.snippet(annot).show(cond.action.span.as_ref().unwrap());
            }
            Repair::AddEff(potential_effect) => {
                let (fluent_id, params, _) = potential_effect.get();
                let fluent = model.env.fluents.get(*fluent_id);
                let fluent = format!("({} {})", fluent.name(), params.iter().map(|p| p.name()).format(" "));
                println!("{} => {}", &potential_effect.action_id, fluent);
                let annot = potential_effect.action_id.info(format!("Add effect: {fluent}"));
                msg = msg
                    .snippet(annot)
                    .show(potential_effect.action_id.span.as_ref().unwrap());
            }
            Repair::RmEff(effect) => {
                println!("   eff: {}/{}", effect.action, effect.effect_id);
                let act = model.actions.get_action(&effect.action).unwrap();
                // format effect for display (will tag the action name)
                // TODO: add span information of effect so we can properly display it inline
                let fmt_eff = model.env.node(&act.effects[effect.effect_id]).to_string();
                let annot = act.name.info(format!("rm effect: {fmt_eff})"));
                msg = msg.snippet(annot).show(effect.action.span.as_ref().unwrap());
            }
        }
    }
    msg
}

/// Scope from convertion function can find the values binded in their environments, (action sart, end, presence, parameters, ...)
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
    x: &planx::Effect,
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
        planx::EffectOp::Assign(e) => {
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

fn create_potential_effect(
    fid: FluentId,
    params: &[Param],
    enalber: Lit,
    model: &Model,
    sched: &mut Sched,
    bindings: &Scope,
) -> Res<aries_sched::Effect> {
    let t = bindings.start;
    let args: Vec<SymAtom> = params.iter().map(|p| bindings.args[p.name()]).collect_vec();
    let sv = aries_sched::StateVar {
        fluent: model.env.fluents.get(fid).name().to_string(),
        args,
    };
    let op = EffectOp::Assign(true);
    let eff = aries_sched::Effect {
        transition_start: t,
        transition_end: t + FAtom::EPSILON,
        mutex_end: sched.new_timepoint(),
        state_var: sv,
        operation: op,
        prez: enalber,
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
        _ => todo!(),
    }
}

fn reify_bool(e: ExprId, model: &Model, _sched: &mut Sched) -> Res<bool> {
    let e = model.env.node(e);
    match e.expr() {
        planx::Expr::Bool(b) => Ok(*b),
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
                value: Lit::TRUE.into(),
                timepoint: reify_timing(tp, model, sched, bindings)?,
                prez: bindings.presence,
            };
            Ok(Box::new(c))
        }
        e => todo!("{e:?}"),
    }
}
