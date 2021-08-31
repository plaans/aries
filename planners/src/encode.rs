//! Functions whose purpose is to encode a planning problem (represented with chronicles)
//! into a combinatorial problem from Aries core.

use crate::encoding::{conditions, effects, refinements_of, refinements_of_task, TaskRef, HORIZON, ORIGIN};
use anyhow::*;
use aries_model::assignments::Assignment;
use aries_model::bounds::Lit;
use aries_model::lang::{BAtom, VarRef};
use aries_model::lang::{IAtom, Variable};
use aries_model::Model;
use aries_planning::chronicles::constraints::ConstraintType;
use aries_planning::chronicles::{
    ChronicleInstance, ChronicleOrigin, ChronicleTemplate, FiniteProblem, InvalidSubstitution, Problem, Sub,
    Substitution, Sv, Task,
};
use env_param::EnvParam;
use std::convert::TryInto;

/// Parameter that defines the symmetry breaking strategy to use.
/// The value of this parameter is loaded from the environment variable `ARIES_LCP_SYMMETRY_BREAKING`.
/// Possible values are `none` and `simple` (default).
pub static SYMMETRY_BREAKING: EnvParam<SymmetryBreakingType> = EnvParam::new("ARIES_LCP_SYMMETRY_BREAKING", "simple");

impl std::str::FromStr for SymmetryBreakingType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(SymmetryBreakingType::None),
            "simple" => Ok(SymmetryBreakingType::Simple),
            x => Err(format!("Unknown symmetry breaking type: {}", x)),
        }
    }
}

/// The type of symmetry breaking to apply to problems.
#[derive(Copy, Clone)]
pub enum SymmetryBreakingType {
    /// no symmetry breaking
    None,
    /// Simple form of symmetry breaking described in the LCP paper (CP 2018).
    /// This enforces that for any two instances of the same template. The first one (in arbitrary total order)
    ///  - is always present if the second instance is present
    ///  - starts before the second instance
    Simple,
}

/// For each chronicle template into the `spec`, appends `num_instances` instances into the `pb`.
pub fn populate_with_template_instances<F: Fn(&ChronicleTemplate) -> Option<u32>>(
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
            let instance = instantiate(template, origin, Lit::TRUE, pb)?;
            pb.chronicles.push(instance);
        }
    }
    Ok(())
}

/// Instantiates a chronicle template into a new chronicle instance.
/// Variables are replaced with new ones, declared to the `pb`.
/// The resulting instance is given the origin passed as parameter.
pub fn instantiate(
    template: &ChronicleTemplate,
    origin: ChronicleOrigin,
    scope: Lit,
    pb: &mut FiniteProblem,
) -> Result<ChronicleInstance, InvalidSubstitution> {
    debug_assert!(
        template
            .parameters
            .iter()
            .map(|v| VarRef::from(*v))
            .any(|x| x == template.chronicle.presence.variable()),
        "presence var not in parameters."
    );

    let lbl_of_new = |v: Variable, model: &Model| format!("{}{}", origin.prefix(), model.fmt(v));

    let mut sub = Sub::empty();

    let prez_template = template
        .parameters
        .iter()
        .find(|&x| VarRef::from(*x) == template.chronicle.presence.variable())
        .copied()
        .expect("Presence variable not in parameters");
    // the presence variable is in placed in the containing scope.
    // thus it can only be true if the containing scope is true as well
    let prez_instance = pb
        .model
        .new_presence_variable(scope, lbl_of_new(prez_template, &pb.model));

    sub.add(prez_template, prez_instance.into())?;

    // the literal that indicates the presence of the chronicle we are building
    let prez_lit = sub.sub_bound(template.chronicle.presence);

    for &v in &template.parameters {
        if sub.contains(v) {
            // we already add this variable, ignore it
            continue;
        }
        let label = lbl_of_new(v, &pb.model);
        let fresh: Variable = match v {
            Variable::Bool(_) => pb.model.new_optional_bvar(prez_lit, label).into(),
            Variable::Int(i) => {
                let (lb, ub) = pb.model.domain_of(i);
                pb.model.new_optional_ivar(lb, ub, prez_lit, label).into()
            }
            Variable::Sym(s) => pb.model.new_optional_sym_var(s.tpe, prez_lit, label).into(),
        };
        sub.add(v, fresh)?;
    }

    template.instantiate(sub, origin)
}

pub fn populate_with_task_network(pb: &mut FiniteProblem, spec: &Problem, max_depth: u32) -> Result<()> {
    struct Subtask {
        task: Task,
        instance_id: usize,
        task_id: usize,
        /// presence literal of the scope in which the task occurs
        scope: Lit,
    }
    let mut subtasks = Vec::new();
    for (instance_id, ch) in pb.chronicles.iter().enumerate() {
        for (task_id, task) in ch.chronicle.subtasks.iter().enumerate() {
            let task = &task.task;
            subtasks.push(Subtask {
                task: task.clone(),
                instance_id,
                task_id,
                scope: ch.chronicle.presence,
            });
        }
    }
    for depth in 0..max_depth {
        let mut new_subtasks = Vec::new();
        for task in &subtasks {
            // TODO: if a task has a unique refinement, we should not create new variables for it.
            //       also, new variables should inherit the domain of the tasks
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
                let instance = instantiate(template, origin, task.scope, pb)?;
                let instance_id = pb.chronicles.len();
                pb.chronicles.push(instance);
                // record all subtasks of this chronicle so taht we can process them on the next iteration
                for (task_id, subtask) in pb.chronicles[instance_id].chronicle.subtasks.iter().enumerate() {
                    let task = &subtask.task;
                    new_subtasks.push(Subtask {
                        task: task.clone(),
                        instance_id,
                        task_id,
                        scope: pb.chronicles[instance_id].chronicle.presence,
                    });
                }
            }
        }
        subtasks = new_subtasks;
    }
    Ok(())
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
    let mut clause: Vec<BAtom> = Vec::with_capacity(supporters.len() + 1);
    clause.push((!t.presence).into());
    for s in &supporters {
        clause.push(s.presence.into());
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
        assert!(model
            .discrete
            .domains
            .only_present_with(s.presence.variable(), t.presence.variable()));
        constraints.push(model.implies(s.presence, t.presence)); // TODO: can we get rid of this

        constraints.push(model.opt_eq(s.start, t.start));
        constraints.push(model.opt_eq(s.end, t.end));
        assert_eq!(s.task.len(), t.task.len());
        for (a, b) in s.task.iter().zip(t.task.iter()) {
            constraints.push(model.opt_eq(*a, *b))
        }
    }
}

fn add_symmetry_breaking(
    pb: &FiniteProblem,
    model: &mut Model,
    constraints: &mut Vec<BAtom>,
    tpe: SymmetryBreakingType,
) {
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
}

pub fn encode(pb: &FiniteProblem) -> anyhow::Result<(Model, Vec<BAtom>)> {
    let mut model = pb.model.clone();
    let symmetry_breaking_tpe = SYMMETRY_BREAKING.get();

    // the set of constraints that should be enforced
    let mut constraints: Vec<BAtom> = Vec::new();

    let effs: Vec<_> = effects(pb).collect();
    let conds: Vec<_> = conditions(pb).collect();
    let eff_ends: Vec<_> = effs.iter().map(|_| model.new_ivar(ORIGIN, HORIZON, "")).collect();

    // for each condition, make sure the end is after the start
    for &(_prez_cond, cond) in &conds {
        constraints.push(model.leq(cond.start, cond.end));
    }

    // for each effect, make sure the three time points are ordered
    for ieff in 0..effs.len() {
        let (_prez_eff, eff) = effs[ieff];
        constraints.push(model.leq(eff.persistence_start, eff_ends[ieff]));
        constraints.push(model.leq(eff.transition_start, eff.persistence_start))
    }

    // are two state variables unifiable?
    let unifiable_sv = |model: &Model, sv1: &Sv, sv2: &Sv| {
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
    let mut clause: Vec<BAtom> = Vec::with_capacity(32);
    for (i, &(p1, e1)) in effs.iter().enumerate() {
        for j in i + 1..effs.len() {
            let &(p2, e2) = &effs[j];

            // skip if they are trivially non-overlapping
            if !unifiable_sv(&model, &e1.state_var, &e2.state_var) {
                continue;
            }

            clause.clear();
            clause.push((!p1).into());
            clause.push((!p2).into());
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
        let mut supported: Vec<BAtom> = Vec::with_capacity(128);
        // no need to support if the condition is not present
        supported.push((!prez_cond).into());

        for (eff_id, &(prez_eff, eff)) in effs.iter().enumerate() {
            // quick check that the condition and effect are not trivially incompatible
            if !unifiable_sv(&model, &cond.state_var, &eff.state_var) {
                continue;
            }
            if !model.unifiable(cond.value, eff.value) {
                continue;
            }
            // vector to store the AND clause
            let mut supported_by_eff_conjunction: Vec<BAtom> = Vec::with_capacity(32);
            // support only possible if the effect is present
            supported_by_eff_conjunction.push(prez_eff.into());

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
                    let mut supported_by_a_line: Vec<BAtom> = Vec::with_capacity(256);
                    supported_by_a_line.push((!instance.chronicle.presence).into());
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
                ConstraintType::Lt => match constraint.variables.as_slice() {
                    &[a, b] => {
                        let a: IAtom = a.try_into()?;
                        let b: IAtom = b.try_into()?;
                        constraints.push(model.lt(a, b))
                    }
                    x => bail!("Invalid variable pattern for LT constraint: {:?}", x),
                },
                ConstraintType::Eq => {
                    if constraint.variables.len() != 2 {
                        bail!(
                            "Wrong number of parameters to equality constraint: {}",
                            constraint.variables.len()
                        );
                    }
                    constraints.push(model.eq(constraint.variables[0], constraint.variables[1]));
                }
                ConstraintType::Neq => {
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
            let conj = vec![
                model.leq(subtask.start, subtask.end),
                model.leq(ch.chronicle.start, subtask.start),
                model.leq(subtask.end, ch.chronicle.end),
            ];
            let conj = model.and(&conj);
            // constraints.push(conj);
            constraints.push(model.implies(ch.chronicle.presence, conj));
        }
    }
    add_decomposition_constraints(pb, &mut model, &mut constraints);
    add_symmetry_breaking(pb, &mut model, &mut constraints, symmetry_breaking_tpe);

    Ok((model, constraints))
}
