use anyhow::{ensure, Context, Result};
use aries::core::state::Domains;
use aries::model::extensions::AssignmentExt;
use aries::model::lang::{Atom, FAtom, SAtom};
use aries_planners::encoding::ChronicleId;
use aries_planning::chronicles::{ChronicleKind, ChronicleOrigin, FiniteProblem, TaskId};
use std::collections::HashMap;
use unified_planning as up;
use unified_planning::{Real, Schedule};

pub fn serialize_plan(
    _problem_request: &up::Problem,
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
        let start = serialize_time(ch.chronicle.start, assignment)?;
        let end = serialize_time(ch.chronicle.end, assignment)?;

        // extract name and parameters (possibly empty if not an action or method chronicle)
        let name = if let Some(name) = ch.chronicle.name.get(0) {
            let name = SAtom::try_from(*name).context("Action name is not a symbol")?;
            let name = assignment.sym_value_of(name).context("Unbound sym var")?;
            problem.model.shape.symbols.symbol(name).to_string()
        } else {
            "".to_string()
        };

        let parameters = if ch.chronicle.name.len() > 1 {
            ch.chronicle.name[1..]
                .iter()
                .map(|&param| serialize_atom(param, problem, assignment))
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        // map identifying subtasks of the chronicle
        let mut subtasks: HashMap<String, String> = Default::default();
        for (tid, t) in ch.chronicle.subtasks.iter().enumerate() {
            let subtask_up_id = t.id.as_ref().cloned().unwrap_or_default();
            let subtask_id = TaskId {
                instance_id: id,
                task_id: tid,
            };
            subtasks.insert(subtask_up_id, refining_chronicle(subtask_id)?.to_string());
        }

        match ch.chronicle.kind {
            ChronicleKind::Problem => {
                // base chronicles, its subtasks are the problem's subtasks
                ensure!(hier.root_tasks.is_empty(), "More than one set of root tasks.");
                hier.root_tasks = subtasks;
            }
            ChronicleKind::Action | ChronicleKind::DurativeAction => {
                ensure!(subtasks.is_empty(), "Action with subtasks.");
                actions.push(up::ActionInstance {
                    id: id.to_string(),
                    action_name: name.to_string(),
                    parameters,
                    start_time: Some(start),
                    end_time: Some(end),
                });
            }

            ChronicleKind::Method => {
                hier.methods.push(up::MethodInstance {
                    id: id.to_string(),
                    method_name: name.to_string(),
                    parameters,
                    subtasks,
                });
            }
        }
    }
    // sort actions by increasing start time.
    actions.sort_by_key(|a| real_to_rational(a.start_time.as_ref().unwrap()));

    fn is_temporal(feature: i32) -> bool {
        feature == (up::Feature::ContinuousTime as i32) || feature == (up::Feature::DiscreteTime as i32)
    }
    if !_problem_request.features.iter().any(|feature| is_temporal(*feature)) {
        // the problem is not temporal, remove time annotations
        // Note that the sorting done earlier ensures the plan is a valid sequence
        for action in &mut actions {
            action.start_time = None;
            action.end_time = None;
        }
    }

    let hierarchy = if _problem_request.hierarchy.is_some() {
        Some(hier)
    } else {
        None
    };

    // If this is a scheduling problem, interpret all actions as activities
    // TODO: currently, variables are not supported.
    let schedule = if _problem_request.scheduling_extension.is_some() {
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
                let mut act: Option<up::Activity> = None;
                for _a in _problem_request
                    .scheduling_extension
                    .as_ref()
                    .unwrap()
                    .activities
                    .iter()
                {
                    if _a.name == name {
                        act = Some(_a.clone());
                        break;
                    }
                }
                let act = act.unwrap_or_else(|| panic!("Cannot find the activity {} definition", name));
                // Assign the solution value to each action parameter
                for (v, p) in a.parameters.iter().zip(act.parameters) {
                    schedule.variable_assignments.insert(p.name, v.clone());
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
