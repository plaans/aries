// This module parses the GRPC service definition into a set of Rust structs.
use anyhow::Error;
use aries_core::state::Domains;
use aries_grpc_api::{final_report, Action, ActionInstance, FinalReport, Plan};
use aries_model::extensions::AssignmentExt;
use aries_model::lang::SAtom;
use aries_planning::chronicles::{ChronicleKind, FiniteProblem};
use std::sync::Arc;

pub fn serialize_answer(problem: &FiniteProblem, assignment: &Arc<Domains>) -> Result<aries_grpc_api::Answer, Error> {
    let fmt = |name: &[SAtom]| -> String {
        let syms: Vec<_> = name
            .iter()
            .map(|x| assignment.sym_domain_of(*x).into_singleton().unwrap())
            .collect();
        problem.model.shape.symbols.format(&syms)
    };

    let answer = aries_grpc_api::Answer::default();
    let mut plan = Vec::new();
    for ch in &problem.chronicles {
        if assignment.value(ch.chronicle.presence) != Some(true) {
            continue;
        }
        match ch.chronicle.kind {
            ChronicleKind::Problem | ChronicleKind::Method => continue,
            _ => {}
        }
        let start = assignment.f_domain(ch.chronicle.start).lb();
        let end = assignment.f_domain(ch.chronicle.end).lb();
        let duration = end - start;
        let name = fmt(&ch.chronicle.name);
        plan.push((start, name.clone(), duration));
    }

    plan.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // TODO: Log messages

    // Rewrite the plan to be a list of actions.
    let mut actions = Vec::new();
    for (start, name, duration) in plan {
        let action_instance = ActionInstance {
            action_name: name,
            start_time: start.into(),
            end_time: (start + duration).into(),
            parameters: todo!(),
            id: todo!(),
        };
        actions.push(action_instance);
    }
    Ok(answer)
}
