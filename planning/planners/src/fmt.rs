//! Functions related to printing and formatting (partial) plans.

use anyhow::*;
use std::fmt::Write;

use crate::Model;
use aries::model::extensions::{AssignmentExt, SavedAssignment, Shaped};
use aries::model::lang::{Atom, SAtom};
use aries_planning::chronicles::{ChronicleInstance, ChronicleKind, ChronicleOrigin, FiniteProblem, SubTask, TaskId};

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
            let dom = ass.sym_domain_of(x);
            let singleton = dom.size() == 1;
            if !singleton {
                write!(out, "{prefix}{{").unwrap();
            }
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
        Atom::Bool(l) => {
            write!(
                out,
                "{}",
                match ass.value_of_literal(l) {
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

pub fn format_partial_name(name: &[Atom], ass: &Model) -> Result<String> {
    let mut res = String::new();
    write!(res, "(")?;
    for (i, sym) in name.iter().enumerate() {
        format_partial_atom(*sym, ass, &mut res);
        if i != (name.len() - 1) {
            write!(res, " ")?;
        }
    }
    write!(res, ")")?;
    Ok(res)
}

pub fn format_atoms(variables: &[Atom], ass: &Model) -> Result<String> {
    let mut res = String::new();
    write!(res, "(")?;
    for (i, sym) in variables.iter().enumerate() {
        write!(res, "{}", ass.fmt(*sym))?;
        if i != (variables.len() - 1) {
            write!(res, " ")?;
        }
    }
    write!(res, ")")?;
    Ok(res)
}
pub fn format_atom(variable: Atom, ass: &Model) -> Result<String> {
    let mut res = String::new();
    format_partial_atom(variable, ass, &mut res);
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
    write!(out, " {}", format_partial_name(&ch.chronicle.name, ass)?)?;
    writeln!(out, "         {}", format_atoms(&ch.chronicle.name, ass)?)?;
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

pub fn format_pddl_plan(problem: &FiniteProblem, ass: &SavedAssignment) -> Result<String> {
    let fmt = |name: &[SAtom]| -> String {
        let syms: Vec<_> = name
            .iter()
            .map(|x| ass.sym_domain_of(*x).into_singleton().unwrap())
            .collect();
        problem.model.shape.symbols.format(&syms)
    };

    let mut out = String::new();
    let mut plan = Vec::new();
    for ch in &problem.chronicles {
        if ass.value(ch.chronicle.presence) != Some(true) {
            continue;
        }
        match ch.chronicle.kind {
            ChronicleKind::Problem | ChronicleKind::Method => continue,
            _ => {}
        }
        let start = ass.f_domain(ch.chronicle.start).lb();
        let end = ass.f_domain(ch.chronicle.end).lb();
        let duration = end - start;
        let name = format_partial_name(&ch.chronicle.name, &problem.model)?;
        plan.push((start, name.clone(), duration));
    }

    plan.sort_by(|a, b| a.partial_cmp(b).unwrap());
    for (start, name, duration) in plan {
        writeln!(out, "{start:>2}: {name} [{duration:.3}]")?;
    }
    Ok(out)
}

/// Formats a hierarchical plan into the format expected by pandaPIparser's verifier
pub fn format_hddl_plan(problem: &FiniteProblem, ass: &SavedAssignment) -> Result<String> {
    let mut f = String::new();
    writeln!(f, "==>")?;
    let fmt1 = |x: &SAtom| -> String {
        let sym = ass.sym_domain_of(*x).into_singleton().unwrap();
        problem.model.shape.symbols.symbol(sym).to_string()
    };
    let fmt = |name: &[SAtom]| -> String {
        let syms: Vec<_> = name
            .iter()
            .map(|x| ass.sym_domain_of(*x).into_singleton().unwrap())
            .collect();
        problem.model.shape.symbols.format(&syms)
    };
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
            writeln!(f, "{} {}", i, format_partial_name(&ch.chronicle.name, &problem.model)?)?;
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
                    format_partial_name(ch.chronicle.task.as_ref().unwrap(), &problem.model)?,
                    {
                        let mut str = String::new();
                        format_partial_atom(ch.chronicle.name[0], &problem.model, &mut str);
                        str
                    }
                )?;
            }
        }
        print_subtasks_ids(&mut f, i)?;
        writeln!(f)?;
    }
    writeln!(f, "<==")?;
    Ok(f)
}
