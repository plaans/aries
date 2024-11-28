use anyhow::{ensure, Context, Result};
use aries::core::state::Domains;
use aries::model::extensions::AssignmentExt;
use aries::model::lang::{Atom, FAtom};
use aries_planners::encoding::ChronicleId;
use aries_planners::fmt::{extract_plan_actions, format_atom};
use aries_planning::chronicles::{ChronicleKind, ChronicleOrigin, FiniteProblem, TaskId};
use std::collections::HashMap;
use unified_planning as up;
use unified_planning::{Real, Schedule};

pub fn serialize_plan(
    problem_request: &up::Problem,
    problem: &FiniteProblem,
    assignment: &Domains,
) -> Result<unified_planning::Plan> {
    // retrieve all chronicles present in the solution, with their chronicle_id
    let mut chronicles: Vec<_> = problem
        .chronicles
        .iter()
        .enumerate()
        .filter(|ch| assignment.boolean_value_of(ch.1.chronicle.presence) == Some(true))
        .collect();
    // sort by start times
    chronicles.sort_by_key(|ch| assignment.f_domain(ch.1.chronicle.start).num.lb);

    // helper functions that return the ChronicleId of the chronicle refining the task
    let refining_chronicle = |task_id: TaskId| -> Result<ChronicleId> {
        for &(i, ch) in &chronicles {
            match &ch.origin {
                ChronicleOrigin::Refinement { refined, .. } if refined.iter().any(|tid| tid == &task_id) => {
                    return Ok(i)
                }
                _ => (),
            }
        }
        anyhow::bail!("No chronicle refining task {:?}", &task_id)
    };

    let mut actions = Vec::new();
    let mut hier = up::PlanHierarchy {
        root_tasks: Default::default(),
        methods: vec![],
    };

    for &(id, ch) in &chronicles {
        if assignment.value(ch.chronicle.presence) != Some(true) {
            continue; // chronicle is absent, skip
        }

        let subtasks: HashMap<String, String> = ch
            .chronicle
            .subtasks
            .iter()
            .enumerate()
            .map(|(tid, t)| {
                let subtask_up_id = t.id.as_ref().cloned().unwrap_or_default();
                let subtask_id = TaskId {
                    instance_id: id,
                    task_id: tid,
                };
                (subtask_up_id, refining_chronicle(subtask_id).unwrap().to_string())
            })
            .collect();

        match ch.chronicle.kind {
            ChronicleKind::Problem => {
                // base chronicles, its subtasks are the problem's subtasks
                ensure!(hier.root_tasks.is_empty(), "More than one set of root tasks.");
                hier.root_tasks = subtasks;
            }
            ChronicleKind::Method => {
                let name = match ch.chronicle.kind {
                    ChronicleKind::Problem => "problem".to_string(),
                    ChronicleKind::Method | ChronicleKind::Action | ChronicleKind::DurativeAction => {
                        format_atom(&ch.chronicle.name[0], &problem.model, assignment)
                    }
                };
                let parameters = ch.chronicle.name[1..]
                    .iter()
                    .map(|&param| serialize_atom(param, problem, assignment))
                    .collect::<Result<Vec<_>>>()?;
                hier.methods.push(up::MethodInstance {
                    id: id.to_string(),
                    method_name: name.to_string(),
                    parameters,
                    subtasks,
                });
            }
            ChronicleKind::Action | ChronicleKind::DurativeAction => {
                ensure!(subtasks.is_empty(), "Action with subtasks.");
                let instances = extract_plan_actions(ch, problem, assignment)?;
                actions.extend(
                    instances
                        .iter()
                        .map(|a| {
                            // The id is used in HTNs plans where there are no rolling
                            let id = if problem_request.hierarchy.is_some() {
                                ensure!(instances.len() == 1, "Rolling in HTN plan");
                                id.to_string()
                            } else {
                                (actions.len() + id).to_string()
                            };
                            let parameters = a
                                .params
                                .iter()
                                .map(|&p| serialize_atom(p.into(), problem, assignment))
                                .collect::<Result<Vec<_>>>()?;
                            let start_time = Some(serialize_time(a.start.into(), assignment)?);
                            let end_time = Some(serialize_time((a.start + a.duration).into(), assignment)?);

                            Ok(up::ActionInstance {
                                id,
                                action_name: a.name.to_string(),
                                parameters,
                                start_time,
                                end_time,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?,
                );
            }
        };
    }
    // sort actions by increasing start time.
    actions.sort_by_key(|a| real_to_rational(a.start_time.as_ref().unwrap()));

    fn is_temporal(feature: i32) -> bool {
        feature == (up::Feature::ContinuousTime as i32) || feature == (up::Feature::DiscreteTime as i32)
    }
    if !problem_request.features.iter().any(|feature| is_temporal(*feature)) {
        // the problem is not temporal, remove time annotations
        // Note that the sorting done earlier ensures the plan is a valid sequence
        for action in &mut actions {
            action.start_time = None;
            action.end_time = None;
        }
    }

    let hierarchy = if problem_request.hierarchy.is_some() {
        Some(hier)
    } else {
        None
    };

    // If this is a scheduling problem, interpret all actions as activities
    // TODO: currently, variables are not supported.
    let schedule = if problem_request.scheduling_extension.is_some() {
        let mut schedule = Schedule {
            activities: vec![],
            variable_assignments: Default::default(),
        };
        for a in actions.drain(..) {
            // empty all actions and transform them into a schedule
            let name = a.action_name;
            schedule.variable_assignments.insert(
                format!("{name}.start"),
                a.start_time.expect("No start time in scheduling solution").into(),
            );
            schedule.variable_assignments.insert(
                format!("{name}.end"),
                a.end_time.expect("No end time in scheduling solution").into(),
            );
            if !a.parameters.is_empty() {
                // Search for the corresponding activity definition
                let act = problem_request
                    .scheduling_extension
                    .as_ref()
                    .expect("Missing scheduling extension")
                    .activities
                    .iter()
                    .find(|a| a.name == name)
                    .unwrap_or_else(|| panic!("Missing the activity `{}` definition", name));

                // Assign the solution value to each action parameter
                for (v, p) in a.parameters.iter().zip(&act.parameters) {
                    schedule.variable_assignments.insert(p.name.clone(), v.clone());
                }
            }
            schedule.activities.push(name);
        }
        Some(schedule)
    } else {
        None
    };
    Ok(up::Plan {
        actions,
        hierarchy,
        schedule,
    })
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

pub fn engine() -> up::Engine {
    up::Engine {
        name: "aries".to_string(),
    }
}
