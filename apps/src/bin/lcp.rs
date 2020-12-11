#![allow(unreachable_code, unused_mut, dead_code, unused_variables, unused_imports)] // TODO: remove
#![allow(clippy::all)]

use anyhow::*;

use aries_planning::chronicles::*;

use aries_collections::ref_store::{Ref, RefVec};
use aries_planning::chronicles::constraints::ConstraintType;
use aries_sat::all::Lit;
use aries_sat::SatProblem;

use aries_model::assignments::{Assignment, SavedAssignment};
use aries_model::lang::{Atom, BAtom, BVar, IAtom, IVar, Variable};
use aries_model::Model;
use aries_planning::classical::from_chronicles;
use aries_planning::parsing::pddl_to_chronicles;
use aries_smt::*;
use aries_tnet::stn::{DiffLogicTheory, Edge, IncSTN, Timepoint};
use aries_tnet::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use structopt::StructOpt;

/// Generates chronicles from a PDDL problem specification.
#[derive(Debug, StructOpt)]
#[structopt(name = "pddl2chronicles", rename_all = "kebab-case")]
struct Opt {
    #[structopt(long, short)]
    domain: Option<String>,
    problem: String,
    #[structopt(long, default_value = "0")]
    min_actions: u32,
    #[structopt(long)]
    max_actions: Option<u32>,
    #[structopt(long = "tables")]
    statics_as_table: Option<bool>,
}

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

    let mut spec = pddl_to_chronicles(&dom, &prob)?;
    if opt.statics_as_table.unwrap_or(true) {
        aries_planning::chronicles::preprocessing::statics_as_tables(&mut spec);
    }

    for n in opt.min_actions..opt.max_actions.unwrap_or(u32::max_value()) {
        println!("{} Solving with {} actions", n, n);
        let start = Instant::now();
        let mut pb = FiniteProblem {
            model: spec.context.model.clone(),
            origin: spec.context.origin(),
            horizon: spec.context.horizon(),
            chronicles: spec.chronicles.clone(),
            tables: spec.context.tables.clone(),
        };
        populate_with_template_instances(&mut pb, &spec, |_| Some(n))?;
        println!("  [{:.3}s] Populated", start.elapsed().as_secs_f32());
        let start = Instant::now();
        let result = solve(&pb);
        println!("  [{:.3}s] solved", start.elapsed().as_secs_f32());
        match result {
            Some(x) => {
                println!("  Solution found");
                break;
            }
            None => (),
        }
    }

    Ok(())
}

fn populate_with_template_instances<F: Fn(&ChronicleTemplate) -> Option<u32>>(
    pb: &mut FiniteProblem,
    spec: &Problem,
    num_instances: F,
) -> Result<()> {
    // instantiate each template n times
    for (template_id, template) in spec.templates.iter().enumerate() {
        let n = num_instances(template).context("Could not determine a number of occurrences for a template")?;
        for instantiation_id in 0..n {
            let fresh_params: Vec<Variable> = Vec::new();
            for v in &template.parameters {
                todo!()
            }
            // // retrieve or build presence var
            // let (prez, presence_param) = match template.chronicle.presence {
            //     Holed::Full(p) => (p, None),
            //     Holed::Param(i) => {
            //         let meta = VarMeta::new(
            //             Domain::boolean(),
            //             None,
            //             Some(format!("{}_{}_?present", template_id, instantiation_id)),
            //         );
            //
            //         (pb.variables.push(meta), Some(i))
            //     }
            // };
            //
            // // create all parameters of the chronicles
            // let mut vars = Vec::with_capacity(template.parameters.len());
            // for (i, p) in template.parameters.iter().enumerate() {
            //     if presence_param == Some(i) {
            //         // we are treating the presence parameter
            //         vars.push(prez);
            //     } else {
            //         let dom = match p.0 {
            //             Type::Time => Domain::temporal(0, DiscreteValue::MAX),
            //             Type::Symbolic(tpe) => {
            //                 let instances = spec.context.symbols.instances_of_type(tpe);
            //                 Domain::symbolic(instances)
            //             }
            //             Type::Boolean => Domain::boolean(),
            //             Type::Integer => Domain::integer(DiscreteValue::MIN, DiscreteValue::MAX),
            //         };
            //         let label =
            //             p.1.as_ref()
            //                 .map(|s| format!("{}_{}_{}", template_id, instantiation_id, &s));
            //         let meta = VarMeta::new(dom, Some(prez), label);
            //         let var = pb.variables.push(meta);
            //         vars.push(var);
            //     }
            // }
            let instance = template.instantiate(
                fresh_params,
                template_id as TemplateID,
                instantiation_id as InstantiationID,
            );
            pb.chronicles.push(instance?);
        }
    }
    Ok(())
}

fn solve(pb: &FiniteProblem) -> Option<SavedAssignment> {
    let (model, constraints) = encode(&pb).unwrap();

    let mut solver = aries_smt::solver::SMTSolver::new(model);
    solver.add_theory(Box::new(DiffLogicTheory::new()));
    solver.enforce(&constraints);
    if solver.solve() {
        print(pb, &solver.model);
        solver.print_stats();
        Some(Assignment::to_owned(&solver.model))
    } else {
        None
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Copy, Clone)]
enum Var {
    Boolean(BAtom, IAtom),
    Integer(IAtom),
}

fn effects(pb: &FiniteProblem) -> impl Iterator<Item = (BAtom, &Effect)> {
    pb.chronicles
        .iter()
        .flat_map(|ch| ch.chronicle.effects.iter().map(move |eff| (ch.chronicle.presence, eff)))
}

fn conditions(pb: &FiniteProblem) -> impl Iterator<Item = (BAtom, &Condition)> {
    pb.chronicles.iter().flat_map(|ch| {
        ch.chronicle
            .conditions
            .iter()
            .map(move |cond| (ch.chronicle.presence, cond))
    })
}

const ORIGIN: i32 = 0;
const HORIZON: i32 = 999999;
//
fn encode(pb: &FiniteProblem) -> anyhow::Result<(Model, Vec<BAtom>)> {
    let mut model = pb.model.clone();

    // the set of constraints that should be enforced
    let mut constraints: Vec<BAtom> = Vec::new();

    let effs: Vec<_> = effects(&pb).collect();
    let conds: Vec<_> = conditions(&pb).collect();
    let eff_ends: Vec<_> = effs.iter().map(|_| model.new_ivar(ORIGIN, HORIZON, "")).collect();

    // for each condition, make sure the end is after the start
    for &(prez_cond, cond) in &conds {
        constraints.push(model.leq(cond.start, cond.end));
    }

    // for each effect, make sure the three time points are ordered
    for ieff in 0..effs.len() {
        let (prez_eff, eff) = effs[ieff];
        constraints.push(model.leq(eff.persistence_start, eff_ends[ieff]));
        constraints.push(model.leq(eff.transition_start, eff.persistence_start))
    }

    // are two variables unifiable?
    let unifiable_vars = |a: Atom, b: Atom| {
        todo!();
        true
        // let dom_a = pb.variables[a].domain;
        // let dom_b = pb.variables[b].domain;
        // dom_a.intersects(&dom_b)
    };

    // are two state variables unifiable?
    let unifiable_sv = |sv1: &SV, sv2: &SV| {
        if sv1.len() != sv2.len() {
            false
        } else {
            for (&a, &b) in sv1.iter().zip(sv2) {
                if !unifiable_vars(a.into(), b.into()) {
                    return false;
                }
            }
            true
        }
    };

    // for each pair of effects, enforce coherence constraints
    let mut clause = Vec::with_capacity(32);
    for (i, &(p1, e1)) in effs.iter().enumerate() {
        for j in i + 1..effs.len() {
            let &(p2, e2) = &effs[j];

            // skip if they are trivially non-overlapping
            if !unifiable_sv(&e1.state_var, &e2.state_var) {
                continue;
            }

            clause.clear();
            clause.push(!p1);
            clause.push(!p2);
            assert_eq!(e1.state_var.len(), e2.state_var.len());
            for idx in 0..e1.state_var.len() {
                let a = e1.state_var[idx];
                let b = e2.state_var[idx];
                // enforce different : a < b || a > b
                // if they are the same variable, there is nothing we can do to separate them
                if a != b {
                    clause.push(model.neq(a, b));
                }
            }

            clause.push(model.leq(eff_ends[j], e1.transition_start));
            clause.push(model.leq(eff_ends[i], e2.transition_start));

            // add coherence constraint
            constraints.push(model.or(&clause));
        }
    }

    // support constraints
    for (prez_cond, cond) in conds {
        let mut supported = Vec::with_capacity(128);
        // no need to support if the condition is not present
        supported.push(!prez_cond);

        for (eff_id, &(prez_eff, eff)) in effs.iter().enumerate() {
            // quick check that the condition and effect are not trivially incompatible
            if !unifiable_sv(&cond.state_var, &eff.state_var) {
                continue;
            }
            if !unifiable_vars(cond.value, eff.value) {
                continue;
            }
            // vector to store the AND clause
            let mut supported_by_eff_conjunction = Vec::with_capacity(32);
            // support only possible if the effect is present
            supported_by_eff_conjunction.push(prez_eff);

            assert_eq!(cond.state_var.len(), eff.state_var.len());
            // same state variable
            for idx in 0..cond.state_var.len() {
                let a = cond.state_var[idx];
                let b = eff.state_var[idx];

                supported_by_eff_conjunction.push(model.eq(a, b));
            }
            // same value
            let condition_value = cond.value;
            let effect_value = eff.value;
            assert_eq!(
                condition_value, effect_value,
                "the next line is removed, and need to be reactivated before adding this check"
            );
            // supported_by_eff_conjunction.push(model.eq(condition_value, effect_value));

            // effect's persistence contains condition
            supported_by_eff_conjunction.push(model.leq(eff.persistence_start, cond.start));
            supported_by_eff_conjunction.push(model.leq(cond.end, eff_ends[eff_id]));

            // add this support expression to the support clause
            supported.push(model.and(&supported_by_eff_conjunction));
        }

        // enforce necessary conditions for condition' support
        constraints.push(model.or(&supported));
    }

    // chronicle constraints
    for instance in &pb.chronicles {
        for constraint in &instance.chronicle.constraints {
            match constraint.tpe {
                ConstraintType::InTable { table_id } => {
                    let mut supported_by_a_line = Vec::with_capacity(256);
                    supported_by_a_line.push(!instance.chronicle.presence);
                    let vars = &constraint.variables;
                    for values in pb.tables[table_id as usize].lines() {
                        assert_eq!(vars.len(), values.len());
                        let mut supported_by_this_line = Vec::with_capacity(16);
                        for (&var, &val) in vars.iter().zip(values.iter()) {
                            supported_by_this_line.push(model.eq(var, val));
                        }
                        supported_by_a_line.push(model.and(&supported_by_this_line));
                    }
                    constraints.push(model.or(&supported_by_a_line));
                }
            }
        }
    }

    Ok((model, constraints))
}

fn print(problem: &FiniteProblem, ass: &impl Assignment) {
    // let domain = |v: Var| match v {
    //     Var::Boolean(_, i) => ass.domain_of(i),
    //     Var::Integer(i) => ass.domain_of(i),
    // };
    // let fmt_time = |t: Time<usize>| {
    //     let (lb, ub) = domain(cor[t.time_var]);
    //     if lb <= ub {
    //         format!("{}", lb + t.delay)
    //     } else {
    //         "NONE".to_string()
    //     }
    // };
    // let fmt_var = |v: usize| {
    //     let (lb, ub) = domain(cor[v]);
    //     if lb == ub {
    //         format!("{}", lb)
    //     } else if lb < ub {
    //         format!("[{}, {}]", lb, ub)
    //     } else {
    //         "NONE".to_string()
    //     }
    // };
    //
    // for (instance_id, instance) in problem.chronicles.iter().enumerate() {
    //     println!(
    //         "INSTANCE {}: present: {}",
    //         instance_id,
    //         fmt_var(instance.chronicle.presence)
    //     );
    //     println!("  EFFECTS:");
    //     for effect in &instance.chronicle.effects {
    //         print!(
    //             "    ]{}, {}] ",
    //             fmt_time(effect.transition_start),
    //             fmt_time(effect.persistence_start)
    //         );
    //         for &x in &effect.state_var {
    //             print!("{} ", fmt_var(x))
    //         }
    //         println!(":= {}", fmt_var(effect.value))
    //     }
    //     println!("  CONDITIONS: ");
    //     for conditions in &instance.chronicle.conditions {
    //         print!("    [{}, {}] ", fmt_time(conditions.start), fmt_time(conditions.end));
    //         for &x in &conditions.state_var {
    //             print!("{} ", fmt_var(x))
    //         }
    //         println!("= {}", fmt_var(conditions.value))
    //     }
    // }
}
