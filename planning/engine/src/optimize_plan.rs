use std::{collections::BTreeMap, time::Instant};

use aries::prelude::*;
use aries_plan_engine::{
    encode::{required_values::RequiredValues, *},
    plans::lifted_plan::{self, LiftedPlan},
};
use derive_more::derive::Display;
use itertools::Itertools;
use planx::{ActionRef, Model, Param, Res, Sym, errors::*};
use timelines::{ConstraintID, SymAtom, Time, boxes::Segment, explain::ExplainableSolver};

pub type RelaxableConstraint = ();

#[derive(clap::Args, Debug, Clone)]
pub struct Options {
    #[arg(short, long, num_args(1..))]
    pub relaxation: Vec<Relaxation>,

    #[arg(short, long, default_value("plan-length"))]
    pub objective: Objective,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, Display)]
pub enum Relaxation {
    ActionPresence,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, Display)]
pub enum Objective {
    PlanLength,
}

pub fn optimize_plan(model: &Model, plan: &LiftedPlan, options: &Options) -> Res<()> {
    let start = Instant::now();
    let mut solver = encode_plan_optimization_problem(model, plan, options)?;

    let _encoding_time = start.elapsed().as_millis();

    if let Some(_solution) = solver.check_satisfiability() {
        println!("Plan is valid.");
        return Ok(());
    } else {
        println!("Invalid plan");
    }
    todo!()
}

pub fn encode_plan_optimization_problem(
    model: &Model,
    lifted_plan: &LiftedPlan,
    _options: &Options, // TODO: use those
) -> Res<ExplainableSolver<RelaxableConstraint>> {
    // build encoding of all objects: associates each object to a int value and each type to a range of values
    let objs = types(model);
    let mut sched = timelines::Sched::new(1, objs);

    let global_scope = Scope::global(&sched);

    // overapproximation of values required at some point in the problem.
    // Will be populated as we encounter new conditions, goals, ...
    let mut required_values = RequiredValues::new();

    // associates each variable in the plan to a fresh variable.
    // TODO: presence of the variable
    let plan_variables: BTreeMap<&Sym, SymAtom> = lifted_plan
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
    let mut operations_scopes = Vec::with_capacity(lifted_plan.operations.len());

    // associates each action in the model with an overapproximation of the values taken by its parameters.
    let mut actions_instanciations: BTreeMap<(ActionRef, Param), Segment> = Default::default();

    // initial processing of all operations
    // we create its scope (binding of timepoints, params, ...) and process its conditions
    // Effects are defered to a later point
    for op in lifted_plan.operations.iter() {
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
                    .object_atom(object.name().canonical_str())
                    .ok_or_else(|| object.name().invalid("unknown object"))?,
                // variable parameter, retrieve the variable we created for it
                lifted_plan::ObjectOrVariable::Variable { name } => plan_variables[name],
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
            presence: Lit::TRUE, // TODO: action is necessarily present!!
            args,
        };

        // for each condition, create a constraint stating it should hold. The constraint is tagged so we can later deactivate it
        for c in a.conditions.iter() {
            if let Some(tp) = c.interval.as_timestamp() {
                let constraint =
                    condition_to_constraint(tp, c.cond, model, &mut sched, &bindings, Some(&mut required_values))?;

                sched.add_constraint(constraint);
            }
        }

        // store the scopes, we will need them when processing the effects
        operations_scopes.push((a, bindings));
    }
    // for each goal, add a constraint stating it must hold (the constriant is tagged but not relaxed for domain repair)
    for x in model.goals.iter() {
        assert!(x.universal_quantification.is_empty());
        match x.goal_expression {
            planx::SimpleGoal::HoldsDuring(time_interval, expr_id) => {
                if let Some(tp) = time_interval.as_timestamp() {
                    let constraint = condition_to_constraint(
                        tp,
                        expr_id,
                        model,
                        &mut sched,
                        &global_scope,
                        Some(&mut required_values),
                    )?;

                    sched.add_constraint(constraint);
                } else {
                    todo!("durative goal")
                }
            }
            _ => todo!("complex goal"),
        }
    }

    // make it immutable, we will start exploiting and want to guard against any addition
    let required_values = required_values;

    // enforce all elemts of the initial state as effects
    for x in &model.init {
        let eff = convert_effect(x, false, model, &mut sched, &global_scope)?;
        sched.add_effect(eff);
    }
    // set all default negative value
    // The function attempts to only put those that may be useful, based on the required values
    add_closed_world_negative_effects(&required_values, model, &mut sched);

    for (op_id, _op) in lifted_plan.operations.iter().enumerate() {
        let (a, bindings) = &operations_scopes[op_id];

        // vec to accumulate all effects of the action.
        // these will then be post-processed to match the set-based semantics of PDDL (add-after-delete, ...)
        let mut action_effects = Vec::with_capacity(64);

        // add an effect to the scheduling problem for each effect in the action template
        // the presence of the effect is controlled by the global enabler of the effect in the template
        for x in a.effects.iter() {
            let eff = convert_effect(x, true, model, &mut sched, bindings)?;
            // replace the effect presence by its enabler
            assert_eq!(eff.prez, Lit::TRUE);
            action_effects.push(eff);
        }

        // post process the effect to align them with PDDL semantics
        let action_effects = convert_to_pddl_set_semantics(action_effects, &mut sched);
        for eff in action_effects {
            sched.add_effect(eff);
        }
    }

    let constraint_to_repair = |_cid: ConstraintID| None;

    Ok(sched.explainable_solver(constraint_to_repair))
}
