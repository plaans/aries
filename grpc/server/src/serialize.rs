// This module parses the GRPC service definition into a set of Rust structs.
use anyhow::{Context, Result};
use aries_core::state::Domains;
use aries_model::extensions::AssignmentExt;
use aries_model::lang::{Atom, FAtom};
use aries_planning::chronicles::{ChronicleInstance, ChronicleKind, FiniteProblem};
use unified_planning as up;
use unified_planning::Real;

pub fn serialize_plan(
    _problem_request: &up::Problem,
    problem: &FiniteProblem,
    assignment: &Domains,
) -> Result<unified_planning::Plan> {
    let mut actions = Vec::new();

    // retrieve all actions present in the solution
    for ch in &problem.chronicles {
        if assignment.value(ch.chronicle.presence) != Some(true) {
            continue;
        }
        match ch.chronicle.kind {
            ChronicleKind::Problem | ChronicleKind::Method => continue,
            _ => {}
        }
        let action = serialize_action_instance(ch, problem, assignment)?;
        actions.push(action);
    }
    // sort actions by increasing start time
    actions.sort_by_key(|a| real_to_rational(a.start_time.as_ref().unwrap()));
    Ok(up::Plan { actions })
}

fn rational_to_real(r: num_rational::Rational64) -> up::Real {
    Real {
        numerator: *r.numer(),
        denominator: *r.denom(),
    }
}
fn real_to_rational(r: &up::Real) -> num_rational::Rational64 {
    num_rational::Rational64::new(r.numerator, r.denominator)
}

fn serialize_time(fatom: FAtom, ass: &Domains) -> Result<up::Real> {
    let num = ass.var_domain(fatom.num).as_singleton().context("Unbound variable")?;
    Ok(rational_to_real(num_rational::Rational64::new(
        num as i64,
        fatom.denom as i64,
    )))
}

fn serialize_atom(atom: Atom, pb: &FiniteProblem, ass: &Domains) -> Result<up::Atom> {
    let content = match atom {
        Atom::Bool(l) => {
            let value = ass.value(l).context("Unassigned literal")?;
            up::atom::Content::Boolean(value)
        }
        Atom::Int(i) => {
            let value = ass.var_domain(i).as_singleton().context("Unbound int variable")?;

            up::atom::Content::Int(value as i64)
        }
        Atom::Fixed(f) => up::atom::Content::Real(serialize_time(f, ass)?),
        Atom::Sym(s) => {
            let sym_id = ass.sym_value_of(s).context("Unbound sym var")?;
            let sym = pb.model.shape.symbols.symbol(sym_id);
            up::atom::Content::Symbol(sym.to_string())
        }
    };
    Ok(up::Atom { content: Some(content) })
}

pub fn serialize_action_instance(
    ch: &ChronicleInstance,
    pb: &FiniteProblem,
    ass: &Domains,
) -> Result<up::ActionInstance> {
    debug_assert_eq!(ass.value(ch.chronicle.presence), Some(true));
    debug_assert!(ch.chronicle.kind == ChronicleKind::DurativeAction || ch.chronicle.kind == ChronicleKind::Action);

    let start = serialize_time(ch.chronicle.start, ass)?;
    let end = serialize_time(ch.chronicle.end, ass)?;

    let name = ch.chronicle.name[0];
    let name = ass.sym_value_of(name).context("Unbound sym var")?;
    let name = pb.model.shape.symbols.symbol(name);

    let parameters = ch.chronicle.name[1..]
        .iter()
        .map(|&param| serialize_atom(param.into(), pb, ass))
        .collect::<Result<Vec<_>>>()?;

    Ok(up::ActionInstance {
        id: "".to_string(),
        action_name: name.to_string(),
        parameters,
        start_time: Some(start),
        end_time: Some(end),
    })
}

pub fn engine() -> up::Engine {
    up::Engine {
        name: "aries".to_string(),
    }
}
