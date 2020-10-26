use anyhow::*;
use aries_planning::chronicles::*;
use aries_planning::classical::search::{plan_search, Cfg};
use aries_planning::classical::{from_chronicles, grounded_problem};
use aries_planning::parsing::pddl_to_chronicles;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

/// Generates chronicles from a PDDL problem specification.
#[derive(Debug, StructOpt)]
#[structopt(name = "pddl2chronicles", rename_all = "kebab-case")]
struct Opt {
    #[structopt(long, short)]
    domain: Option<String>,
    problem: String,
    #[structopt(long)]
    from_plan: bool,
    #[structopt(long)]
    from_actions: Option<u32>,
}

/// This tool is intended to transform a planning problem into a set of chronicles
/// instances to be consumed by an external solver.
///
/// Usage:
/// ```
/// # generates a problem by generating three instances of each action
/// pddl2chronicles <domain.pddl> <problem.pddl> --from-actions 3
/// ```
///
/// The program write a json structure to standard output that you for prettyfy and record like so
/// `pddl2chronicles [ARGS] | python -m json.tool > chronicles.json`
///
fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();
    eprintln!("Options: {:?}", opt);

    let problem_file = Path::new(&opt.problem);
    ensure!(
        problem_file.exists(),
        "Problem file {} does not exist",
        problem_file.display()
    );

    let problem_file = problem_file.canonicalize().unwrap();
    let domain_file = match opt.domain {
        Some(name) => PathBuf::from(&name),
        None => {
            let dir = problem_file.parent().unwrap();
            let candidate1 = dir.join("domain.pddl");
            let candidate2 = dir.parent().unwrap().join("domain.pddl");
            if candidate1.exists() {
                candidate1
            } else if candidate2.exists() {
                candidate2
            } else {
                bail!("Could not find find a corresponding 'domain.pddl' file in same or parent directory as the problem file.\
                 Consider adding it explicitly with the -d/--domain option");
            }
        }
    };

    let dom = std::fs::read_to_string(domain_file)?;

    let prob = std::fs::read_to_string(problem_file)?;

    let spec = pddl_to_chronicles(&dom, &prob)?;

    let mut pb = FiniteProblem {
        variables: spec.context.variables.clone(),
        origin: spec.context.origin(),
        horizon: spec.context.horizon(),
        tautology: spec.context.tautology(),
        contradiction: spec.context.contradiction(),
        chronicles: spec.chronicles.clone(),
    };

    if let Some(n) = opt.from_actions {
        assert!(!opt.from_plan, "The from-actions and from-plan options are exclusive");
        populate_with_template_instances(&mut pb, &spec, |_| Some(n))?;
    } else if opt.from_plan {
        eprintln!("Converting to classical planning");
        let lifted = from_chronicles(&spec)?;
        let grounded = grounded_problem(&lifted)?;
        let symbols = &lifted.world.table;

        eprintln!("Looking for a sequential plan...");

        match plan_search(
            &grounded.initial_state,
            &grounded.operators,
            &grounded.goals,
            &Cfg::default(),
        ) {
            Some(plan) => {
                eprintln!("Got plan: {} actions", plan.len());
                eprintln!("=============");
                let mut count = HashMap::new();
                for &op in &plan {
                    let action = grounded.operators.name(op)[0];
                    let action = symbols.symbol(action);
                    count.insert(action, count.get(action).unwrap_or(&0u32) + 1);
                    eprintln!("{}", symbols.format(grounded.operators.name(op)));
                }

                let f = |template: &ChronicleTemplate<Var>| {
                    template.label.as_ref().map(|lbl| count.get(&lbl).copied().unwrap_or(0))
                };
                populate_with_template_instances(&mut pb, &spec, f)?;
            }
            None => bail!("Planning problem has no solution"),
        }
    } else {
        bail!("Error: you should specify an instantiation method: --from-plan or --from-actions");
    }

    let x = serde_json::to_string(&pb).unwrap();
    println!("{}", x);

    Ok(())
}

fn populate_with_template_instances<F: Fn(&ChronicleTemplate<Var>) -> Option<u32>>(
    pb: &mut FiniteProblem<Var>,
    spec: &Problem<String, String, Var>,
    num_instances: F,
) -> Result<()> {
    // instantiate each template n times
    for (template_id, template) in spec.templates.iter().enumerate() {
        let n = num_instances(template).context("Could not determine a number of occurences for a template")?;
        for instantiation_id in 0..n {
            // retrieve or build presence var
            let (prez, presence_param) = match template.chronicle.presence {
                Holed::Full(p) => (p, None),
                Holed::Param(i) => {
                    let meta = VarMeta::new(
                        Domain::boolean(),
                        None,
                        Some(format!("{}_{}_?present", template_id, instantiation_id)),
                    );

                    (pb.variables.push(meta), Some(i))
                }
            };

            // create all parameters of the chronicles
            let mut vars = Vec::with_capacity(template.parameters.len());
            for (i, p) in template.parameters.iter().enumerate() {
                if presence_param == Some(i) {
                    // we are treating the presence parameter
                    vars.push(prez);
                } else {
                    let dom = match p.0 {
                        Type::Time => Domain::temporal(0, Integer::MAX),
                        Type::Symbolic(tpe) => {
                            let instances = spec.context.symbols.instances_of_type(tpe);
                            Domain::symbolic(instances)
                        }
                        Type::Boolean => Domain::boolean(),
                        Type::Integer => Domain::integer(Integer::MIN, Integer::MAX),
                    };
                    let label =
                        p.1.as_ref()
                            .map(|s| format!("{}_{}_{}", template_id, instantiation_id, &s));
                    let meta = VarMeta::new(dom, Some(prez), label);
                    let var = pb.variables.push(meta);
                    vars.push(var);
                }
            }
            let instance = template.instantiate(&vars, template_id as TemplateID, instantiation_id as InstantiationID);
            pb.chronicles.push(instance);
        }
    }
    Ok(())
}
