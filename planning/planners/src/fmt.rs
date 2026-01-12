//! Functions related to printing and formatting (partial) plans.

use anyhow::*;
use aries::core::state::Domains;
use aries::model::symbols::ContiguousSymbols;
use itertools::Itertools;
use std::fmt::Write;

use crate::Model;
use aries::model::extensions::{DomainsExt, Shaped};
use aries::model::lang::{Atom, Cst, Rational};
use aries_planning::chronicles::plan::ActionInstance;
use aries_planning::chronicles::{
    ChronicleInstance, ChronicleKind, ChronicleOrigin, FiniteProblem, SubTask, TaskId, TIME_SCALE,
};

pub fn format_partial_atom<A: Into<Atom>>(x: A, ass: &Model, out: &mut String) {
    let x: Atom = x.into();
    let prefix = match ass.present(x) {
        Some(false) => {
            write!(out, "_").unwrap();
            return;
        }
        None => "?",
        Some(true) => "",
    };
    match x {
        Atom::Sym(x) => {
            let (lb, ub) = ass.bounds(x);
            let dom = ContiguousSymbols::new(lb, ub);
            let singleton = dom.size() == 1;
            match dom.into_singleton() {
                Some(sym) => {
                    write!(out, "{}", ass.shape.symbols.symbol(sym)).unwrap();
                }
                None => {
                    write!(out, "{prefix}{{").unwrap();
                    for (i, sym) in dom.enumerate() {
                        write!(out, "{}", ass.get_symbol(sym)).unwrap();
                        if !singleton && (i as u32) != (dom.size() - 1) {
                            write!(out, ", ").unwrap();
                        }
                    }
                    if !singleton {
                        write!(out, "}}").unwrap();
                    }
                }
            }
        }
        Atom::Bool(l) => {
            write!(
                out,
                "{}",
                match ass.value_of(l) {
                    Some(true) => "true",
                    Some(false) => "false",
                    None => "{true, false}",
                }
            )
            .unwrap();
        }
        Atom::Int(i) => {
            write!(out, "{}", ass.var_domain(i)).unwrap();
        }
        Atom::Fixed(f) => {
            write!(out, "{}", ass.f_domain(f)).unwrap();
        }
    }
}

pub fn format_atoms(variables: &[Atom], ass: &Model) -> Result<String> {
    let mut str = "(".to_string();
    for (i, atom) in variables.iter().enumerate() {
        write!(str, "{}", ass.fmt(*atom))?;
        if i != (variables.len() - 1) {
            str.push(' ')
        }
    }
    str.push(')');
    Ok(str)
}

pub fn format_partial_name(name: &[impl Into<Atom> + Copy], ass: &Model) -> Result<String> {
    let mut res = String::new();
    write!(res, "(")?;
    for (i, sym) in name.iter().enumerate() {
        let sym: Atom = (*sym).into();
        format_partial_atom(sym, ass, &mut res);
        if i != (name.len() - 1) {
            write!(res, " ")?;
        }
    }
    write!(res, ")")?;
    Ok(res)
}

pub fn format_atom(atom: &Atom, model: &Model, ass: &Domains) -> String {
    match atom {
        Atom::Sym(s) => {
            let sym = ass.var_domain(*s).as_singleton().unwrap();
            model.shape.symbols.symbol(sym).to_string()
        }
        Atom::Bool(l) => ass.value_of(*l).unwrap().to_string(),
        Atom::Int(i) => ass.var_domain(*i).as_singleton().unwrap().to_string(),
        Atom::Fixed(f) => ass.f_domain(*f).to_string(),
    }
}

pub fn format_cst(cst: Cst, model: &Model) -> String {
    match cst {
        Cst::Sym(s) => model.shape.symbols.symbol(s.sym).to_string(),
        Cst::Bool(l) => l.to_string(),
        Cst::Int(i) => i.to_string(),
        Cst::Fixed(f) => str(f),
    }
}

pub fn format_name(variables: &[Atom], model: &Model, ass: &Domains) -> Result<String> {
    let mut res = String::new();
    write!(res, "(")?;
    for (i, atom) in variables.iter().enumerate() {
        write!(res, "{}", format_atom(atom, model, ass))?;
        if i != (variables.len() - 1) {
            write!(res, " ")?;
        }
    }
    write!(res, ")")?;
    Ok(res)
}

type Chronicle<'a> = (usize, &'a ChronicleInstance);

fn format_chronicle_partial(
    (ch_id, ch): Chronicle,
    chronicles: &[Chronicle],
    ass: &Model,
    depth: usize,
    out: &mut String,
) -> Result<()> {
    write!(out, "{}", "  ".repeat(depth))?;
    write!(
        out,
        "{} ",
        match ass.boolean_value_of(ch.chronicle.presence) {
            None => "?",
            Some(true) => "+",
            Some(false) => "-",
        }
    )?;
    write!(out, "{} ", ass.int_bounds(ch.chronicle.start).0)?;
    writeln!(out, " {}", format_partial_name(&ch.chronicle.name, ass)?)?;
    // writeln!(out, "         {}", format_atoms(&ch.chronicle.name, ass)?)?;
    if ass.boolean_value_of(ch.chronicle.presence) != Some(false) {
        for (task_id, task) in ch.chronicle.subtasks.iter().enumerate() {
            let subtask_id = TaskId {
                instance_id: ch_id,
                task_id,
            };
            format_task_partial(subtask_id, task, chronicles, ass, depth + 2, out)?;
        }
    }
    Ok(())
}
fn format_task_partial(
    task_id: TaskId,
    task: &SubTask,
    chronicles: &[Chronicle],
    ass: &Model,
    depth: usize,
    out: &mut String,
) -> Result<()> {
    write!(out, "{}", "  ".repeat(depth))?;
    let start = ass.int_bounds(task.start).0;
    write!(out, "{} {}", start, format_partial_name(&task.task_name, ass)?)?;
    writeln!(out, "         {}", format_atoms(&task.task_name, ass)?)?;
    for &(i, ch) in chronicles.iter() {
        match &ch.origin {
            ChronicleOrigin::Refinement { refined, .. } if refined.contains(&task_id) => {
                format_chronicle_partial((i, ch), chronicles, ass, depth + 2, out)?;
            }
            _ => (),
        }
    }

    Ok(())
}

pub fn format_partial_plan(problem: &FiniteProblem, ass: &Model) -> Result<String> {
    let mut f = String::new();
    writeln!(f, "==>")?;

    let mut chronicles: Vec<_> = problem
        .chronicles
        .iter()
        .enumerate()
        // .filter(|ch| ass.boolean_value_of(ch.1.chronicle.presence) == Some(true))
        .collect();
    // sort by start times
    chronicles.sort_by_key(|ch| ass.f_domain(ch.1.chronicle.start).num.lb);

    for &(i, ch) in &chronicles {
        match ch.origin {
            ChronicleOrigin::Refinement { .. } => {}
            _ => format_chronicle_partial((i, ch), &chronicles, ass, 0, &mut f)?,
        }
    }
    Ok(f)
}

pub fn extract_plan(problem: &FiniteProblem, ass: &Domains) -> Result<Vec<ActionInstance>> {
    let mut plan = problem.chronicles.iter().try_fold(vec![], |p, c| {
        let mut r = p.clone();
        r.extend(extract_plan_actions(c, problem, ass)?);
        Ok(r)
    })?;
    plan.sort_by_key(|a| a.start);
    Ok(plan)
}

pub fn extract_plan_actions(
    ch: &ChronicleInstance,
    problem: &FiniteProblem,
    ass: &Domains,
) -> Result<Vec<ActionInstance>> {
    if ass.value(ch.chronicle.presence) != Some(true) {
        return Ok(vec![]);
    }
    match ch.chronicle.kind {
        ChronicleKind::Problem | ChronicleKind::Method => return Ok(vec![]),
        _ => {}
    }
    let start = ass.f_domain(ch.chronicle.start).lb();
    let end = ass.f_domain(ch.chronicle.end).lb();
    let duration = end - start;
    let name = format_atom(&ch.chronicle.name[0], &problem.model, ass);
    let params = ch.chronicle.name[1..]
        .iter()
        .map(|atom| ass.evaluate(*atom).unwrap())
        .collect_vec();

    let instance = ActionInstance {
        name,
        params,
        start,
        duration,
    };

    // if the action corresponds to a rolled-up action, unroll it in the solution
    let roll_compil = match ch.origin {
        ChronicleOrigin::FreeAction { template_id, .. } => problem.meta.action_rolling.get(&template_id),
        _ => None,
    };
    Ok(if let Some(roll_compil) = roll_compil {
        roll_compil.unroll(&instance)
    } else {
        vec![instance]
    })
}

fn str(r: Rational) -> String {
    let scale = TIME_SCALE.get();
    if scale % r.denom() != 0 {
        // default to formatting float
        return format!("{:.3}", *r.numer() as f32 / *r.denom() as f32);
    }
    let r_scaled = (r * scale).to_integer();
    let int_part = r_scaled / scale;
    let decimal_part = r_scaled % scale;

    match scale {
        1 => format!("{int_part}"),
        10 => format!("{int_part}.{decimal_part:0<1}"),
        100 => format!("{int_part}.{decimal_part:0<2}"),
        1000 => format!("{int_part}.{decimal_part:0<3}"),
        _ => format!("{:.3}", *r.numer() as f32 / *r.denom() as f32), // default to formatting float
    }
}

pub fn format_pddl_plan(problem: &FiniteProblem, ass: &Domains) -> Result<String> {
    let mut out = String::new();
    let plan = extract_plan(problem, ass)?;
    for a in &plan {
        let start = str(a.start);
        let duration = str(a.duration);
        write!(out, "{start:>5}: ({}", a.name)?;
        for &p in &a.params {
            write!(out, " {}", format_cst(p, &problem.model))?;
        }
        writeln!(out, ") [{duration}]")?;
    }
    Ok(out)
}

/// Formats a hierarchical plan into the format expected by pandaPIparser's verifier
pub fn format_hddl_plan(problem: &FiniteProblem, ass: &Domains) -> Result<String> {
    let mut f = String::new();
    writeln!(f, "==>")?;
    let fmt1 = |x: &Atom| -> String { format_atom(x, &problem.model, ass) };
    let fmt = |name: &[Atom]| -> Result<String> { format_name(name, &problem.model, ass) };
    let mut chronicles: Vec<_> = problem
        .chronicles
        .iter()
        .enumerate()
        .filter(|ch| ass.boolean_value_of(ch.1.chronicle.presence) == Some(true))
        .collect();
    // sort by start times
    chronicles.sort_by_key(|ch| ass.f_domain(ch.1.chronicle.start).num.lb);

    // print all actions with their ids
    for &(i, ch) in &chronicles {
        if ch.chronicle.kind == ChronicleKind::Action || ch.chronicle.kind == ChronicleKind::DurativeAction {
            writeln!(f, "{} {}", i, fmt(&ch.chronicle.name)?)?;
        }
    }
    // print the ids of all subtasks of the given chronicle
    let print_subtasks_ids = |out: &mut String, chronicle_id: usize| -> Result<()> {
        for &(i, ch) in &chronicles {
            match &ch.origin {
                ChronicleOrigin::Refinement { refined, .. }
                    if refined.iter().any(|tid| tid.instance_id == chronicle_id) =>
                {
                    write!(out, " {i}")?;
                }
                _ => (),
            }
        }
        Ok(())
    };
    // for the root and each method, print their name all subtasks
    for &(i, ch) in &chronicles {
        match ch.chronicle.kind {
            ChronicleKind::Action | ChronicleKind::DurativeAction => continue,
            ChronicleKind::Problem => write!(f, "root")?,
            ChronicleKind::Method => {
                write!(
                    f,
                    "{} {} -> {}",
                    i,
                    fmt(ch.chronicle.task.as_ref().unwrap())?,
                    fmt1(&ch.chronicle.name[0])
                )?;
            }
        }
        print_subtasks_ids(&mut f, i)?;
        writeln!(f)?;
    }
    writeln!(f, "<==")?;
    Ok(f)
}
