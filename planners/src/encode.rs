//! Functions whose purpose is to encode a planning problem (represented with chronicles)
//! into a combinatorial problem from Aries core.

use crate::encoding::{conditions, effects, refinements_of, refinements_of_task, TaskRef, HORIZON, ORIGIN};
use crate::Model;
use anyhow::*;
use aries_model::extensions::{AssignmentExt, Shaped};
use aries_model::lang::expr::*;
use aries_model::lang::VarRef;
use aries_model::lang::{IAtom, Variable};
use aries_model::literals::Lit;
use aries_planning::chronicles::constraints::ConstraintType;
use aries_planning::chronicles::*;
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
            let instance_id = pb.chronicles.len();
            let instance = instantiate(instance_id, template, origin, Lit::TRUE, Sub::empty(), pb)?;
            pb.chronicles.push(instance);
        }
    }
    Ok(())
}

/// Instantiates a chronicle template into a new chronicle instance.
/// For each template parameter, if `sub` does not already provide a valid substitution
/// Variables are replaced with new ones, declared to the `pb`.
///
/// # Arguments
///
/// - `instance_id`: ID of the chronicle to be created.
/// - `template`: Chronicle template that must be instantiated.
/// - `origin`: Metadata on the origin of this instantiation that will be added
/// - `scope`: scope in which the chronicle appears. Will be used as the scope of the presence variable
/// - `sub`: partial substitution, only parameters that do not already have a substitution will provoke the creation of a new variable.
/// - `pb`: problem description in which the variables will be created.
pub fn instantiate(
    instance_id: usize,
    template: &ChronicleTemplate,
    origin: ChronicleOrigin,
    scope: Lit,
    mut sub: Sub,
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

    // creation of a new label, based on the label on the variable `v` that is instantiated
    let lbl_of_new = |v: Variable, model: &Model| model.get_label(v).unwrap().on_instance(instance_id);

    let prez_template = template
        .parameters
        .iter()
        .find(|&x| VarRef::from(*x) == template.chronicle.presence.variable())
        .copied()
        .expect("Presence variable not in parameters");

    if !sub.contains(prez_template) {
        // the presence variable is in placed in the containing scope.
        // thus it can only be true if the containing scope is true as well
        let prez_instance = pb
            .model
            .new_presence_variable(scope, lbl_of_new(prez_template, &pb.model));

        sub.add(prez_template, prez_instance.into())?;
    }

    // the literal that indicates the presence of the chronicle we are building
    let prez_lit = sub.sub_lit(template.chronicle.presence);

    for &v in &template.parameters {
        if sub.contains(v) {
            // we already add this variable, ignore it
            continue;
        }
        let label = lbl_of_new(v, &pb.model);
        let fresh: Variable = match v {
            Variable::Bool(_) => pb.model.new_optional_bvar(prez_lit, label).into(),
            Variable::Int(i) => {
                let (lb, ub) = pb.model.int_bounds(i);
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
        task_name: Task,
        instance_id: usize,
        task_id: usize,
        /// presence literal of the scope in which the task occurs
        scope: Lit,
        start: IAtom,
        end: IAtom,
    }
    let mut subtasks = Vec::new();
    for (instance_id, ch) in pb.chronicles.iter().enumerate() {
        for (task_id, task) in ch.chronicle.subtasks.iter().enumerate() {
            let task_name = &task.task_name;
            subtasks.push(Subtask {
                task_name: task_name.clone(),
                instance_id,
                task_id,
                scope: ch.chronicle.presence,
                start: task.start,
                end: task.end,
            });
        }
    }
    for depth in 0..max_depth {
        if subtasks.is_empty() {
            break; // reached bottom of the hierarchy
        }
        let mut new_subtasks = Vec::new();
        for task in &subtasks {
            // TODO: new variables should inherit the domain of the tasks
            let refinements = refinements_of_task(&task.task_name, pb, spec);
            for &template in &refinements {
                if depth == max_depth - 1 && !template.chronicle.subtasks.is_empty() {
                    // this chronicle has subtasks that cannot be achieved since they would require
                    // an higher decomposition depth
                    continue;
                }
                let origin = ChronicleOrigin::Refinement {
                    instance_id: task.instance_id,
                    task_id: task.task_id,
                };
                // partial substitution of the templates parameters.
                let mut sub = Sub::empty();

                if refinements.len() == 1 {
                    // Attempt to minimize the number of created variables (purely optional).
                    // The current subtask has only one possible refinement: this `template`
                    // if the task is present, this refinement must be with exactly the same parameters
                    // We can thus unify the presence, start, end and parameters of   subtask/task pair.
                    // Unification is a best effort and might not succeed due to syntactical difference.
                    // We ignore any failed unification and let normal instantiation run its course.
                    let _ = sub.add_bool_expr_unification(template.chronicle.presence, task.scope);
                    let _ = sub.add_int_expr_unification(template.chronicle.start, task.start);
                    let _ = sub.add_int_expr_unification(template.chronicle.end, task.end);

                    let template_task_name = template.chronicle.task.as_ref().unwrap();
                    #[allow(clippy::needless_range_loop)]
                    for i in 0..template_task_name.len() {
                        let _ = sub.add_sym_expr_unification(template_task_name[i], task.task_name[i]);
                    }
                }

                // complete the instantiation of the template by creating new variables
                let instance_id = pb.chronicles.len();
                let instance = instantiate(instance_id, template, origin, task.scope, sub, pb)?;
                pb.chronicles.push(instance);

                // record all subtasks of this chronicle so that we can process them on the next iteration
                for (task_id, subtask) in pb.chronicles[instance_id].chronicle.subtasks.iter().enumerate() {
                    let task = &subtask.task_name;
                    new_subtasks.push(Subtask {
                        task_name: task.clone(),
                        instance_id,
                        task_id,
                        scope: pb.chronicles[instance_id].chronicle.presence,
                        start: subtask.start,
                        end: subtask.end,
                    });
                }
            }
        }
        subtasks = new_subtasks;
    }
    Ok(())
}

fn add_decomposition_constraints(pb: &FiniteProblem, model: &mut Model) {
    for (instance_id, chronicle) in pb.chronicles.iter().enumerate() {
        for (task_id, task) in chronicle.chronicle.subtasks.iter().enumerate() {
            let subtask = TaskRef {
                presence: chronicle.chronicle.presence,
                start: task.start,
                end: task.end,
                task: &task.task_name,
            };
            let refiners = refinements_of(instance_id, task_id, pb);
            enforce_refinement(subtask, refiners, model);
        }
    }
}

fn enforce_refinement(t: TaskRef, supporters: Vec<TaskRef>, model: &mut Model) {
    // if t is present then at least one supporter is present
    let mut clause: Vec<Lit> = Vec::with_capacity(supporters.len() + 1);
    clause.push(!t.presence);
    for s in &supporters {
        clause.push(s.presence);
    }
    model.enforce(or(clause));

    // if a supporter is present, then all others are absent
    for (i, s1) in supporters.iter().enumerate() {
        for (j, s2) in supporters.iter().enumerate() {
            if i != j {
                model.enforce(implies(s1.presence, !s2.presence));
            }
        }
    }

    // if a supporter is present, then all its parameters are unified with the ones of the supported task
    for s in &supporters {
        // if the supporter is present, the supported is as well
        assert!(model
            .state
            .only_present_with(s.presence.variable(), t.presence.variable()));
        model.enforce(implies(s.presence, t.presence)); // TODO: can we get rid of this

        model.enforce(opt_eq(s.start, t.start));
        model.enforce(opt_eq(s.end, t.end));
        assert_eq!(s.task.len(), t.task.len());
        for (a, b) in s.task.iter().zip(t.task.iter()) {
            model.enforce(opt_eq(*a, *b))
        }
    }
}

fn add_symmetry_breaking(pb: &FiniteProblem, model: &mut Model, tpe: SymmetryBreakingType) {
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
                        model.enforce(implies(instance1.chronicle.presence, instance2.chronicle.presence));
                        model.enforce(leq(instance1.chronicle.start, instance2.chronicle.start));
                    }
                }
            }
        }
    };
}

pub fn encode(pb: &FiniteProblem) -> anyhow::Result<Model> {
    let mut model = pb.model.clone();
    let symmetry_breaking_tpe = SYMMETRY_BREAKING.get();

    let effs: Vec<_> = effects(pb).collect();
    let conds: Vec<_> = conditions(pb).collect();
    let eff_ends: Vec<_> = effs
        .iter()
        .map(|(instance_id, prez, _)| {
            model.new_optional_ivar(
                ORIGIN,
                HORIZON,
                *prez,
                Container::Instance(*instance_id) / VarType::EffectEnd,
            )
        })
        .collect();

    // for each condition, make sure the end is after the start
    for &(_prez_cond, cond) in &conds {
        model.enforce(leq(cond.start, cond.end));
    }

    // for each effect, make sure the three time points are ordered
    for ieff in 0..effs.len() {
        let (_, _, eff) = effs[ieff];
        model.enforce(leq(eff.persistence_start, eff_ends[ieff]));
        model.enforce(leq(eff.transition_start, eff.persistence_start));
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
    let mut clause: Vec<Lit> = Vec::with_capacity(32);
    for (i, &(_, p1, e1)) in effs.iter().enumerate() {
        for j in i + 1..effs.len() {
            let &(_, p2, e2) = &effs[j];

            // skip if they are trivially non-overlapping
            if !unifiable_sv(&model, &e1.state_var, &e2.state_var) {
                continue;
            }

            clause.clear();
            clause.push(!p1);
            clause.push(!p2);
            assert_eq!(e1.state_var.len(), e2.state_var.len());
            for idx in 0..e1.state_var.len() {
                let a = e1.state_var[idx];
                let b = e2.state_var[idx];
                // enforce different : a < b || a > b
                // if they are the same variable, there is nothing we can do to separate them
                if a != b {
                    clause.push(model.reify(neq(a, b)));
                }
            }

            clause.push(model.reify(leq(eff_ends[j], e1.transition_start)));
            clause.push(model.reify(leq(eff_ends[i], e2.transition_start)));

            // add coherence constraint
            model.enforce(or(clause.as_slice()));
        }
    }

    // support constraints
    for (_cond_id, &(prez_cond, cond)) in conds.iter().enumerate() {
        let mut supported: Vec<Lit> = Vec::with_capacity(128);
        // no need to support if the condition is not present
        supported.push(!prez_cond);

        for (eff_id, &(_, prez_eff, eff)) in effs.iter().enumerate() {
            // quick check that the condition and effect are not trivially incompatible
            if !unifiable_sv(&model, &cond.state_var, &eff.state_var) {
                continue;
            }
            if !model.unifiable(cond.value, eff.value) {
                continue;
            }
            // vector to store the AND clause
            let mut supported_by_eff_conjunction: Vec<Lit> = Vec::with_capacity(32);
            // support only possible if the effect is present
            supported_by_eff_conjunction.push(prez_eff);

            assert_eq!(cond.state_var.len(), eff.state_var.len());
            // same state variable
            for idx in 0..cond.state_var.len() {
                let a = cond.state_var[idx];
                let b = eff.state_var[idx];

                supported_by_eff_conjunction.push(model.reify(eq(a, b)));
            }
            // same value
            let condition_value = cond.value;
            let effect_value = eff.value;
            supported_by_eff_conjunction.push(model.reify(eq(condition_value, effect_value)));

            // effect's persistence contains condition
            supported_by_eff_conjunction.push(model.reify(leq(eff.persistence_start, cond.start)));
            supported_by_eff_conjunction.push(model.reify(leq(cond.end, eff_ends[eff_id])));

            let support_lit = model.reify(and(supported_by_eff_conjunction));
            debug_assert!({
                let prez_support = model.presence_literal(support_lit.variable());
                model.state.implies(prez_cond, prez_support)
            });

            // add this support expression to the support clause
            supported.push(support_lit);
        }

        debug_assert!({
            let or_reif = model.reify(or(supported.as_slice()));
            let or_reif_prez = model.presence_literal(or_reif.variable());
            or_reif_prez == Lit::TRUE
        });

        // enforce necessary conditions for condition' support
        model.enforce(or(supported));
    }

    // chronicle constraints
    for instance in &pb.chronicles {
        for constraint in &instance.chronicle.constraints {
            match constraint.tpe {
                ConstraintType::InTable { table_id } => {
                    let mut supported_by_a_line: Vec<Lit> = Vec::with_capacity(256);
                    supported_by_a_line.push(!instance.chronicle.presence);
                    let vars = &constraint.variables;
                    for values in pb.tables[table_id as usize].lines() {
                        assert_eq!(vars.len(), values.len());
                        let mut supported_by_this_line = Vec::with_capacity(16);
                        for (&var, &val) in vars.iter().zip(values.iter()) {
                            let var = var.int_view().unwrap();
                            supported_by_this_line.push(model.reify(leq(var, val)));
                            supported_by_this_line.push(model.reify(geq(var, val)));
                        }
                        supported_by_a_line.push(model.reify(and(supported_by_this_line)));
                    }
                    model.enforce(or(supported_by_a_line));
                }
                ConstraintType::Lt => match constraint.variables.as_slice() {
                    &[a, b] => {
                        let a: IAtom = a.try_into()?;
                        let b: IAtom = b.try_into()?;
                        model.enforce(lt(a, b));
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
                    model.enforce(eq(constraint.variables[0], constraint.variables[1]));
                }
                ConstraintType::Neq => {
                    if constraint.variables.len() != 2 {
                        bail!(
                            "Wrong number of parameters to inequality constraint: {}",
                            constraint.variables.len()
                        );
                    }
                    model.enforce(neq(constraint.variables[0], constraint.variables[1]));
                }
            }
        }
    }

    for ch in &pb.chronicles {
        // chronicle finishes before the horizon and has a non negative duration
        model.enforce(opt_leq(ch.chronicle.end, pb.horizon));
        model.enforce(opt_leq(ch.chronicle.start, ch.chronicle.end));

        // enforce temporal coherence between the chronicle and its subtasks
        for subtask in &ch.chronicle.subtasks {
            model.enforce(opt_leq(subtask.start, subtask.end));
            model.enforce(opt_leq(ch.chronicle.start, subtask.start));
            model.enforce(opt_leq(subtask.end, ch.chronicle.end));
        }
    }
    add_decomposition_constraints(pb, &mut model);
    add_symmetry_breaking(pb, &mut model, symmetry_breaking_tpe);

    Ok(model)
}
