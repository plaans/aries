use aries_plan_engine::{encode::tags::format_culprit_set, plans::lifted_plan::LiftedPlan};
use itertools::Itertools;
use planx::{Message, Model, Res};

use crate::optimize_plan::encode_plan_optimization_problem;

#[derive(clap::Args, Debug, Clone)]
pub struct Options {}

pub enum ValidationResult {
    Valid { validated_plan: planx::SolutionPlan },
    Invalid,
}

pub fn validate(model: &Model, plan: &LiftedPlan, _options: &Options) -> Res<ValidationResult> {
    // we frame the problem as an optimization problem with no relaxation,
    // hence the solver is forced to reproduce the plan
    let opt_options = crate::optimize_plan::Options {
        relaxation: vec![], // no relaxation
        objectives: vec![], // unused
    };
    let (mut solver, encoding, _sched) =
        encode_plan_optimization_problem(model, plan, Default::default(), &opt_options, true)?;

    if let Some(solution) = solver.check_satisfiability() {
        println!("> Plan is valid");
        let plan = encoding.plan(&solution);
        let plan = plan.export(model)?;
        println!("> Objective: {}", plan.metrics.iter().format(" -- "));
        // _sched.print(&solution);
        Ok(ValidationResult::Valid { validated_plan: plan })
    } else {
        println!("Plan is INVALID!!!!");
        for mus in solver.muses() {
            let msg = format_culprit_set(Message::error("INVALID PLAN"), &mus, model, plan);
            println!("\n{msg}\n");
        }
        println!("Plan is INVALID!!!!");
        Ok(ValidationResult::Invalid)
    }
}
