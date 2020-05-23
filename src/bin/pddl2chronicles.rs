use aries::planning::chronicles::*;
use aries::planning::classical::search::plan_search;
use aries::planning::classical::{from_chronicles, grounded_problem};
use aries::planning::parsing::pddl_to_chronicles;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "pddl2chronicles")]
struct Opt {
    domain: String,
    problem: String,
    #[structopt(long)]
    from_plan: bool,
    #[structopt(long)]
    from_actions: Option<usize>,
}

fn main() -> Result<(), String> {
    let opt: Opt = Opt::from_args();
    eprintln!("Options: {:?}", opt);

    let dom = std::fs::read_to_string(&opt.domain).map_err(|o| format!("{}", o))?;

    let prob = std::fs::read_to_string(&opt.problem).map_err(|o| format!("{}", o))?;

    let spec = pddl_to_chronicles(&dom, &prob)?;

    let mut pb = FiniteProblem {
        variables: spec.context.variables.clone(),
        chronicles: spec.chronicles.clone(),
    };

    if let Some(n) = opt.from_actions {
        assert!(
            !opt.from_plan,
            "The from-actions and from-plan options are exclusive"
        );

        // instantiate each template n times
        for (template_id, template) in spec.templates.iter().enumerate() {
            for instantiation_id in 0..n {
                // retrieve of build presence var
                let (prez, presence_param) = match template.chronicle.prez {
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

                let mut vars = Vec::with_capacity(template.params.len());
                for (i, p) in template.params.iter().enumerate() {
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
                let instance = template.instantiate(
                    &vars,
                    template_id as TemplateID,
                    instantiation_id as InstantiationID,
                );
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

        panic!("Not implemented yet")
    } else {
        eprintln!("Error: you should specify an instantiation method")
    }

    let x = serde_json::to_string(&pb).unwrap();
    println!("{}", x);

    Ok(())
}
