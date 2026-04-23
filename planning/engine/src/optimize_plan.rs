use std::{collections::BTreeMap, fmt::Debug, time::Instant};

use aries::{
    core::{state::Evaluable, views::Boundable},
    model::lang::{IntExpr, Store},
    prelude::*,
};
use aries_plan_engine::{
    encode::{
        constraints::{ConditionConstraint, ReificationConstraint},
        encoding::{ActionInstance, Encoding, ObjectVar},
        tags::{ActionCondition, OperatorId, Tag, format_culprit_set},
        *,
    },
    plans::lifted_plan::{self, LiftedPlan},
};
use derive_more::derive::Display;
use itertools::Itertools;

use planx::{ActionRef, Duration, Goal, Model, Res, SimpleGoal, Sym, Type, errors::*};
use std::path::Path;
use timelines::{ConstraintID, IntExp, IntTerm, Sched, SymAtom, Task, Time, explain::ExplainableSolver};

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
    /// Number of actions in the plan
    PlanLength,
    /// End time of the latest action
    Makespan,
}

pub fn optimize_plan(model: &Model, plan: &LiftedPlan, options: &Options, output_plan: Option<&Path>) -> Res<()> {
    let start = Instant::now();
    // Encode the planning problem into a constraint satisfaction problem
    let (mut solver, encoding, _sched) = encode_plan_optimization_problem(model, plan, Default::default(), options)?;
    let _encoding_time = start.elapsed().as_millis();

    // Pinning literals from previous phases; grows as objectives are solved
    let mut phase_assumptions: Vec<Lit> = vec![];
    let mut last_solution = None;

    let print_plan = |sol: &Solution| {
        println!(
            "==== Plan (objectives: {:?}) =====\n\n{}",
            encoding
                .objectives
                .iter()
                .map(move |o| o.evaluate(sol).unwrap())
                .format(" / "),
            encoding.plan(sol)
        );
    };
    // Solve objectives lexicographically: each phase fixes the previous optimal values
    for objective in encoding.objectives.iter().copied() {
        // Minimize objective under normal constraints + previous pinnings
        let Some(sol) = solver.find_optimal(objective, &print_plan, phase_assumptions.as_slice()) else {
            println!("No solution !!!!");
            for mus in solver.muses() {
                let msg = format_culprit_set(Message::error("Invalid in all relaxation"), &mus, model, plan);
                println!("\n{msg}\n");
            }
            return Ok(());
        };

        // Pin objective == opt_val for subsequent phases (upper + lower bound)
        let opt_val = sol.eval(objective).unwrap();
        phase_assumptions.push(objective.leq(opt_val)); // objective ≤ opt_val
        phase_assumptions.push(objective.geq(opt_val)); // objective ≥ opt_val
        last_solution = Some(sol);
    }

    if let Some(solution) = last_solution {
        println!("> Found optimal solution\n");
        print_plan(&solution);
        let plan = encoding.plan(&solution);
        plan.write_to_file(output_plan)?;
    }

    Ok(())
}

fn build_objective(
    objective: &Objective,
    model: &Model,
    sched: &mut Sched,
    bindings: &Scope,
    encoding: &mut Encoding,
) -> Res<LinTerm> {
    Ok(match objective {
        Objective::Original if model.metric.is_some() => {
            // TODO: use if let guard when stabilized
            let metric = model.metric.unwrap();
            match metric {
                planx::Metric::Minimize(expr_id) => {
                    let lin_obj = reify_expression(expr_id, Some(sched.horizon), model, sched, bindings, encoding)?;
                    flatten_expression(lin_obj, sched, bindings)
                }
                planx::Metric::Maximize(_) => {
                    return Message::error("unsupported maximization metric").failed();
                }
            }
        }
        // Fall back to plan length when no metric is defined in the domain
        Objective::PlanLength | Objective::Original => {
            let mut sum = LinSum::zero();
            for t in sched.tasks.iter() {
                sum += timelines::constraints::bool2int(t.presence, &mut sched.model);
            }
            reify_sum(sum, sched)
        }
        Objective::Makespan => sched.makespan.into(),
    })
}

/// Creates a variable for a parameter with a given type and scope.
fn create_param_variable(var_name: &Sym, var_type: &Type, scope: Lit, sched: &mut Sched) -> Res<SymAtom> {
    let Type::User(var_type) = var_type else {
        return var_name.todo("Unsupported parameter type").failed();
    };
    let Some(var_type) = var_type.to_single_type() else {
        return var_name.todo("Unsupported parameter type (union type)").failed();
    };
    let type_bounds = sched
        .objects
        .domain_of_type(var_type.name.as_str())
        .ok_or_else(|| var_type.name.invalid("Could not determine the domain of this type."))?;
    let var: SymAtom = sched
        .model
        .new_optional_ivar(type_bounds.first, type_bounds.last, scope, var_name)
        .into();
    Ok::<_, Message>(var)
}

pub fn encode_plan_optimization_problem(
    model: &Model,
    lifted_plan: &LiftedPlan,
    free_actions: BTreeMap<ActionRef, u32>,
    options: &Options,
) -> Res<(ExplainableSolver<RelaxableConstraint>, Encoding, Sched)> {
    let mut encoding = Encoding::new();

    // build encoding of all objects: associates each object to a int value and each type to a range of values
    let objs = types(model);
    let object_decoder = objs.decoder();
    let mut sched = timelines::Sched::new(1, objs);

    let global_scope = Scope::global(&sched);

    // associates each variable in the plan to a fresh variable.
    // TODO: presence of the variable
    let plan_variables: BTreeMap<&Sym, SymAtom> = lifted_plan
        .variables
        .iter()
        .map(|(var_name, var_type)| {
            create_param_variable(var_name, &var_type.clone().into(), Lit::TRUE, &mut sched).map(|var| (var_name, var))
        })
        .try_collect()?;

    // associates each operation of the plan to its scope (binding of parameters, start/end, ...)
    let mut operations_scopes = Vec::with_capacity(lifted_plan.operations.len());

    // initial processing of all operations in the plan
    // we create its scope (binding of timepoints, params, ...) and process its conditions
    // Effects are deferred to a later point
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
                    .object_atom(object.name().as_str())
                    .ok_or_else(|| object.name().invalid("unknown object"))?,
                // variable parameter, retrieve the variable we created for it
                lifted_plan::ObjectOrVariable::Variable { name } => plan_variables[name],
            };

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
        assert_eq!(op.duration, 0, "we use the start as end");
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
            arguments: a
                .parameters
                .iter()
                .map(|param| ObjectVar::new(bindings.args[&param.name], &object_decoder))
                .collect(),
        });

        // store the scopes, we will need them when processing the effects
        operations_scopes.push((a, bindings, OperatorId::FromPlan(op_id)));
    }

    // create all free actions. These will be pushed to `operation_scopes` and after this step will be indistinguishable from actions in the plan.
    for (action_name, n) in free_actions {
        for instance in 0..n {
            let a = model
                .actions
                .get_action(&action_name)
                .ok_or_else(|| action_name.invalid("cannot find corresponding action"))?;

            // free actions are optional
            let presence = sched.model.new_literal(Lit::TRUE);

            // building a scope object so that downstream methods can find the value to replace the actions params/start/end/prez with

            // Create all arguments, that shared the same scope `presence` as the action
            let mut args = im::OrdMap::new();
            for param in a.parameters.iter() {
                let arg = create_param_variable(&param.name, param.tpe(), presence, &mut sched)?;
                args.insert(&param.name, arg);
            }
            let start = sched.new_opt_timepoint(presence);
            if a.duration != Duration::Instantaneous {
                return a.name.todo("Unsupported non-instantaneous action").failed();
            }
            let end = start;

            // record a task in `Sched` which
            //  - gives a task id (use by the condition enforcement constraints to enforce mutex conditions)
            //  - make the scheduler aware of the tasks when computing the makespan.
            let task_id = sched.add_task(Task {
                name: a.name.to_string(),
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
                arguments: a
                    .parameters
                    .iter()
                    .map(|param| ObjectVar::new(bindings.args[&param.name], &object_decoder))
                    .collect(),
            });

            // store the scopes, we will need them when processing the effects
            operations_scopes.push((
                a,
                bindings,
                OperatorId::FreeInsertion {
                    action_name: a.name.clone(),
                    instance,
                },
            ));
        }
    }

    for (a, bindings, op_id) in &operations_scopes {
        // for each condition, create a constraint stating it should hold. The constraint is tagged so we can later deactivate it
        for (cond_id, c) in a.conditions.iter().enumerate() {
            if let Some(tp) = c.interval.as_timestamp() {
                let constraint = condition_to_constraint(tp, c.cond, model, &mut sched, bindings, &mut encoding)?;
                // update the required values if requested by caller
                constraint.add_required_values(&mut encoding.required_values, model, &sched);

                let cid = sched.add_constraint(constraint);
                encoding.constraints_tags.insert(
                    cid,
                    Tag::Support {
                        operator_id: op_id.clone(),
                        cond: ActionCondition {
                            action: a.name.clone(),
                            condition_id: cond_id,
                        },
                    },
                );
            }
        }
    }

    // for each goal, add a constraint stating it must hold (the constraint is tagged but not relaxed for domain repair)
    for (gid, x) in model.goals.iter().enumerate() {
        let constraint = parse_goal(x, model, &mut sched, &global_scope, &mut encoding)?;
        constraint.add_required_values(&mut encoding.required_values, model, &sched);
        let cid = sched.add_constraint(constraint);
        encoding.constraints_tags.insert(cid, Tag::EnforceGoal(gid));
    }

    for pref in model.preferences.iter() {
        assert!(pref.universal_quantification.is_empty());
        // parse the goal into an equivalent expression
        let pref_satisfied = parse_goal(&pref.goal, model, &mut sched, &global_scope, &mut encoding)?;

        // reify the expression into a literal that is true iff the preference is satisfied
        let reification = sched.model.new_bvar(&pref.name).true_lit();
        let constraint = ReificationConstraint {
            reification,
            constraint: pref_satisfied,
        };

        constraint.add_required_values(&mut encoding.required_values, model, &sched);
        sched.add_constraint(constraint);

        // record the association of the preference with the literal
        encoding
            .preferences
            .entry(pref.name.to_string())
            .or_default()
            .push(reification);
    }

    // enforce all elements of the initial state as effects
    for x in &model.init {
        let eff = convert_effect(x, false, model, &mut sched, &global_scope, &mut encoding)?;
        sched.add_effect(eff);
    }

    for (a, bindings, _op_id) in &operations_scopes {
        // vec to accumulate all effects of the action.
        // these will then be post-processed to match the set-based semantics of PDDL (add-after-delete, ...)
        let mut action_effects = Vec::with_capacity(16);
        // store the effects on predicates independently at first because they will need post-processing
        let mut predicate_action_effects = Vec::with_capacity(16);

        // add an effect to the scheduling problem for each effect in the action template
        // the presence of the effect is controlled by the global enabler of the effect in the template
        for x in a.effects.iter() {
            let eff = convert_effect(x, true, model, &mut sched, bindings, &mut encoding)?;
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

    // Build all objectives
    for obj in &options.objectives {
        let obj = build_objective(obj, model, &mut sched, &global_scope, &mut encoding)?;
        encoding.add_objective(obj);
    }

    // set all default negative value
    // The function attempts to only put those that may be useful, based on the required values
    // Important: this MUST be done last so we have already identified all values that may be required (inside conditions, effect values, goals...)
    add_closed_world_negative_effects(&encoding.required_values, model, &mut sched);

    let tags = encoding.constraints_tags.clone();
    let constraint_to_repair = |cid: ConstraintID| tags.get(&cid).cloned();

    Ok((sched.explainable_solver(constraint_to_repair), encoding, sched))
}

fn reify_sum(sum: IntExp, model: &mut Sched) -> IntTerm {
    sum.reify(sum.conj_scope(&model), &mut model.model)
}

/// Parses a goal (possibly quantified) into an equivalent expression
pub fn parse_goal(
    goal: &Goal,
    model: &Model,
    sched: &mut Sched,
    bindings: &Scope,
    encoding: &mut Encoding,
) -> Res<ConditionConstraint> {
    if !goal.universal_quantification.is_empty() {
        return model
            .env
            .node(goal)
            .todo("Unsupported universal quantification")
            .failed();
    }
    parse_simple_goal(&goal.goal_expression, model, sched, bindings, encoding)
}

/// Parses a quantifier-free goal into an equivalent expression
pub fn parse_simple_goal(
    goal: &SimpleGoal,
    model: &Model,
    sched: &mut Sched,
    bindings: &Scope,
    encoding: &mut Encoding,
) -> Res<ConditionConstraint> {
    match goal {
        planx::SimpleGoal::HoldsDuring(time_interval, expr_id) => {
            if let Some(tp) = time_interval.as_timestamp() {
                condition_to_constraint(tp, *expr_id, model, sched, bindings, encoding)
            } else {
                todo!("durative goal")
            }
        }
        _ => todo!("complex goal"),
    }
}
