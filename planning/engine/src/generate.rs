use std::{collections::BTreeMap, path::PathBuf, time::Instant};

use aries_plan_engine::{
    encode::{encoding::Encoding, tags::Tag},
    plans::lifted_plan::LiftedPlan,
};
use aries_solver::{core::state::Evaluable, prelude::*};
use planx::{Model, Res};
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
    pub max_instances: u32,

    /// Defines the objective to be minimized
    #[arg(short, long, default_value("original"))]
    pub objective: Objective,

    /// If set, the planner will try to find the optimal solution
    #[arg(long)]
    pub optimize: bool,

    /// If provided, the final plan will be written to this file.
    #[arg(short = 'w', long = "write-plan")]
    plan_file: Option<PathBuf>,
}

pub fn solve_finite_planning_problem(model: &Model, options: &Options) -> Res<()> {
    // create a dummy plan with the appropriate number of actions
    // this is temporary a workaround to reuse the existing `optimize_plan` facilities
    let plan = LiftedPlan::default();

    let start = Instant::now();
    let (mut solver, encoding, _sched) = encode_finite_planning_problem(model, &plan, options)?;

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
        encoding.plan(&solution).write_to_file(options.plan_file.as_ref())?;
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

    let num_free_instances_per_action =
        BTreeMap::from_iter(model.actions.iter().map(|a| (a.name.clone(), options.max_instances)));

    optimize_plan::encode_plan_optimization_problem(
        model,
        lifted_plan,
        num_free_instances_per_action,
        &optimize_plan::Options {
            relaxation: vec![
                optimize_plan::Relaxation::ActionPresence,
                optimize_plan::Relaxation::StartTime,
            ],
            objectives: vec![options.objective],
        },
    )
}
