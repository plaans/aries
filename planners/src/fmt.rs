//! Functions related to printing and formatting (partial) plans.

use anyhow::*;
use std::fmt::Write;

use aries_model::extensions::{AssignmentExt, SavedAssignment, Shaped};
use aries_model::lang::SAtom;
use aries_model::symbols::SymId;
use aries_model::Model;
use aries_planning::chronicles::{ChronicleInstance, ChronicleKind, ChronicleOrigin, FiniteProblem, SubTask};

pub fn format_partial_symbol(x: &SAtom, ass: &Model, out: &mut String) {
    let dom = ass.sym_domain_of(*x);
    // based on symbol presence, either return "_" (absence) or have a an "?" prefix if presence if not determined
    let prefix = match ass.sym_present(*x) {
        Some(false) => {
            write!(out, "_").unwrap();
            return;
        }
        None => "?",
        Some(true) => "",
    };
    let singleton = dom.size() == 1;
    if !singleton {
        write!(out, "{}{{", prefix).unwrap();
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

pub fn format_partial_name(name: &[SAtom], ass: &Model) -> Result<String> {
    let mut res = String::new();
    write!(res, "(")?;
    for (i, sym) in name.iter().enumerate() {
        format_partial_symbol(sym, ass, &mut res);
        if i != (name.len() - 1) {
            write!(res, " ")?;
        }
    }
    write!(res, ")")?;
    Ok(res)
}

pub fn format_atoms(variables: &[SAtom], ass: &Model) -> Result<String> {
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
            format_task_partial((ch_id, task_id), task, chronicles, ass, depth + 2, out)?;
        }
    }
    Ok(())
}
fn format_task_partial(
    (containing_ch_id, containing_subtask_id): (usize, usize),
    task: &SubTask,
    chronicles: &[Chronicle],
    ass: &Model,
    depth: usize,
    out: &mut String,
) -> Result<()> {
    write!(out, "{}", "  ".repeat(depth))?;
    let start = ass.int_bounds(task.start).0;
    write!(out, "{} {}", start, format_partial_name(&task.task, ass)?)?;
    writeln!(out, "         {}", format_atoms(&task.task, ass)?)?;
    for &(i, ch) in chronicles.iter() {
        match ch.origin {
            ChronicleOrigin::Refinement { instance_id, task_id }
                if instance_id == containing_ch_id && task_id == containing_subtask_id =>
            {
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
    chronicles.sort_by_key(|ch| ass.domain_of(ch.1.chronicle.start).0);

    for &(i, ch) in &chronicles {
        match ch.origin {
            ChronicleOrigin::Refinement { .. } => {}
            _ => format_chronicle_partial((i, ch), &chronicles, ass, 0, &mut f)?,
        }
    }
    Ok(f)
}

pub fn format_pddl_plan(problem: &FiniteProblem, ass: &SavedAssignment) -> Result<String> {
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
pub fn format_hddl_plan(problem: &FiniteProblem, ass: &SavedAssignment) -> Result<String> {
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
                fmt(ch.chronicle.task.as_ref().unwrap()),
                fmt1(&ch.chronicle.name[0])
            )?;
        }
        print_subtasks_ids(&mut f, i)?;
        writeln!(f)?;
    }
    writeln!(f, "<==")?;
    Ok(f)
}
