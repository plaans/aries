use anyhow::*;
use aries::planning::chronicles::*;
use aries::planning::classical::search::{plan_search, Cfg};
use aries::planning::classical::{from_chronicles, grounded_problem};
use aries::planning::parsing::pddl_to_chronicles;
use structopt::StructOpt;

/// Generates chronicles from a PDDL problem specification.
#[derive(Debug, StructOpt)]
#[structopt(name = "pddl2chronicles", rename_all = "kebab-case")]
struct Opt {
    domain: String,
    problem: String,
    #[structopt(long)]
    from_plan: bool,
    #[structopt(long)]
    from_actions: Option<usize>,
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

    let dom = std::fs::read_to_string(&opt.domain)?;

    let prob = std::fs::read_to_string(&opt.problem)?;

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

        // instantiate each template n times
        for (template_id, template) in spec.templates.iter().enumerate() {
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
                let instance =
                    template.instantiate(&vars, template_id as TemplateID, instantiation_id as InstantiationID);
                pb.chronicles.push(instance);
            }
        }
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
                for &op in &plan {
                    eprintln!("{}", symbols.format(grounded.operators.name(op)));
                }
            }
            None => eprintln!("Infeasible"),
        }

        // TODO: should create on instance for
        bail!("Not implemented yet");
    } else {
        bail!("Error: you should specify an instantiation method: --from-plan or --from-actions");
    }

    let x = serde_json::to_string(&pb).unwrap();
    println!("{}", x);

    Ok(())
}
