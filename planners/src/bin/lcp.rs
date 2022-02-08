use anyhow::{Context, Result};
use structopt::StructOpt;

use aries_planning::parsing::pddl::{find_domain_of, parse_pddl_domain, parse_pddl_problem, PddlFeature};
use aries_planning::parsing::pddl_to_chronicles;
use aries_utils::input::Input;

use aries_planners::solver::Opt;
use aries_planners::Planner;

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();

    let problem_file = &opt.problem;
    anyhow::ensure!(
        problem_file.exists(),
        "Problem file {} does not exist",
        problem_file.display()
    );

    let problem_file = problem_file.canonicalize().unwrap();
    let domain_file = match opt.domain {
        Some(ref name) => name.clone(),
        None => find_domain_of(&problem_file).context("Consider specifying the domain with the option -d/--domain")?,
    };

    let dom = Input::from_file(&domain_file)?;
    let prob = Input::from_file(&problem_file)?;
    let mut planner = Planner::new(opt.clone());

    let dom = parse_pddl_domain(dom)?;
    let prob = parse_pddl_problem(prob)?;

    // true if we are doing HTN planning, false otherwise
    planner.htn_mode = dom.features.contains(&PddlFeature::Hierarchy);

    let spec = pddl_to_chronicles(&dom, &prob)?;

    planner.solve(spec, &opt)?;
    if planner.plan.is_some() {
        println!("\nPlan found!");
    } else {
        println!("\nNo plan found");
    }
    planner.format_plan(&planner.plan)?;

    Ok(())
}
