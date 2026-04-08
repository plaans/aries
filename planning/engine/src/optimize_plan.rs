use std::{collections::BTreeMap, time::Instant};

use aries::{
    core::state::Evaluable,
    model::lang::{FAtom, Store, linear::LinearSum},
    prelude::*,
};
use aries_plan_engine::{
    encode::{
        encoding::{ActionInstance, Encoding, ObjectVar},
        required_values::RequiredValues,
        tags::{ActionCondition, Tag, format_culprit_set},
        *,
    },
    plans::lifted_plan::{self, LiftedPlan},
};
use derive_more::derive::Display;
use itertools::Itertools;
use planx::{ActionRef, Model, Param, Res, Sym, errors::*};
use timelines::{ConstraintID, Sched, SymAtom, Task, Time, boxes::Segment, explain::ExplainableSolver, rational::QCst};
use aries::core::views::Boundable;

pub type RelaxableConstraint = Tag;

#[derive(clap::Args, Debug, Clone)]
pub struct Options {
    #[arg(short, long, num_args(1..))]
    pub relaxation: Vec<Relaxation>,

    #[arg(short, long, num_args(1..), default_values(["original"]))]
    pub objectives: Vec<Objective>,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, Display, PartialEq, PartialOrd, Eq, Ord)]
pub enum Relaxation {
    ActionPresence,
    StartTime,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, Display, PartialEq, Eq, PartialOrd, Ord)]
pub enum Objective {
    /// The objective value defined in the domain
    Original,
    PlanLength,
    Makespan,
}

pub fn optimize_plan(model: &Model, plan: &LiftedPlan, options: &Options) -> Res<()> {
    let start = Instant::now();
    // Encode the planning problem into a constraint satisfaction problem
    let (mut solver, encoding, _sched) = encode_plan_optimization_problem(model, plan, options)?;
    let _encoding_time = start.elapsed().as_millis();

    // Pinning literals from previous phases; grows as objectives are solved
    let mut phase_assumptions: Vec<Lit> = vec![];
    let mut last_solution = None;

    // Solve objectives lexicographically: each phase fixes the previous optimal values
    for objective in encoding.objective.iter() {
        // Minimize objective under normal constraints + previous pinnings
        let Some(sol) = solver.find_optimal(*objective, |_| {}, phase_assumptions.clone()) else {
            println!("No solution !!!!");
            for mus in solver.muses() {
                let msg = format_culprit_set(Message::error("Invalid in all relaxation"), &mus, model, plan);
                println!("\n{msg}\n");
            }
            return Ok(());
        };

        // Pin objective == opt_val for subsequent phases (upper + lower bound)
        let opt_val = sol.eval(objective.num).unwrap();
        phase_assumptions.push(objective.num.leq(opt_val));         // objective ≤ opt_val
        phase_assumptions.push(!objective.num.leq(opt_val - 1));    // objective ≥ opt_val
        last_solution = Some(sol);
    }

    if let Some(solution) = last_solution {
        let last_objective = encoding.objective.last().unwrap();
        println!("==== Plan (objective: {}) =====\n", last_objective.evaluate(&solution).unwrap());
        println!("{}\n", encoding.plan(&solution));
    }

    Ok(())
}

fn build_objective(
    objective: &Objective,
    model: &Model,
    sched: &mut Sched,
    operations_scopes: &[(&planx::Action, Scope)], // <-- necesario para PlanLength
    global_scope: &Scope,
) -> Res<FAtom> {
    Ok(match objective {
        Objective::Original if model.metric.is_some() => {
            // TODO: is if let guard when stabilized
            let metric = model.metric.unwrap();
            match metric {
                planx::Metric::Minimize(expr_id) => {
                    let lin_obj = reify_expression(expr_id, Some(sched.horizon), model, sched, global_scope)?;
                    let obj = flatten_expression(expr_id, lin_obj, model, sched, global_scope)?;
                    FAtom::new(obj, 1)
                }
                planx::Metric::Maximize(_) => {
                    return Message::error("unsupported maximization metric").failed();
                }
            }
        }
        // Fall back to plan length when no metric is defined in the domain
        Objective::PlanLength | Objective::Original => {
            let mut sum = LinearSum::zero();
            for (_a, scope) in operations_scopes {
                sum += timelines::constraints::bool2int(scope.presence, &mut sched.model);
            }
            reify_sum(sum, sched)
        }
        Objective::Makespan => sched.makespan,
    })
}

pub fn encode_plan_optimization_problem(
    model: &Model,
    lifted_plan: &LiftedPlan,
    options: &Options,
) -> Res<(ExplainableSolver<RelaxableConstraint>, Encoding, Sched)> {
    let mut encoding = Encoding::new();

    // build encoding of all objects: associates each object to a int value and each type to a range of values
    let objs = types(model);
    let object_decoder = objs.decoder();
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
    for (op_id, op) in lifted_plan.operations.iter().enumerate() {
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
        let presence = if options.relaxation.contains(&Relaxation::ActionPresence) {
            sched.model.new_literal(Lit::TRUE)
        } else {
            Lit::TRUE
        };
        let start = if options.relaxation.contains(&Relaxation::StartTime) {
            sched.new_opt_timepoint(presence)
        } else {
            Time::from(op.start)
        };
        assert_eq!(op.duration, QCst::ZERO, "we use the start as end");
        let end = start;

        // record a task in `Sched` which
        //  - gives a task id (use by the condition enforcement constraints to enforce mutex conditions)
        //  - make the scheduler aware of the tasks when computing the makespan.
        let task_id = sched.add_task(Task {
            name: format!("op{op_id}"),
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
        // add the action to the encoding so we can retrieve it later
        encoding.add_action(ActionInstance {
            action_ref: a.name.clone(),
            prez: bindings.presence,
            start: bindings.start,
            end: bindings.end,
            arguments: bindings
                .args
                .values()
                .map(|var| ObjectVar::new(*var, &object_decoder))
                .collect(),
        });

        // for each condition, create a constraint stating it should hold. The constraint is tagged so we can later deactivate it
        for (cond_id, c) in a.conditions.iter().enumerate() {
            if let Some(tp) = c.interval.as_timestamp() {
                let constraint =
                    condition_to_constraint(tp, c.cond, model, &mut sched, &bindings, Some(&mut required_values))?;

                let cid = sched.add_constraint(constraint);
                encoding.constraints_tags.insert(
                    cid,
                    Tag::Support {
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

    // for each goal, add a constraint stating it must hold (the constraint is tagged but not relaxed for domain repair)
    for (gid, x) in model.goals.iter().enumerate() {
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

                    let cid = sched.add_constraint(constraint);
                    encoding.constraints_tags.insert(cid, Tag::EnforceGoal(gid));
                } else {
                    todo!("durative goal")
                }
            }
            _ => todo!("complex goal"),
        }
    }

    // make it immutable, we will start exploiting and want to guard against any addition
    let required_values = required_values;

    // enforce all elements of the initial state as effects
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
        let mut action_effects = Vec::with_capacity(16);
        // store the effects on predicates independently at first because they will need post-processing
        let mut predicate_action_effects = Vec::with_capacity(16);

        // add an effect to the scheduling problem for each effect in the action template
        // the presence of the effect is controlled by the global enabler of the effect in the template
        for x in a.effects.iter() {
            let eff = convert_effect(x, true, model, &mut sched, bindings)?;
            // store the effect either in the global pool or in the predicate specific one
            let is_predicate = model
                .env
                .fluents
                .get(x.effect_expression.state_variable.fluent)
                .return_type
                == planx::Type::Bool;
            if is_predicate {
                predicate_action_effects.push(eff);
            } else {
                action_effects.push(eff);
            }
        }

        // post process the effects on predicates to align them with PDDL semantics
        let predicate_action_effects = convert_to_pddl_set_semantics(predicate_action_effects, &mut sched);

        // merge the post-processed effects on predicate in the global effect set
        action_effects.extend(predicate_action_effects);
        for eff in action_effects {
            sched.add_effect(eff);
        }
    }

    let tags = encoding.constraints_tags.clone();
    let constraint_to_repair = |cid: ConstraintID| tags.get(&cid).cloned();

    // Build all objectives
    for obj in &options.objectives {
        let obj = build_objective(obj, model, &mut sched, &operations_scopes, &global_scope)?;
        encoding.set_objective(obj);
    }

    Ok((sched.explainable_solver(constraint_to_repair), encoding, sched))
}

fn reify_sum(sum: LinearSum, model: &mut Sched) -> FAtom {
    let reified: FAtom = model
        .model
        .new_fvar(INT_CST_MIN, INT_CST_MAX, sum.denom(), "Sum reif")
        .into();
    model.add_constraint(sum.clone().leq(reified));
    model.add_constraint(sum.geq(reified));

    reified
}
