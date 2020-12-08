#![allow(unreachable_code, unused_mut, dead_code, unused_variables, unused_imports)] // TODO: remove
#![allow(clippy::all)]

use anyhow::*;

use aries_planning::chronicles::{
    ChronicleTemplate, Condition, DiscreteValue, Domain, Effect, FiniteProblem, Holed, InstantiationID, Problem,
    TemplateID, Time, Type, VarKind, VarMeta,
};

use aries_collections::ref_store::{Ref, RefVec};
use aries_planning::chronicles::constraints::ConstraintType;
use aries_sat::all::Lit;
use aries_sat::SatProblem;

use aries_planning::classical::from_chronicles;
use aries_planning::parsing::pddl_to_chronicles;
use aries_smt::model::assignments::{Assignment, SavedAssignment};
use aries_smt::model::lang::{BAtom, BVar, IAtom, IVar};
use aries_smt::model::Model;
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

    let mut spec = pddl_to_chronicles(&dom, &prob)?;
    if opt.statics_as_table.unwrap_or(true) {
        aries_planning::chronicles::preprocessing::statics_as_tables(&mut spec);
    }

    for n in opt.min_actions..opt.max_actions.unwrap_or(u32::max_value()) {
        println!("{} Solving with {} actions", n, n);
        let start = Instant::now();
        let mut pb = FiniteProblem {
            variables: spec.context.variables.clone(),
            origin: spec.context.origin(),
            horizon: spec.context.horizon(),
            tautology: spec.context.tautology(),
            contradiction: spec.context.contradiction(),
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

fn populate_with_template_instances<F: Fn(&ChronicleTemplate<usize>) -> Option<u32>>(
    pb: &mut FiniteProblem<usize>,
    spec: &Problem<String, String, usize>,
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
                        Type::Time => Domain::temporal(0, DiscreteValue::MAX),
                        Type::Symbolic(tpe) => {
                            let instances = spec.context.symbols.instances_of_type(tpe);
                            Domain::symbolic(instances)
                        }
                        Type::Boolean => Domain::boolean(),
                        Type::Integer => Domain::integer(DiscreteValue::MIN, DiscreteValue::MAX),
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

fn solve(pb: &FiniteProblem<usize>) -> Option<SavedAssignment> {
    let (model, constraints, cor) = encode(&pb).unwrap();

    let mut solver = aries_smt::solver::SMTSolver::new(model);
    solver.add_theory(Box::new(DiffLogicTheory::new()));
    solver.enforce(&constraints);
    if solver.solve() {
        print(pb, &solver.model, &cor);
        solver.print_stats();
        Some(solver.model.to_owned())
    } else {
        None
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Copy, Clone)]
enum Var {
    Boolean(BAtom, IAtom),
    Integer(IAtom),
}

// type SMT = SMTSolver<Edge<i32>, IncSTN<i32>>;

fn effects<X: Ref>(pb: &FiniteProblem<X>) -> impl Iterator<Item = (X, &Effect<X>)> {
    pb.chronicles
        .iter()
        .flat_map(|ch| ch.chronicle.effects.iter().map(move |eff| (ch.chronicle.presence, eff)))
}

fn conditions<X: Ref>(pb: &FiniteProblem<X>) -> impl Iterator<Item = (X, &Condition<X>)> {
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
fn encode(pb: &FiniteProblem<usize>) -> anyhow::Result<(Model, Vec<BAtom>, RefVec<usize, Var>)> {
    let mut model = Model::default();

    let mut cor = RefVec::new();
    let mut cor_back = HashMap::new();

    // the set of constraints that should be enforced
    let mut constraints: Vec<BAtom> = Vec::new();

    for (v, meta) in pb.variables.entries() {
        match meta.domain.kind {
            VarKind::Boolean => {
                let bool_var = if meta.domain.min == meta.domain.max {
                    if meta.domain.min == 0 {
                        // false
                        Var::Boolean(false.into(), 0.into())
                    } else {
                        assert_eq!(meta.domain.min, 1);
                        Var::Boolean(true.into(), 1.into())
                    }
                } else {
                    // non constant boolean var, create a boolean and integer version of it
                    let tp = model.new_ivar(0, 1, &meta.label);
                    Var::Boolean(model.geq(tp, 1), tp.into())
                };
                cor.set_next(v, bool_var);
                cor_back.insert(bool_var, v);
            }
            _ => {
                // integer variable.
                let ivar = if meta.domain.min == meta.domain.max {
                    // this is constant
                    meta.domain.min.into()
                } else {
                    model.new_ivar(meta.domain.min, meta.domain.max, &meta.label).into()
                };
                cor.set_next(v, Var::Integer(ivar));
                cor_back.insert(Var::Integer(ivar), v);
            }
        }
    }

    let bool = |x| match cor[x] {
        Var::Boolean(y, _) => y,
        Var::Integer(_) => panic!(),
    };
    let int = |x| match cor[x] {
        Var::Boolean(_, i) => i,
        Var::Integer(i) => i,
    };
    // converts a Time construct from chronicles into an IAtom
    let ii = |x: Time<_>| int(x.time_var) + x.delay;

    let effs: Vec<_> = effects(&pb).collect();
    let conds: Vec<_> = conditions(&pb).collect();
    let eff_ends: Vec<_> = effs.iter().map(|_| model.new_ivar(ORIGIN, HORIZON, "")).collect();

    // for each condition, make sure the end is after the start
    for &(prez_cond, cond) in &conds {
        constraints.push(model.leq(ii(cond.start), ii(cond.end)));
    }

    // for each effect, make sure the three time points are ordered
    for ieff in 0..effs.len() {
        let (prez_eff, eff) = effs[ieff];
        constraints.push(model.leq(ii(eff.persistence_start), eff_ends[ieff]));
        constraints.push(model.leq(ii(eff.transition_start), ii(eff.persistence_start)))
    }

    // are two variables unifiable?
    let unifiable_vars = |a, b| {
        let dom_a = pb.variables[a].domain;
        let dom_b = pb.variables[b].domain;
        dom_a.intersects(&dom_b)
    };

    // are two state variables unifiable?
    let unifiable_sv = |sv1: &[usize], sv2: &[usize]| {
        if sv1.len() != sv2.len() {
            false
        } else {
            for (&a, &b) in sv1.iter().zip(sv2) {
                if !unifiable_vars(a, b) {
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
            clause.push(!bool(p1));
            clause.push(!bool(p2));
            assert_eq!(e1.state_var.len(), e2.state_var.len());
            for idx in 0..e1.state_var.len() {
                let a = int(e1.state_var[idx]);
                let b = int(e2.state_var[idx]);
                // enforce different : a < b || a > b
                // if they are the same variable, there is nothing we can do to separate them
                if a != b {
                    clause.push(model.neq(a, b));
                }
            }

            clause.push(model.leq(eff_ends[j], ii(e1.transition_start)));
            clause.push(model.leq(eff_ends[i], ii(e2.transition_start)));

            // add coherence constraint
            constraints.push(model.or(&clause));
        }
    }

    // support constraints
    for (prez_cond, cond) in conds {
        let mut supported = Vec::with_capacity(128);
        // no need to support if the condition is not present
        supported.push(!bool(prez_cond));

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
            supported_by_eff_conjunction.push(bool(prez_eff));

            assert_eq!(cond.state_var.len(), eff.state_var.len());
            // same state variable
            for idx in 0..cond.state_var.len() {
                let a = int(cond.state_var[idx]);
                let b = int(eff.state_var[idx]);

                supported_by_eff_conjunction.push(model.eq(a, b));
            }
            // same value
            let condition_value = int(cond.value);
            let effect_value = int(eff.value);

            supported_by_eff_conjunction.push(model.eq(condition_value, effect_value));

            // effect's persistence contains condition
            supported_by_eff_conjunction.push(model.leq(ii(eff.persistence_start), ii(cond.start)));
            supported_by_eff_conjunction.push(model.leq(ii(cond.end), eff_ends[eff_id]));

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
                    supported_by_a_line.push(!bool(instance.chronicle.presence));
                    let vars = &constraint.variables;
                    for values in pb.tables[table_id as usize].lines() {
                        assert_eq!(vars.len(), values.len());
                        let mut supported_by_this_line = Vec::with_capacity(16);
                        for (&var, &val) in vars.iter().zip(values.iter()) {
                            supported_by_this_line.push(model.eq(int(var), val));
                        }
                        supported_by_a_line.push(model.and(&supported_by_this_line));
                    }
                    constraints.push(model.or(&supported_by_a_line));
                }
            }
        }
    }

    Ok((model, constraints, cor))
}

fn print(problem: &FiniteProblem<usize>, ass: &impl Assignment, cor: &RefVec<usize, Var>) {
    let domain = |v: Var| match v {
        Var::Boolean(_, i) => ass.domain_of(i),
        Var::Integer(i) => ass.domain_of(i),
    };
    let fmt_time = |t: Time<usize>| {
        let (lb, ub) = domain(cor[t.time_var]);
        if lb <= ub {
            format!("{}", lb + t.delay)
        } else {
            "NONE".to_string()
        }
    };
    let fmt_var = |v: usize| {
        let (lb, ub) = domain(cor[v]);
        if lb == ub {
            format!("{}", lb)
        } else if lb < ub {
            format!("[{}, {}]", lb, ub)
        } else {
            "NONE".to_string()
        }
    };

    for (instance_id, instance) in problem.chronicles.iter().enumerate() {
        println!(
            "INSTANCE {}: present: {}",
            instance_id,
            fmt_var(instance.chronicle.presence)
        );
        println!("  EFFECTS:");
        for effect in &instance.chronicle.effects {
            print!(
                "    ]{}, {}] ",
                fmt_time(effect.transition_start),
                fmt_time(effect.persistence_start)
            );
            for &x in &effect.state_var {
                print!("{} ", fmt_var(x))
            }
            println!(":= {}", fmt_var(effect.value))
        }
        println!("  CONDITIONS: ");
        for conditions in &instance.chronicle.conditions {
            print!("    [{}, {}] ", fmt_time(conditions.start), fmt_time(conditions.end));
            for &x in &conditions.state_var {
                print!("{} ", fmt_var(x))
            }
            println!("= {}", fmt_var(conditions.value))
        }
    }
}
