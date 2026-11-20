//! The `analyze` command: runs the analysis passes on a problem and reports their results.

use std::{collections::HashMap, io::IsTerminal, path::PathBuf};

use itertools::Itertools;
use planx::{
    Message, Res,
    errors::*,
    pddl::{self, input::Input},
    preprocessing::analysis::{CountConstraints, Dtg, PlanScope},
};

use crate::preprocessing::{ANALYSIS_TRANSFORMATIONS, DEFAULT_MODEL_TRANSFORMATIONS, preprocess_model};

#[derive(clap::Parser, Debug)]
pub struct Analyze {
    /// Path to the problem file
    problem_file: PathBuf,
    /// Path to the PDDL domain file.
    /// If not specified, we will attempt to automatically infer it based on the problem file.
    #[arg(short, long)]
    domain: Option<PathBuf>,
    /// Do not print the annotated model.
    #[arg(short, long)]
    quiet: bool,
    /// Plans (of the problem) against which to check the constraints. Each violated constraint is reported on a line
    /// starting with `VIOLATION`.
    #[arg(short, long)]
    plan: Vec<PathBuf>,
}

pub fn analyze(command: &Analyze) -> Res<()> {
    let problem_file = &command.problem_file;
    if !problem_file.exists() {
        return Err(Message::error(format!(
            "Problem file {} does not exist",
            problem_file.display()
        )));
    }
    let problem_file = problem_file.canonicalize().unwrap();
    let domain_file = match command.domain.as_ref() {
        Some(name) => name.clone(),
        None => pddl::find_domain_of(&problem_file).title(
            "Unable to automatically find the domain file. Consider specifying the domain with the option -d/--domain",
        )?,
    };
    let domain = pddl::parse_pddl_domain(Input::from_file(&domain_file)?)?;
    let problem = pddl::parse_pddl_problem(Input::from_file(&problem_file)?)?;
    let raw_model = pddl::build_model(&domain, &problem)?;
    let model = preprocess_model(raw_model.clone(), DEFAULT_MODEL_TRANSFORMATIONS)?;
    let model = preprocess_model(model, ANALYSIS_TRANSFORMATIONS)?;

    if !command.quiet {
        planx::set_colored_annotations(std::io::stdout().is_terminal());
        println!("===== Annotated model =====\n\n{model}");
    }

    println!("===== Domain transition graphs =====\n");
    let dtgs = Dtg::all(&model, 2000);
    let (with, without): (Vec<_>, Vec<_>) = dtgs.iter().partition(|d| !d.transitions.is_empty());
    for dtg in with {
        println!("{}", model.env.node(dtg));
    }
    if !without.is_empty() {
        let names = without
            .iter()
            .map(|d| model.env.node(d.fluent).name().to_string())
            .join(", ");
        println!("(fluents without transitions, only changed by writers: {names})");
    }
    println!();

    let constraints = CountConstraints::new(&model);
    println!("===== Constraints on the number of instances =====\n");
    println!("[scope] generator  constraint  -- explanation");
    println!(
        "  scopes: all = any valid plan, NR = non-redundant plans, NR+ = NR plans without mutually cancelling actions\n"
    );
    print!("{constraints}");

    println!("\n===== Bounds (simple propagation, for information) =====\n");
    let scopes = [
        PlanScope::AllPlans,
        PlanScope::NonRedundant,
        PlanScope::NonRedundantPlus,
    ];
    let bounds = scopes.map(|s| constraints.propagate(s));
    let width = (constraints.vars.iter())
        .map(|(_, t)| t.to_string().len())
        .max()
        .unwrap_or(0);
    println!("{:width$}   {:>12} {:>12} {:>12}", "", "all", "NR", "NR+");
    for (_, t) in constraints.vars.iter() {
        let cells = bounds.iter().map(|b| match b {
            None => "infeasible".to_string(),
            Some(b) => {
                let (lb, ub) = b[t];
                format!("[{lb}, {}]", ub.map_or("inf".to_string(), |u| u.to_string()))
            }
        });
        println!(
            "{:width$}   {}",
            t.to_string(),
            cells.map(|c| format!("{c:>12}")).join(" ")
        );
    }

    for plan_file in &command.plan {
        let plan = pddl::parse_plan(Input::from_file(plan_file)?)?;
        let plan = pddl::build_plan(&plan, &raw_model)?;
        let counts: HashMap<_, i64> = (plan.operations.iter())
            .map(|op| op.action_ref.clone())
            .counts()
            .into_iter()
            .map(|(a, n)| (a, n as i64))
            .collect();
        println!("\n===== Plan {} =====\n", plan_file.display());
        for (_, t) in constraints.vars.iter() {
            println!("  n({t}) = {}", counts.get(t).copied().unwrap_or(0));
        }
        for c in constraints.violations(&counts) {
            println!("VIOLATION {}", constraints.vars.display(c));
        }
    }
    Ok(())
}
