use std::{
    collections::BTreeMap,
    time::Instant,
};

use aries::{
    core::state::Evaluable,
    prelude::*,
};
use aries_plan_engine::{
    encode::{
        encoding::Encoding,
        tags::{Tag, format_culprit_set},
    },
    plans::lifted_plan::{self, LiftedPlan},
};
use derive_more::derive::Display;
use planx::{Model, Res, errors::*};
use timelines::{Sched, explain::ExplainableSolver};

use crate::optimize_plan;

pub type RelaxableConstraint = Tag;

#[derive(clap::Args, Debug, Clone)]
pub struct Options {
    #[arg(short, long)]
    pub max_depth: usize,

    // #[arg(short, long, num_args(1..))]
    // pub relaxation: Vec<Relaxation>,
    #[arg(short, long, default_value("plan-length"))]
    pub objective: Objective,
}

// #[derive(clap::ValueEnum, Debug, Clone, Copy, Display, PartialEq, PartialOrd, Eq, Ord)]
// pub enum Relaxation {
//     ActionPresence,
//     StartTime,
// }

#[derive(clap::ValueEnum, Debug, Clone, Copy, Display, PartialEq, Eq, PartialOrd, Ord)]
pub enum Objective {
    /// The objective value defined in the domain
    Original,
    PlanLength,
    Makespan,
}

pub fn solve_finite_planning_problem(model: &Model, options: &Options) -> Res<()> {
    let plan = &lifted_plan::new_empty_lifted_plan(model, BTreeMap::new(), options.max_depth)?;

    let start = Instant::now();
    let (mut solver, encoding, _sched) = encode_finite_planning_problem(model, plan, options)?;

    let _encoding_time = start.elapsed().as_millis();

    let objective = encoding.objective.unwrap(); //TODO: error message

    let print = |sol: &Solution| {
        println!("==== Plan (objective: {}) =====", objective.evaluate(sol).unwrap());
        println!("{}\n", encoding.plan(sol));
    };

    if let Some(solution) = solver.find_optimal(objective, &print) {
        println!("\n> Found optimal solution:");
        print(&solution);
    } else {
        println!("No solution !!!!");
        for mus in solver.muses() {
            let msg = format_culprit_set(Message::error("Invalid in all relaxation"), &mus, model, plan);
            println!("\n{msg}\n");
        }
    }
    Ok(())
}

pub fn encode_finite_planning_problem(
    model: &Model,
    lifted_plan: &LiftedPlan,
    options: &Options,
) -> Res<(ExplainableSolver<RelaxableConstraint>, Encoding, Sched)> {
    // TODO: make specific function.
    // - ability to specify explanations vocabulary via RelaxableConstraint (Tag), including removing (pre)conditions (like in domain repair).

    let objective = match options.objective {
        Objective::Original => optimize_plan::Objective::Original,
        Objective::PlanLength => optimize_plan::Objective::PlanLength,
        Objective::Makespan => optimize_plan::Objective::Makespan,
    };
    optimize_plan::encode_plan_optimization_problem(
        model,
        lifted_plan,
        &optimize_plan::Options {
            relaxation: vec![optimize_plan::Relaxation::ActionPresence, optimize_plan::Relaxation::StartTime],
            objective,
        },
    )
}
