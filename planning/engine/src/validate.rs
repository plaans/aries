use aries::core::state::Evaluable;
use aries_plan_engine::{encode::tags::format_culprit_set, plans::lifted_plan::LiftedPlan};
use planx::{Message, Model, Res};

use crate::optimize_plan::{self, encode_plan_optimization_problem};

#[derive(clap::Args, Debug, Clone)]
pub struct Options {}

pub enum ValidationResult {
    Valid { objective_value: Option<timelines::IntCst> },
    Invalid,
}

pub fn validate(model: &Model, plan: &LiftedPlan, _options: &Options) -> Res<ValidationResult> {
    // we frame the problem as an optimization problem with no relaxation,
    // hence the solver is forced to reproduce the plan
    let opt_options = crate::optimize_plan::Options {
        relaxation: vec![],                            // no relaxation
        objective: optimize_plan::Objective::Original, // TODO: change to domain's metric
    };
    let (mut solver, encoding, _sched) = encode_plan_optimization_problem(model, plan, &opt_options)?;

    if let Some(solution) = solver.check_satisfiability() {
        println!("> Plan is valid");
        let objective_value = if let Some(objective) = encoding.objective {
            let objective_value = objective.evaluate(&solution).unwrap();
            println!("> Objective: {objective_value}",);
            Some(objective_value)
        } else {
            None
        };
        // sched.print(&solution);
        Ok(ValidationResult::Valid { objective_value })
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
