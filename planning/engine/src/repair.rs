pub mod potential_effects;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    sync::Arc,
    time::Instant,
};

use aries_plan_engine::{
    encode::{encoding::Encoding, *},
    plans::lifted_plan::{self, LiftedPlan},
};
use aries_solver::lang::Store;
use aries_solver::model::extensions::DomainsExt;
use aries_solver::prelude::*;
use derive_more::derive::Display;
use itertools::Itertools;
use planx::{ActionRef, FluentId, Message, Model, Param, Res, Sym, errors::Spanned};
use timelines::{ConstraintID, EffectOp, Sched, SymAtom, Task, Time, boxes::Segment, explain::ExplainableSolver};

use crate::{
    ctags::{ActionCondition, ActionEffect, CTag, PotentialEffect, Repair},
    repair::potential_effects::PotentialEffects,
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

    if solver.check_satisfiability().is_some() {
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
                    aries_solver::solver::musmcs::MusMcs::Mus(elems) => {
                        mus_count += 1;
                        (Message::error("MUS"), elems)
                    }
                    aries_solver::solver::musmcs::MusMcs::Mcs(elems) => {
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
    let mut encoding = Encoding::new();

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
    let mut sched = timelines::Sched::new(1, objs);

    let global_scope = Scope::global(&sched);

    // associates each variable in the plan to a fresh variable.
    let plan_variables: BTreeMap<&Sym, SymAtom> = plan
        .variables
        .iter()
        .map(|(var_name, var_type)| {
            let type_bounds = sched
                .objects
                .domain_of_type(var_type.name.as_str())
                .ok_or_else(|| var_type.name.invalid("Could not determine the domain of this type."))?;
            let var: SymAtom = sched
                .model
                .new_ivar(type_bounds.first, type_bounds.last, var_name)
                .into();
            Ok::<_, Message>((var_name, var))
        })
        .try_collect()?;

    // associates each operation of the plan to its scope (binding of parameters, start/end, ...)
    let mut operations_scopes = Vec::with_capacity(plan.operations.len());

    // associates each action in the model with an over-approximation of the values taken by its parameters.
    let mut actions_instantiations: BTreeMap<(ActionRef, Param), Segment> = Default::default();

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
                lifted_plan::ObjectOrVariable::Ground(object) => sched
                    .objects
                    .object_atom(object.name().as_str())
                    .ok_or_else(|| object.name().invalid("unknown object"))?,
                // variable parameter, retrieve the variable we created for it
                lifted_plan::ObjectOrVariable::Variable { name } => plan_variables[name],
            };

            // incorpare the potential values taken by this operation param into the one of the action
            let seg = Segment::from(sched.model.bounds(arg));
            actions_instantiations
                .entry((a.name.clone(), param.clone()))
                .or_insert(seg)
                .union(&seg);

            // add argument to the bindings
            args.insert(&param.name, arg);
        }
        // start time is the index of the action in the plan
        let start = Time::from(op.start);
        let end = Time::from(op.start + op.duration);
        // action is necessarily present
        let presence = Lit::TRUE;

        let task_id = sched.add_task(Task {
            name: format!("operation{op_id}"),
            start,
            end,
            presence,
        });

        let bindings = Scope {
            start,
            end,
            presence,
            args,
            source: Some(task_id),
        };

        // for each condition, create a constraint stating it should hold. The constraint is tagged so we can later deactivate it
        for (cond_id, c) in a.conditions.iter().enumerate() {
            if let Some(tp) = c.interval.as_timestamp() {
                let constraint = condition_to_constraint(tp, c.cond, model, &mut sched, &bindings, &mut encoding)?;
                constraint.add_required_values(&mut encoding.required_values, model, &sched);

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
                    let constraint =
                        condition_to_constraint(tp, expr_id, model, &mut sched, &global_scope, &mut encoding)?;
                    constraint.add_required_values(&mut encoding.required_values, model, &sched);

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
    let param_bounds = |action_ref: &Sym, param: &Param| {
        actions_instantiations
            .get(&(action_ref.clone(), param.clone()))
            .copied()
            .unwrap_or(Segment::empty())
    };

    // compute the set of all effects that may be added to each action template
    // be give it the overapproximations of the value potentially required and of the values each action parameter may take to allow
    // eager discarding of useless potential effects.
    let potential_effects = Arc::new(PotentialEffects::compute(
        model,
        &encoding.required_values,
        param_bounds,
        || sched.model.new_optional_bool_var(Lit::TRUE),
    ));

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
                    let imposed_value = reify_constant(*expr_id, model, &mut sched, &global_scope, &mut encoding)?;
                    // note that this work when no required value may come from the effects themselves (ok in STRIPS)
                    let possibly_detrimental = encoding
                        .required_values
                        .may_require_value(eff.effect_expression.state_variable.fluent, 1 - imposed_value);
                    if possibly_detrimental {
                        // effect may delete a precondition, it must be relaxable and we tie its presence to a new literal
                        sched.model.new_optional_bool_var(Lit::TRUE)
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
        let eff = convert_effect(x, false, model, &mut sched, &global_scope, &mut encoding)?;
        sched.add_effect(eff);
    }

    for (op_id, _op) in plan.operations.iter().enumerate() {
        let (a, bindings) = &operations_scopes[op_id];

        // vec to accumulate all effects of the action.
        // this will then be post-processed to match the set-based semantics of PDDL (add-after-delete, ...)
        let mut action_effects = Vec::with_capacity(64);

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
            let mut eff = convert_effect(x, true, model, &mut sched, bindings, &mut encoding)?;
            // replace the effect presence by its enabler
            assert_eq!(eff.prez, Lit::TRUE);
            eff.prez = effect_enablers[&aeff];
            action_effects.push(eff);
        }

        // for each potential effect, add it as well (it will be assumed absent by default due to the global constraint)
        for (fid, params, value, enabler) in potential_effects.for_action(&a.name) {
            let eff = create_potential_effect(*fid, params.as_slice(), *value, *enabler, model, &mut sched, bindings)?;
            action_effects.push(eff);
        }

        // post process the effect to align them with PDDL semantics
        let action_effects = convert_to_pddl_set_semantics(action_effects, &mut sched);
        for eff in action_effects {
            sched.add_effect(eff);
        }
    }

    // set all default negative value (the call attempts to only put that may be useful)
    // this must be done last becaue it uses the required values gathered in all previous chapses.
    add_closed_world_negative_effects(&encoding.required_values, model, &mut sched);

    let constraint_to_repair = |cid: ConstraintID| match constraints_tags.get(&cid) {
        Some(ctag) => ctag.to_repair(),
        _ => None,
    };

    Ok(sched.explainable_solver(constraint_to_repair))
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
fn create_potential_effect(
    fid: FluentId,
    params: &[Param],
    value: bool,
    enalber: Lit,
    model: &Model,
    sched: &mut Sched,
    bindings: &Scope,
) -> Res<timelines::Effect> {
    let t = bindings.start;
    let args: Vec<SymAtom> = params.iter().map(|p| bindings.args[p.name()]).collect_vec();
    let sv = timelines::StateVar {
        fluent: model.env.fluents.get(fid).name().to_string(),
        args,
    };
    let value = if value { 1 } else { 0 };
    let op = EffectOp::Assign(value.into());
    let eff = timelines::Effect {
        transition_start: t,
        transition_end: t + sched.epsilon,
        mutex_end: sched.new_timepoint(),
        state_var: sv,
        operation: op,
        prez: enalber,
        source: bindings.source,
    };
    Ok(eff)
}
