//! Extraction of a plan from a solution, as plain data passed to Python.
//!
//! This is a port of `planning/grpc/server/src/serialize.rs`: the protobuf `Plan` is replaced by
//! structures with the same fields that are converted to Python dicts.

use anyhow::{ensure, Context, Result};
use aries_planners::encoding::ChronicleId;
use aries_planners::fmt::{extract_plan_actions, format_atom};
use aries_planning::chronicles::{ChronicleKind, ChronicleOrigin, FiniteProblem, TaskId};
use aries_planning::legacy::*;
use aries_solver::prelude::*;
use num_rational::Rational64;
use pyo3::IntoPyObject;
use std::collections::HashMap;

/// Information on the original problem that is needed to build its plans.
#[derive(Clone, Debug)]
pub struct ProblemInfo {
    pub hierarchical: bool,
    pub temporal: bool,
    /// Parameter names of each activity, for scheduling problems.
    pub activities: Option<HashMap<String, Vec<String>>>,
}

#[derive(Clone, Debug, IntoPyObject)]
pub enum Value {
    Symbol(String),
    Int(i64),
    Real(Rational64),
    Boolean(bool),
}

#[derive(Clone, Debug, IntoPyObject)]
pub struct ActionInstance {
    pub id: String,
    pub action_name: String,
    pub parameters: Vec<Value>,
    pub start_time: Option<Rational64>,
    pub end_time: Option<Rational64>,
}

#[derive(Clone, Debug, IntoPyObject)]
pub struct MethodInstance {
    pub id: String,
    pub method_name: String,
    pub parameters: Vec<Value>,
    pub subtasks: HashMap<String, String>,
}

#[derive(Clone, Debug, IntoPyObject)]
pub struct PlanHierarchy {
    pub root_tasks: HashMap<String, String>,
    pub methods: Vec<MethodInstance>,
}

#[derive(Clone, Debug, IntoPyObject)]
pub struct Schedule {
    pub activities: Vec<String>,
    pub variable_assignments: HashMap<String, Value>,
}

#[derive(Clone, Debug, IntoPyObject)]
pub struct Plan {
    pub actions: Vec<ActionInstance>,
    pub hierarchy: Option<PlanHierarchy>,
    pub schedule: Option<Schedule>,
}

pub fn serialize_plan(problem_request: &ProblemInfo, problem: &FiniteProblem, assignment: &Solution) -> Result<Plan> {
    // retrieve all chronicles present in the solution, with their chronicle_id
    let mut chronicles: Vec<_> = problem
        .chronicles
        .iter()
        .enumerate()
        .filter(|ch| assignment.boolean_value_of(ch.1.chronicle.presence) == Some(true))
        .collect();
    // sort by start times
    chronicles.sort_by_key(|ch| assignment.lb(ch.1.chronicle.start));

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
    let mut hier = PlanHierarchy {
        root_tasks: Default::default(),
        methods: vec![],
    };

    for &(id, ch) in &chronicles {
        if assignment.value_of(ch.chronicle.presence) != Some(true) {
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
                Ok((subtask_up_id, refining_chronicle(subtask_id)?.to_string()))
            })
            .collect::<Result<_>>()?;

        match ch.chronicle.kind {
            ChronicleKind::Problem => {
                // base chronicles, its subtasks are the problem's subtasks
                ensure!(hier.root_tasks.is_empty(), "More than one set of root tasks.");
                hier.root_tasks = subtasks;
            }
            ChronicleKind::Method => {
                let name = format_atom(&ch.chronicle.name[0], problem, assignment);
                let parameters = ch.chronicle.name[1..]
                    .iter()
                    .map(|&param| serialize_atom(param, problem, assignment))
                    .collect::<Result<Vec<_>>>()?;
                hier.methods.push(MethodInstance {
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
                            let id = if problem_request.hierarchical {
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

                            Ok(ActionInstance {
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
    actions.sort_by_key(|a| a.start_time.unwrap());

    if !problem_request.temporal {
        // the problem is not temporal, remove time annotations
        // Note that the sorting done earlier ensures the plan is a valid sequence
        for action in &mut actions {
            action.start_time = None;
            action.end_time = None;
        }
    }

    let hierarchy = if problem_request.hierarchical { Some(hier) } else { None };

    // If this is a scheduling problem, interpret all actions as activities
    // TODO: currently, variables are not supported.
    let schedule = if let Some(activities) = problem_request.activities.as_ref() {
        let mut schedule = Schedule {
            activities: vec![],
            variable_assignments: Default::default(),
        };
        for a in actions.drain(..) {
            // empty all actions and transform them into a schedule
            let name = a.action_name;
            schedule.variable_assignments.insert(
                format!("{name}.start"),
                Value::Real(a.start_time.context("No start time in scheduling solution")?),
            );
            schedule.variable_assignments.insert(
                format!("{name}.end"),
                Value::Real(a.end_time.context("No end time in scheduling solution")?),
            );
            if !a.parameters.is_empty() {
                // Search for the corresponding activity definition
                let act = activities
                    .get(&name)
                    .with_context(|| format!("Missing the activity `{}` definition", name))?;

                // Assign the solution value to each action parameter
                for (v, p) in a.parameters.iter().zip(act) {
                    schedule.variable_assignments.insert(p.clone(), v.clone());
                }
            }
            schedule.activities.push(name);
        }
        Some(schedule)
    } else {
        None
    };
    Ok(Plan {
        actions,
        hierarchy,
        schedule,
    })
}

fn serialize_time(fatom: FAtom, ass: &Solution) -> Result<Rational64> {
    let num = ass.var_domain(fatom.num).as_singleton().context("Unbound variable")?;
    Ok(Rational64::new(num as i64, fatom.denom as i64))
}

fn serialize_atom(atom: Atom, pb: &FiniteProblem, ass: &Solution) -> Result<Value> {
    let content = match atom {
        Atom::Bool(l) => {
            let value = ass.value_of(l).context("Unassigned literal")?;
            Value::Boolean(value)
        }
        Atom::Int(i) => {
            let value = ass.var_domain(i).as_singleton().context("Unbound int variable")?;

            Value::Int(value as i64)
        }
        Atom::Fixed(f) => Value::Real(serialize_time(f, ass)?),
        Atom::Sym(s) => {
            let sym_id = ass.eval(s).context("Unbound sym var")?;
            let sym = pb.symbols.symbol(sym_id);
            Value::Symbol(sym.to_string())
        }
    };
    Ok(content)
}
