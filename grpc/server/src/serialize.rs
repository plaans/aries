// This module parses the GRPC service definition into a set of Rust structs.
use anyhow::Error;
use aries_core::state::Domains;
use aries_grpc_api::PlanGenerationResult;
use aries_model::extensions::AssignmentExt;
use aries_model::lang::SAtom;
use aries_planning::chronicles::{ChronicleKind, FiniteProblem};
use std::sync::Arc;

pub fn serialize_answer(
    _problem_request: &aries_grpc_api::Problem,
    problem: &FiniteProblem,
    assignment: &Option<Arc<Domains>>,
) -> Result<aries_grpc_api::PlanGenerationResult, Error> {
    if let Some(assignment) = assignment {
        let fmt = |name: &[SAtom]| -> String {
            let syms: Vec<_> = name
                .iter()
                .map(|x| assignment.sym_domain_of(*x).into_singleton().unwrap())
                .collect();
            problem.model.shape.symbols.format(&syms)
        };

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
        // TODO: Check that the parameters are valid.
        // TODO: Add metrics to the final report.
        Ok(PlanGenerationResult::default())
    } else {
        Err(Error::msg("No assignment provided"))

        // Rewrite the plan to be a list of actions.
        //     let mut action_instances = Vec::new();
        //     for (start, name, duration) in plan {
        //         let parameters = problem_request
        //             .actions
        //             .iter()
        //             .find(|x| x.name == name)
        //             .unwrap()
        //             .parameters
        //             .clone();

        //         let parameters = parameters
        //             .iter()
        //             .map(|x| Atom {
        //                 content: Some(atom::Content::Symbol(x.name.clone())),
        //             })
        //             .collect();

        //         let action_instance = ActionInstance {
        //             action_name: name,
        //             start_time: start.into(),
        //             end_time: (start + duration).into(),
        //             id: "".to_string(),
        //             parameters,
        //         };
        //         action_instances.push(action_instance);
        //     }
        //     let _report = FinalReport {
        //         status: final_report::Status::Opt.into(),
        //         best_plan: Some(Plan {
        //             actions: action_instances,
        //         }),
        //         logs: vec![],
        //         metrics: HashMap::new(),
        //     };
        //     Ok(Answer {
        //         content: Some(answer::Content::Final(_report)),
        //     })
        // } else {
        //     let _report = FinalReport {
        //         status: final_report::Status::InternalError.into(),
        //         best_plan: None,
        //         logs: vec![],
        //         metrics: HashMap::new(),
        //     };
        //     Ok(Answer {
        //         content: Some(answer::Content::Final(_report)),
        //     })
    }
}
