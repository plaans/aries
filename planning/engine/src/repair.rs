pub mod lifted_plan;
pub mod potential_effects;
pub mod required_values;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    sync::Arc,
    time::Instant,
};

use aries::{
    core::{INT_CST_MAX, Lit},
    model::{
        extensions::AssignmentExt,
        lang::{FAtom, hreif::Store},
    },
};
use aries_sched::{
    ConstraintID, EffectOp, Sched, StateVar, SymAtom, Time, boxes::Segment, constraints::HasValueAt,
    explain::ExplainableSolver, symbols::ObjectEncoding,
};
use derive_more::derive::Display;
use itertools::Itertools;
use planx::{ActionRef, ExprId, FluentId, Message, Model, Param, Res, Sym, TimeRef, Timestamp, errors::Spanned};

use crate::{
    ctags::{ActionCondition, ActionEffect, CTag, PotentialEffect, Repair},
    repair::{lifted_plan::LiftedPlan, potential_effects::PotentialEffects, required_values::RequiredValues},
};

#[derive(clap::Args, Debug, Clone)]
pub struct RepairOptions {
    #[arg(long, default_value = "smallest")]
    pub mode: RepairMode,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, Display)]
pub enum RepairMode {
    Smallest,
    All,
}

#[derive(Display)]
#[display("{mode} {status:>12} {runtime:>10?} (runtime ms)  {encoding_time:>8?} (enctime ms)")]
pub struct RepairReport {
    pub status: RepairStatus,
    pub mode: RepairMode,
    pub runtime: u128,
    pub encoding_time: u128,
}
#[derive(Display)]
pub enum RepairStatus {
    /// The plan is valid without any modification
    #[display("VALID  ")]
    ValidPlan,
    /// A smallest MCS was found.
    #[display("SMCS({_0})")]
    SmallestFound(usize),
    /// The domain cannot be repaired automatically.
    /// There may be several reasons for this, a common one being that the plan is not valid in the first place.
    #[display("BROKEN ")]
    Unrepairable,
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
            if let Some(repair_set) = solver.find_smallest_mcs() {
                let msg = format_culprit_set(Message::error("Smallest MCS"), &repair_set, model);
                println!("\n\n{msg}");
                RepairReport {
                    status: RepairStatus::SmallestFound(repair_set.len()),
                    mode: options.mode,
                    runtime: start.elapsed().as_millis(),
                    encoding_time,
                }
            } else {
                RepairReport {
                    status: RepairStatus::Unrepairable,
                    mode: options.mode,
                    runtime: start.elapsed().as_millis(),
                    encoding_time,
                }
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
    // ignore all non boolean fluents. The only one we are expecting in classical planning are those for the action costs, which are irrelevant for this use case.
    let ignored = |eff: &planx::Effect| {
        let fluent_id = eff.effect_expression.state_variable.fluent;
        let fluent = model.env.fluents.get(fluent_id);
        fluent.return_type != planx::Type::Bool
    };

    // for each constraint we may which to relax, stores a CTag (constraint tag) so that we can later decide if it should be relaxed.
    let mut constraints_tags: BTreeMap<ConstraintID, CTag> = Default::default();

    // build encoding of all objects: associates each object to a int value and each type to a range of values
    let objs = types(model);
    let mut sched = aries_sched::Sched::new(1, objs);

    let global_scope = Scope::global(&sched);

    // overapproximation of values required at some point in the problem.
    // Will be populated as we encounter new conditions, goals, ...
    let mut required_values = RequiredValues::new();

    // associates each variable in the plan to a fresh variable.
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

    // associates each operation of the plan to its scope (binding of parameters, start/end, ...)
    let mut operations_scopes = Vec::with_capacity(plan.operations.len());

    // associates each action in the model with an overapproximation of the values taken by its parameters.
    let mut actions_instanciations: BTreeMap<(ActionRef, Param), Segment> = Default::default();

    // initial processing of all operations
    // we create its scope (binding of timepoints, params, ...) and process its conditions
    // Effects are defered to a later point
    for (op_id, op) in plan.operations.iter().enumerate() {
        // corresponding action in the model
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

            // incorpare the potential values taken by this operation param into the one of the action
            let seg = Segment::from(sched.model.int_bounds(arg));
            actions_instanciations
                .entry((a.name.clone(), param.clone()))
                .or_insert(seg)
                .union(&seg);

            // add argument to the bindings
            args.insert(&param.name, arg);
        }

        let bindings = Scope {
            start: Time::from(op.start), // start time is the index of the action in the plan
            end: Time::from(op.start + op.duration),
            presence: Lit::TRUE, // action is necessarily present
            args,
        };

        // for each condition, create a constraint stating it should hold. The constraint is tagged so we can later deactivate it
        for (cond_id, c) in a.conditions.iter().enumerate() {
            if let Some(tp) = c.interval.as_timestamp() {
                let constraint = condition_to_constraint(tp, c.cond, model, &mut sched, &bindings)?;

                let fluent_id = model
                    .env
                    .fluents
                    .get_by_name(&constraint.state_var.fluent)
                    .expect("no such fluent");
                // incorporate the value required by this condition into the global tracker
                required_values.add(fluent_id, constraint.value_box(|v| sched.model.int_bounds(v)).as_ref());

                let cid = sched.add_constraint(constraint);
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

        // store the scopes, we will need them when processing the effects
        operations_scopes.push((a, bindings));
    }
    // for each goal, add a constraint stating it must hold (the constriant is tagged but not relaxed for domain repair)
    for (gid, x) in model.goals.iter().enumerate() {
        assert!(x.universal_quantification.is_empty());
        match x.goal_expression {
            planx::SimpleGoal::HoldsDuring(time_interval, expr_id) => {
                if let Some(tp) = time_interval.as_timestamp() {
                    let constraint = condition_to_constraint(tp, expr_id, model, &mut sched, &global_scope)?;
                    let fluent_id = model
                        .env
                        .fluents
                        .get_by_name(&constraint.state_var.fluent)
                        .expect("no such fluent");
                    // incorporate the value required by this condition into the global tracker
                    required_values.add(fluent_id, constraint.value_box(|v| sched.model.int_bounds(v)).as_ref());

                    let cid = sched.add_constraint(constraint);
                    constraints_tags.insert(cid, CTag::EnforceGoal(gid));
                } else {
                    todo!()
                }
            }
            _ => todo!(),
        }
    }

    // make it immutable, we will start exploiting and want to guard against any addition
    let required_values = required_values;
    let param_bounds = |action_ref: &Sym, param: &Param| {
        actions_instanciations
            .get(&(action_ref.clone(), param.clone()))
            .copied()
            .unwrap_or(Segment::empty())
    };

    // compute the set of all effects that may be added to each action template
    // be give it the overapproximations of the value potentially required and of the values each action parameter may take to allow
    // eager discarding of useless potential effects.
    let potential_effects = Arc::new(PotentialEffects::compute(model, &required_values, param_bounds, || {
        sched.model.new_literal(Lit::TRUE)
    }));

    // for each potential effect, add a (soft constraint) that it is absent
    for (a, pot_effs) in &potential_effects.effs {
        for (eff_id, pot_eff) in pot_effs.iter().enumerate() {
            // create a constraint disabling the effect, and tag so that we can mark it as soft
            let cid = sched.add_constraint(!pot_eff.3);
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
            if ignored(eff) {
                continue;
            }
            let aeff = ActionEffect {
                action: a.name.clone(),
                effect_id: eff_id,
            };
            let enabler = match &eff.effect_expression.operation {
                planx::EffectOp::Assign(expr_id) => {
                    // hacky way to determine if the effect is positive (will only work for classical planning)
                    let imposed_value = reify_bool(*expr_id, model, &mut sched)?;
                    let possibly_detrimental =
                        required_values.may_require_value(eff.effect_expression.state_variable.fluent, !imposed_value);
                    if possibly_detrimental {
                        // effect may delete a precondition, it must be relaxable and we tie its presence to a new literal
                        sched.model.new_literal(Lit::TRUE)
                    } else {
                        // effect can never be detrimental and we thus always force its presence
                        Lit::TRUE
                    }
                }
                _ => todo!(), // numeric fluents, but those a ignored because the presence of numeric fluents for action costs)
            };
            // record the enabler and place a (soft) constraint forcing it to true
            effect_enablers.insert(aeff.clone(), enabler);
            let cid = sched.add_constraint(enabler);
            constraints_tags.insert(cid, CTag::EnforceEffect(aeff));
        }
    }

    // enforce all elemts of the initial state as effects
    for x in &model.init {
        if ignored(x) {
            continue;
        }
        let eff = convert_effect(x, false, model, &mut sched, &global_scope)?;
        sched.add_effect(eff);
    }
    // set all default negative value (the call attempts to only put that may be useful)
    add_closed_world_negative_effects(&required_values, model, &mut sched);

    for (op_id, _op) in plan.operations.iter().enumerate() {
        let (a, bindings) = &operations_scopes[op_id];
        // add an effect to the scheduling problem for each effect in the action template
        // the presence of the effect is controlled by the global enabler of the effect in the template
        for (eff_id, x) in a.effects.iter().enumerate() {
            if ignored(x) {
                continue;
            }
            let aeff = ActionEffect {
                action: a.name.clone(),
                effect_id: eff_id,
            };
            let mut eff = convert_effect(x, true, model, &mut sched, bindings)?;
            // replace the effect presence by its enabler
            assert_eq!(eff.prez, Lit::TRUE);
            eff.prez = effect_enablers[&aeff];
            sched.add_effect(eff);
        }

        // for each potential effect, add it as well (it will be assumed absent by default due to the global constraint)
        for (fid, params, value, enabler) in potential_effects.for_action(&a.name) {
            let eff = create_potential_effect(*fid, params.as_slice(), *value, *enabler, model, &mut sched, bindings)?;
            sched.add_effect(eff);
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
                .sorted() // sorting is unecessary but may be useful to group together similar objects in the absence of typing
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
                let (fluent_id, params, value, _) = potential_effect.get();
                let fluent = model.env.fluents.get(*fluent_id);
                let fluent = format!(
                    "({} {}) := {value}",
                    fluent.name(),
                    params.iter().map(|p| p.name()).format(" ")
                );
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

/// Add all required default negative values as effects just before the origin.
fn add_closed_world_negative_effects(reqs: &RequiredValues, model: &Model, sched: &mut Sched) {
    // time at which to place the negative effects. This is -1 so that it can be overrided by an initial effect (at 0).
    let t = Time::from(-1);

    // all state variables that may require a `false` value
    // we will only place a negative effect for those state variables.
    let req_state_vars = reqs.state_variables(|v| v == 0);

    for sv in req_state_vars {
        let args: Vec<SymAtom> = sv.params.0.into_iter().map(SymAtom::from).collect_vec();
        let sv = aries_sched::StateVar {
            fluent: model.env.fluents.get(sv.fluent).name().to_string(),
            args,
        };
        // we manually create the mutex-end since it may have a negative value if canceledd by an initial positive effect
        let mutex_end: Time = sched.model.new_fvar(-1, INT_CST_MAX, sched.time_scale, "_").into();
        let eff = aries_sched::Effect {
            transition_start: t,
            transition_end: t,
            mutex_end,
            state_var: sv,
            operation: EffectOp::Assign(false),
            prez: Lit::TRUE,
        };
        sched.add_effect(eff);
    }
}

fn create_potential_effect(
    fid: FluentId,
    params: &[Param],
    value: bool,
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
    let op = EffectOp::Assign(value);
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
) -> Res<HasValueAt> {
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
            Ok(c)
        }
        planx::Expr::App(planx::Fun::Not, exprs) if exprs.len() == 1 => {
            let mut c = condition_to_constraint(tp, exprs[0], model, sched, bindings)?;
            let Ok(x) = Lit::try_from(c.value) else {
                panic!();
            };
            c.value = x.not().into();
            Ok(c)
        }
        e => todo!("{e:?}"),
    }
}
