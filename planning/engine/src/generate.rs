use std::{collections::BTreeMap, time::Instant};

use aries::{core::state::Evaluable, prelude::*};
use aries_plan_engine::{
    encode::{encoding::Encoding, tags::Tag},
    plans::{
        Operation,
        lifted_plan::{LiftedPlan, ObjectOrVariable},
    },
};
use planx::{Model, Res, Sym};
use timelines::{Sched, explain::ExplainableSolver};

use crate::optimize_plan::{self, Objective};

pub type RelaxableConstraint = Tag;

#[derive(clap::Args, Debug, Clone)]
pub struct Options {
    /// Defines the maximum number of instances per action template.
    ///
    /// For instance, if set to 3, the resulting plan may have *at most* three instances
    /// of a `pick` action and at most 3 instances of a `drop` action.
    #[arg(short, long)]
    pub max_instances: usize,

    /// Defines the objective to be minimized
    #[arg(short, long, default_value("original"))]
    pub objective: Objective,

    /// If set, the planner will try tro find the optimal solution
    #[arg(long)]
    pub optimize: bool,
}

pub fn solve_finite_planning_problem(model: &Model, options: &Options) -> Res<()> {
    // create a dummy plan with the appropriate number of actions
    // this is temporary a workaround to reuse the existing `optimize_plan` facilities
    let plan = &new_empty_lifted_plan(model, BTreeMap::new(), options.max_instances)?;

    let start = Instant::now();
    let (mut solver, encoding, _sched) = encode_finite_planning_problem(model, plan, options)?;

    let _encoding_time = start.elapsed().as_millis();

    let objective = encoding
        .objectives
        .first()
        .copied()
        .expect("no objective specified (no default)");

    // set the objective to a constant if we are not optimizing
    let solver_objective = if options.optimize { objective } else { 0.into() };

    let print = |sol: &Solution| {
        println!("\n==== Plan (objective: {}) =====", objective.evaluate(sol).unwrap());
        println!("{}\n", encoding.plan(sol));
    };

    if let Some(solution) = solver.find_optimal(solver_objective, &print, []) {
        println!("\n> Found {}solution:", if options.optimize { "optimal " } else { "" });
        print(&solution);
    } else {
        println!("No solution !!!!");
    }
    Ok(())
}

fn encode_finite_planning_problem(
    model: &Model,
    lifted_plan: &LiftedPlan,
    options: &Options,
) -> Res<(ExplainableSolver<RelaxableConstraint>, Encoding, Sched)> {
    // TODO: make specific function.
    // - ability to specify explanations vocabulary via RelaxableConstraint (Tag), including removing (pre)conditions (like in domain repair).

    optimize_plan::encode_plan_optimization_problem(
        model,
        lifted_plan,
        &optimize_plan::Options {
            relaxation: vec![
                optimize_plan::Relaxation::ActionPresence,
                optimize_plan::Relaxation::StartTime,
            ],
            objectives: vec![options.objective],
        },
    )
}

fn new_empty_lifted_plan(
    model: &Model,
    a_instances_per_template: BTreeMap<planx::ActionRef, usize>,
    a_instances_default: usize,
) -> Res<LiftedPlan> {
    let top_type = model.env.types.top_user_type();
    use planx::errors::*;

    let num_instances = |a_name| *a_instances_per_template.get(a_name).unwrap_or(&a_instances_default);

    // all actions in the plan
    let mut operations = Vec::with_capacity(model.actions.iter().map(|a| num_instances(&a.name)).sum());

    // all variables appearing in the plan
    let mut variables = BTreeMap::new();

    for a in model.actions.iter() {
        for aid in 0..num_instances(&a.name) {
            let mut arguments = Vec::with_capacity(a.parameters.len());

            for param in a.parameters.iter() {
                let name = Sym::with_source(
                    format!("{}.{}.{}", a.name, aid, param.name()),
                    param.name().span_or_default(),
                );
                let tpe = if let planx::Type::User(tpe) = param.tpe() {
                    tpe.to_single_type().unwrap_or_else(|| top_type.clone())
                } else {
                    top_type.clone()
                };

                variables.insert(name.clone(), tpe);

                arguments.push(ObjectOrVariable::Variable { name });
            }
            operations.push(Operation {
                start: 0,
                duration: 0,
                action_ref: a.name.clone(),
                arguments,
                span: None,
            });
        }
    }
    Ok(LiftedPlan { operations, variables })
}
