#![allow(unreachable_code, unused_mut, dead_code, unused_variables, unused_imports)] // TODO: remove
#![allow(clippy::all)]

use anyhow::*;

use aries_planning::chronicles::*;

use aries_collections::ref_store::{Ref, RefVec};
use aries_planning::chronicles::constraints::ConstraintType;

use aries_model::assignments::{Assignment, SavedAssignment};
use aries_model::lang::{Atom, BAtom, BVar, IAtom, IVar, SAtom, Variable};
use aries_model::symbols::SymId;
use aries_model::Model;
use aries_planning::chronicles::Task;
use aries_planning::classical::from_chronicles;
use aries_planning::parsing::pddl::{parse_pddl_domain, parse_pddl_problem, PddlFeature};
use aries_planning::parsing::pddl_to_chronicles;
use aries_smt::*;
use aries_tnet::stn::{Edge, IncSTN, Timepoint};
use aries_tnet::*;
use aries_utils::input::Input;
use aries_utils::Fmt;
use env_param::EnvParam;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;
use structopt::StructOpt;

/// Generates chronicles from a PDDL problem specification.
#[derive(Debug, StructOpt)]
#[structopt(name = "lcp", rename_all = "kebab-case")]
struct Opt {
    #[structopt(long, short)]
    domain: Option<PathBuf>,
    problem: PathBuf,
    #[structopt(long = "output", short = "o")]
    plan_out_file: Option<PathBuf>,
    #[structopt(long, default_value = "0")]
    min_actions: u32,
    #[structopt(long)]
    max_actions: Option<u32>,
    #[structopt(long = "optimize")]
    optimize_makespan: bool,
}

/// Parameter that defines the symmetry breaking strategy to use.
/// The value of this parameter is loaded from the environment variable `ARIES_LCP_SYMMETRY_BREAKING`.
/// Possible values are `none` and `simple` (default).
static SYMMETRY_BREAKING: EnvParam<SymmetryBreakingType> = EnvParam::new("ARIES_LCP_SYMMETRY_BREAKING", "simple");

/// The type of symmetry breaking to apply to problems.
#[derive(Copy, Clone)]
enum SymmetryBreakingType {
    /// no symmetry breaking
    None,
    /// Simple form of symmetry breaking described in the LCP paper (CP 2018).
    /// This enforces that for any two instances of the same template. The first one (in arbitrary total order)
    ///  - is always present if the second instance is present
    ///  - starts before the second instance
    Simple,
}
impl std::str::FromStr for SymmetryBreakingType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(SymmetryBreakingType::None),
            "simple" => Ok(SymmetryBreakingType::Simple),
            x => Err(format!("Unknown symmetry breaking type: {}", s)),
        }
    }
}

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();

    let problem_file = &opt.problem;
    ensure!(
        problem_file.exists(),
        "Problem file {} does not exist",
        problem_file.display()
    );

    let problem_file = problem_file.canonicalize().unwrap();
    let domain_file = match opt.domain {
        Some(name) => name,
        None => aries::find_domain_of(&problem_file)
            .context("Consider specifying the domain with the option -d/--domain")?,
    };

    let dom = Input::from_file(&domain_file)?;
    let prob = Input::from_file(&problem_file)?;

    let dom = parse_pddl_domain(dom)?;
    let prob = parse_pddl_problem(prob)?;

    // true if we are doing HTN planning, false otherwise
    let htn_mode = dom.features.contains(&PddlFeature::Hierarchy);

    let mut spec = pddl_to_chronicles(&dom, &prob)?;

    println!("===== Preprocessing ======");
    aries_planning::chronicles::preprocessing::preprocess(&mut spec);
    println!("==========================");

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
        if htn_mode {
            populate_with_task_network(&mut pb, &spec, n)?;
        } else {
            populate_with_template_instances(&mut pb, &spec, |_| Some(n))?;
        }
        println!("  [{:.3}s] Populated", start.elapsed().as_secs_f32());
        let start = Instant::now();
        let result = solve(&pb, opt.optimize_makespan);
        println!("  [{:.3}s] solved", start.elapsed().as_secs_f32());
        match result {
            Some(x) => {
                println!("  Solution found");
                let plan = if htn_mode {
                    format_hddl_plan(&pb, &x)?
                } else {
                    format_pddl_plan(&pb, &x)?
                };
                println!("{}", plan);
                if let Some(plan_out_file) = opt.plan_out_file {
                    let mut file = File::create(plan_out_file)?;
                    file.write_all(plan.as_bytes())?;
                }
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
        let n = num_instances(template).context("Could not determine a number of occurrences for a template")? as usize;
        for instantiation_id in 0..n {
            let origin = ChronicleOrigin::FreeAction {
                template_id,
                generation_id: instantiation_id,
            };
            let instance = instantiate(template, origin, pb)?;
            pb.chronicles.push(instance);
        }
    }
    Ok(())
}

/// Instantiates a chronicle template into a new chronicle instance.
/// Variables are replaced with new ones, declared to the `pb`.
/// The resulting instance is given the origin passed as parameter.
fn instantiate(
    template: &ChronicleTemplate,
    origin: ChronicleOrigin,
    pb: &mut FiniteProblem,
) -> Result<ChronicleInstance, InvalidSubstitution> {
    let mut fresh_params: Vec<Variable> = Vec::new();
    for v in &template.parameters {
        let label = format!("{}{}", origin.prefix(), pb.model.fmt(*v));
        let fresh: Variable = match v {
            Variable::Bool(b) => pb.model.new_bvar(label).into(),
            Variable::Int(i) => {
                let (lb, ub) = pb.model.domain_of(*i);
                pb.model.new_ivar(lb, ub, label).into()
            }
            Variable::Sym(s) => pb.model.new_sym_var(s.tpe, label).into(),
        };
        fresh_params.push(fresh);
    }

    template.instantiate(fresh_params, origin)
}

fn populate_with_task_network(pb: &mut FiniteProblem, spec: &Problem, max_depth: u32) -> Result<()> {
    struct Subtask {
        task: Task,
        instance_id: usize,
        task_id: usize,
    }
    let mut subtasks = Vec::new();
    for (instance_id, ch) in pb.chronicles.iter().enumerate() {
        for (task_id, task) in ch.chronicle.subtasks.iter().enumerate() {
            let task = &task.task;
            subtasks.push(Subtask {
                task: task.clone(),
                instance_id,
                task_id,
            });
        }
    }
    for depth in 0..max_depth {
        let mut new_subtasks = Vec::new();
        for task in &subtasks {
            for template in refinements_of_task(&task.task, pb, spec) {
                if depth == max_depth - 1 && !template.chronicle.subtasks.is_empty() {
                    // this chronicle has subtasks that cannot be achieved since they would require
                    // an higher decomposition depth
                    continue;
                }
                let origin = ChronicleOrigin::Refinement {
                    instance_id: task.instance_id,
                    task_id: task.task_id,
                };
                let instance = instantiate(template, origin, pb)?;
                let instance_id = pb.chronicles.len();
                pb.chronicles.push(instance);
                for (task_id, subtask) in pb.chronicles[instance_id].chronicle.subtasks.iter().enumerate() {
                    let task = &subtask.task;
                    new_subtasks.push(Subtask {
                        task: task.clone(),
                        instance_id,
                        task_id,
                    });
                }
            }
        }
        subtasks = new_subtasks;
    }
    Ok(())
}

fn refinements_of_task<'a>(task: &Task, pb: &FiniteProblem, spec: &'a Problem) -> Vec<&'a ChronicleTemplate> {
    let mut candidates = Vec::new();
    for template in &spec.templates {
        if let Some(ch_task) = &template.chronicle.task {
            if pb.model.unifiable_seq(task.as_slice(), ch_task.as_slice()) {
                candidates.push(template);
            }
        }
    }
    candidates
}

fn solve(pb: &FiniteProblem, optimize_makespan: bool) -> Option<SavedAssignment> {
    let (model, constraints) = encode(&pb).unwrap(); // TODO: report error

    let mut solver = aries_smt::solver::SMTSolver::new(model);
    solver.add_theory(Box::new(IncSTN::new()));
    solver.enforce_all(&constraints);

    let found_plan = if optimize_makespan {
        let res = solver.minimize_with(pb.horizon, |makespan, ass| {
            println!(
                "\nFound plan with makespan: {}\n{}",
                makespan,
                format_pddl_plan(&pb, ass).unwrap_or_else(|e| format!("Error while formatting:\n{}", e))
            );
        });
        res.map(|tup| tup.1)
    } else {
        if solver.solve() {
            Some(solver.model.clone())
        } else {
            None
        }
    };

    if let Some(solution) = found_plan {
        println!("{}", &solver.stats);
        Some(solution)
    } else {
        None
    }
}

#[derive(Eq, PartialEq, Hash, Copy, Clone)]
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

struct TaskRef<'a> {
    presence: BAtom,
    start: IAtom,
    end: IAtom,
    task: &'a Task,
}

fn add_decomposition_constraints(pb: &FiniteProblem, model: &mut Model, constraints: &mut Vec<BAtom>) {
    for (instance_id, chronicle) in pb.chronicles.iter().enumerate() {
        for (task_id, task) in chronicle.chronicle.subtasks.iter().enumerate() {
            let subtask = TaskRef {
                presence: chronicle.chronicle.presence,
                start: task.start,
                end: task.end,
                task: &task.task,
            };
            let refiners = refinements_of(instance_id, task_id, pb);
            enforce_refinement(subtask, refiners, model, constraints);
        }
    }
}

fn enforce_refinement(t: TaskRef, supporters: Vec<TaskRef>, model: &mut Model, constraints: &mut Vec<BAtom>) {
    // if t is present then at least one supporter is present
    let mut clause = Vec::new();
    clause.push(!t.presence);
    for s in &supporters {
        clause.push(s.presence);
    }
    constraints.push(model.or(&clause));

    // if a supporter is present, then all others are absent
    for (i, s1) in supporters.iter().enumerate() {
        for (j, s2) in supporters.iter().enumerate() {
            if i != j {
                constraints.push(model.implies(s1.presence, !s2.presence));
            }
        }
    }

    // if a supporter is present, then all its parameters are unified with the ones of the supported task
    for s in &supporters {
        // if the supporter is present, the supported is as well
        constraints.push(model.implies(s.presence, t.presence));

        let mut conjunction = Vec::new();
        conjunction.push(model.eq(s.start, t.start));
        conjunction.push(model.eq(s.end, t.end));
        assert_eq!(s.task.len(), t.task.len());
        for (a, b) in s.task.iter().zip(t.task.iter()) {
            conjunction.push(model.eq(*a, *b))
        }
        let identical = model.and(&conjunction);
        constraints.push(model.implies(s.presence, identical));
    }
}

fn refinements_of(instance_id: usize, task_id: usize, pb: &FiniteProblem) -> Vec<TaskRef> {
    let mut supporters = Vec::new();
    let target_origin = ChronicleOrigin::Refinement { instance_id, task_id };
    for ch in pb.chronicles.iter().filter(|ch| ch.origin == target_origin) {
        let task = ch.chronicle.task.as_ref().unwrap();
        supporters.push(TaskRef {
            presence: ch.chronicle.presence,
            start: ch.chronicle.start,
            end: ch.chronicle.end,
            task,
        });
    }
    supporters
}

fn add_symmetry_breaking(
    pb: &FiniteProblem,
    model: &mut Model,
    constraints: &mut Vec<BAtom>,
    tpe: SymmetryBreakingType,
) -> Result<()> {
    match tpe {
        SymmetryBreakingType::None => {}
        SymmetryBreakingType::Simple => {
            let chronicles = || {
                pb.chronicles.iter().filter_map(|c| match c.origin {
                    ChronicleOrigin::FreeAction {
                        template_id,
                        generation_id,
                    } => Some((c, template_id, generation_id)),
                    _ => None,
                })
            };
            for (instance1, template_id1, generation_id1) in chronicles() {
                for (instance2, template_id2, generation_id2) in chronicles() {
                    if template_id1 == template_id2 && generation_id1 < generation_id2 {
                        constraints.push(model.implies(instance1.chronicle.presence, instance2.chronicle.presence));
                        constraints.push(model.leq(instance1.chronicle.start, instance2.chronicle.start))
                    }
                }
            }
        }
    };

    Ok(())
}

fn encode(pb: &FiniteProblem) -> anyhow::Result<(Model, Vec<BAtom>)> {
    let mut model = pb.model.clone();
    let symmetry_breaking_tpe = *SYMMETRY_BREAKING.get();

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

    // are two state variables unifiable?
    let unifiable_sv = |model: &Model, sv1: &SV, sv2: &SV| {
        if sv1.len() != sv2.len() {
            false
        } else {
            for (&a, &b) in sv1.iter().zip(sv2) {
                if !model.unifiable(a, b) {
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
            if !unifiable_sv(&model, &e1.state_var, &e2.state_var) {
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
            if !unifiable_sv(&model, &cond.state_var, &eff.state_var) {
                continue;
            }
            if !model.unifiable(cond.value, eff.value) {
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
            supported_by_eff_conjunction.push(model.eq(condition_value, effect_value));

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
                ConstraintType::LT => match constraint.variables.as_slice() {
                    &[a, b] => {
                        let a: IAtom = a.try_into()?;
                        let b: IAtom = b.try_into()?;
                        constraints.push(model.lt(a, b))
                    }
                    x => bail!("Invalid variable pattern for LT constraint: {:?}", x),
                },
                ConstraintType::EQ => {
                    if constraint.variables.len() != 2 {
                        bail!(
                            "Wrong number of parameters to equality constraint: {}",
                            constraint.variables.len()
                        );
                    }
                    constraints.push(model.eq(constraint.variables[0], constraint.variables[1]));
                }
                ConstraintType::NEQ => {
                    if constraint.variables.len() != 2 {
                        bail!(
                            "Wrong number of parameters to inequality constraint: {}",
                            constraint.variables.len()
                        );
                    }
                    constraints.push(model.neq(constraint.variables[0], constraint.variables[1]));
                }
            }
        }
    }

    for ch in &pb.chronicles {
        // make sure the chronicle finishes before the horizon
        let end_before_horizon = model.leq(ch.chronicle.end, pb.horizon);
        constraints.push(model.implies(ch.chronicle.presence, end_before_horizon));

        // enforce temporal coherence between the chronicle and its subtasks
        constraints.push(model.leq(ch.chronicle.start, ch.chronicle.end));
        for subtask in &ch.chronicle.subtasks {
            let mut conj = Vec::new();
            conj.push(model.leq(subtask.start, subtask.end));
            conj.push(model.leq(ch.chronicle.start, subtask.start));
            conj.push(model.leq(subtask.end, ch.chronicle.end));
            let conj = model.and(&conj);
            // constraints.push(conj);
            constraints.push(model.implies(ch.chronicle.presence, conj));
        }
    }
    add_decomposition_constraints(pb, &mut model, &mut constraints);
    add_symmetry_breaking(pb, &mut model, &mut constraints, symmetry_breaking_tpe)?;

    Ok((model, constraints))
}

fn format_pddl_plan(problem: &FiniteProblem, ass: &impl Assignment) -> Result<String> {
    let mut out = String::new();
    let mut plan = Vec::new();
    for ch in &problem.chronicles {
        if ass.boolean_value_of(ch.chronicle.presence) != Some(true) {
            continue;
        }
        if ch.origin == ChronicleOrigin::Original {
            continue;
        }
        let start = ass.domain_of(ch.chronicle.start).0;
        let name: Vec<SymId> = ch
            .chronicle
            .name
            .iter()
            .map(|satom| ass.sym_domain_of(*satom).into_singleton().unwrap())
            .collect();
        let name = ass.symbols().format(&name);
        plan.push((start, name));
    }

    plan.sort();
    for (start, name) in plan {
        writeln!(out, "{:>3}: {}", start, name)?;
    }
    Ok(out)
}

/// Formats a hierarchical plan into the format expected by pandaPIparser's verifier
fn format_hddl_plan(problem: &FiniteProblem, ass: &impl Assignment) -> Result<String> {
    let mut f = String::new();
    writeln!(f, "==>")?;
    let fmt1 = |x: &SAtom| -> String {
        let sym = ass.sym_domain_of(*x).into_singleton().unwrap();
        ass.symbols().symbol(sym).to_string()
    };
    let fmt = |name: &[SAtom]| -> String {
        let syms: Vec<_> = name
            .iter()
            .map(|x| ass.sym_domain_of(*x).into_singleton().unwrap())
            .collect();
        ass.symbols().format(&syms)
    };
    let mut chronicles: Vec<_> = problem
        .chronicles
        .iter()
        .enumerate()
        .filter(|ch| ass.boolean_value_of(ch.1.chronicle.presence) == Some(true))
        .collect();
    // sort by start times
    chronicles.sort_by_key(|ch| ass.domain_of(ch.1.chronicle.start).0);

    for &(i, ch) in &chronicles {
        if ch.chronicle.kind == ChronicleKind::Action {
            writeln!(f, "{} {}", i, fmt(&ch.chronicle.name))?;
        }
    }
    let print_subtasks_ids = |out: &mut String, chronicle_id: usize| -> Result<()> {
        for &(i, ch) in &chronicles {
            match ch.origin {
                ChronicleOrigin::Refinement { instance_id, .. } if instance_id == chronicle_id => {
                    write!(out, " {}", i)?;
                }
                _ => (),
            }
        }
        Ok(())
    };
    for &(i, ch) in &chronicles {
        if ch.chronicle.kind == ChronicleKind::Action {
            continue;
        }
        if ch.chronicle.kind == ChronicleKind::Problem {
            write!(f, "root")?;
        } else if ch.chronicle.kind == ChronicleKind::Method {
            write!(
                f,
                "{} {} -> {}",
                i,
                fmt(&ch.chronicle.task.as_ref().unwrap()),
                fmt1(&ch.chronicle.name[0])
            )?;
        }
        print_subtasks_ids(&mut f, i)?;
        writeln!(f)?;
    }
    writeln!(f, "<==")?;
    Ok(f)
}
