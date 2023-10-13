//! Functions whose purpose is to encode a planning problem (represented with chronicles)
//! into a combinatorial problem from Aries core.

use crate::encoding::*;
use crate::solver::{init_solver, Metric};
use crate::Model;
use anyhow::{Context, Result};
use aries::core::state::Conflict;
use aries::core::*;
use aries::model::extensions::{AssignmentExt, Shaped};
use aries::model::lang::linear::{LinearSum, LinearTerm};
use aries::model::lang::{expr::*, Kind, Type};
use aries::model::lang::{Atom, FAtom, FVar, IAtom, Variable};
use aries::solver::Solver;
use aries_planning::chronicles::constraints::{ConstraintType, Duration};
use aries_planning::chronicles::*;
use env_param::EnvParam;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::ptr;

/// Parameter that defines the symmetry breaking strategy to use.
/// The value of this parameter is loaded from the environment variable `ARIES_LCP_SYMMETRY_BREAKING`.
/// Possible values are `none` and `simple` (default).
pub static SYMMETRY_BREAKING: EnvParam<SymmetryBreakingType> = EnvParam::new("ARIES_LCP_SYMMETRY_BREAKING", "simple");

/// Parameter that activates the temporal relaxation of temporal constraints of a task's
/// interval and the its methods intervals. The temporal relaxation can be used when
/// using an acting system to allow the interval of a method to be included in the interval
/// of the task it refined,without constraining the equality of the start and end timepoints
/// of both intervals. The parameter is loaded from the environment variable
/// ARIES_LCP_RELAXED_TEMPORAL_CONSTRAINT_TASK_METHOD, and is set to *false* as default.
pub static RELAXED_TEMPORAL_CONSTRAINT: EnvParam<bool> =
    EnvParam::new("ARIES_LCP_RELAXED_TEMPORAL_CONSTRAINT_TASK_METHOD", "false");

impl std::str::FromStr for SymmetryBreakingType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(SymmetryBreakingType::None),
            "simple" => Ok(SymmetryBreakingType::Simple),
            x => Err(format!("Unknown symmetry breaking type: {x}")),
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
    let default_label = VarLabel(Container::Base, VarType::Parameter("?".to_string()));
    let lbl_of_new = |v: Variable, model: &Model| {
        model
            .get_label(v)
            .unwrap_or_else(|| {
                tracing::warn!("Chronicle parameter with no label.");
                &default_label
            })
            .on_instance(instance_id)
    };

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
            Variable::Fixed(f) => {
                let (lb, ub) = pb.model.int_bounds(f.num);
                pb.model.new_optional_fvar(lb, ub, f.denom, prez_lit, label).into()
            }
            Variable::Sym(s) => pb.model.new_optional_sym_var(s.tpe, prez_lit, label).into(),
        };
        sub.add(v, fresh)?;
    }

    template.instantiate(sub, origin)
}

/// A subtask of chronicle instance
struct Subtask {
    /// task name, including parameters
    task_name: Task,
    /// Index of the chronicle instance that contains the task
    instance_id: usize,
    /// Index of the task in the chronicle's subtask list
    task_id: usize,
    /// presence literal of the scope in which the task occurs
    scope: Lit,
    start: FAtom,
    end: FAtom,
}
impl From<&Subtask> for TaskId {
    fn from(value: &Subtask) -> Self {
        TaskId {
            instance_id: value.instance_id,
            task_id: value.task_id,
        }
    }
}

/// A group of homogeneous and exclusive subtasks that can be decomposed by the same methods/actions
///
/// Consider a task `t` that can be decomposed by two methods `m1` and `m2`
/// each also with a subtask `t`.
/// Note that: the subtasks `m1.t` and `m2.t` are exclusive: they cannot be
/// present together.
///
/// Thus they can be gathered in the same `SubtaskGroup` which will allow us to add a
/// single m1 instance and a single m2 instance for both `m1.t` and `m2.t`.
struct SubtaskGroup {
    /// A scope where all subtasks are present.
    /// DO NOT USE directly. Prefer using the `shared_scope` method that will provide a more specific
    /// answer if, e.g. there is a single task.
    parent_scope: Lit,
    /// A set of homogeneous tasks that can be decomposed by the same methods/actions
    tasks: Vec<Subtask>,
    /// ids of chronicle templates that decompose this task group
    refiners_ids: HashSet<usize>,
}
impl SubtaskGroup {
    /// A scope that is shared by all subtasks: if one of the subtasks is present, then this scope literal is true
    fn shared_scope(&self) -> Lit {
        if self.tasks.len() == 1 {
            self.tasks[0].scope
        } else {
            self.parent_scope
        }
    }
}

pub fn populate_with_task_network(pb: &mut FiniteProblem, spec: &Problem, max_depth: u32) -> Result<()> {
    // the set ob subtasks for which we need to introduce refinements in the current iteration
    let mut subtasks = Vec::new();

    // gather subtasks from existing chronicle instances
    for (instance_id, ch) in pb.chronicles.iter().enumerate() {
        for (task_id, task) in ch.chronicle.subtasks.iter().enumerate() {
            let task_name = &task.task_name;
            let subtask = Subtask {
                task_name: task_name.clone(),
                instance_id,
                task_id,
                scope: ch.chronicle.presence,
                start: task.start,
                end: task.end,
            };
            let refiners_ids = refinements_of_task(&subtask.task_name, pb, spec);
            let group = SubtaskGroup {
                parent_scope: ch.chronicle.presence,
                tasks: vec![subtask],
                refiners_ids,
            };
            subtasks.push(group);
        }
    }
    for depth in 0..max_depth {
        if subtasks.is_empty() {
            break; // reached bottom of the hierarchy
        }
        // subtasks that will need to be added in the next iterations
        let mut new_subtasks = Vec::new();
        for task_group in &subtasks {
            // indirect subtasks of `task`
            let mut local_subtasks: Vec<SubtaskGroup> = Vec::with_capacity(16);

            // Will store the presence variables of all chronicles supporting it the tasks
            let mut refiners_presence_variables: Vec<Lit> = Vec::with_capacity(16);

            let refined: Vec<TaskId> = task_group.tasks.iter().map(TaskId::from).collect();

            for &template_id in &task_group.refiners_ids {
                // instantiate a template of the refiner
                let template = &spec.templates[template_id];

                if depth == max_depth - 1 && !template.chronicle.subtasks.is_empty() {
                    // this chronicle has subtasks that cannot be achieved since they would require
                    // an higher decomposition depth
                    continue;
                }
                let origin = ChronicleOrigin::Refinement {
                    refined: refined.clone(),
                    template_id,
                };

                let mut sub = Sub::empty();
                if task_group.refiners_ids.len() == 1 && task_group.tasks.len() == 1 {
                    // Single chronicle that refines a single task.
                    // Attempt to minimize the number of created variables (purely optional).
                    // The current subtask has only one possible refinement: this `template`
                    // if the task is present, this refinement must be with exactly the same parameters
                    // We can thus unify the presence, start, end and parameters of   subtask/task pair.
                    // Unification is a best effort and might not succeed due to syntactical difference.
                    // We ignore any failed unification and let normal instantiation run its course.
                    let task = &task_group.tasks[0];
                    let _ = sub.add_bool_expr_unification(template.chronicle.presence, task.scope);
                    let _ = sub.add_fixed_expr_unification(template.chronicle.start, task.start);
                    let _ = sub.add_fixed_expr_unification(template.chronicle.end, task.end);

                    let template_task_name = template.chronicle.task.as_ref().unwrap();
                    #[allow(clippy::needless_range_loop)]
                    for i in 0..template_task_name.len() {
                        let _ = sub.add_expr_unification(template_task_name[i], task.task_name[i]);
                    }
                }

                // complete the instantiation of the template by creating new variables
                let instance_id = pb.chronicles.len();
                let instance = instantiate(instance_id, template, origin, task_group.shared_scope(), sub, pb)?;

                // make this method exclusive with all previous methods for the same task
                for &o in &refiners_presence_variables {
                    pb.model.state.add_implication(instance.chronicle.presence, !o);
                    pb.model.state.add_implication(o, !instance.chronicle.presence);
                }
                refiners_presence_variables.push(instance.chronicle.presence);
                pb.chronicles.push(instance);

                // record all subtasks of this chronicle so that we can process them on the next iteration
                // compatible and exclusive subtasks are grouped
                for (task_id, subtask) in pb.chronicles[instance_id].chronicle.subtasks.iter().enumerate() {
                    let task = &subtask.task_name;
                    let sub = Subtask {
                        task_name: task.clone(),
                        instance_id,
                        task_id,
                        scope: pb.chronicles[instance_id].chronicle.presence,
                        start: subtask.start,
                        end: subtask.end,
                    };
                    let refiners = refinements_of_task(&sub.task_name, pb, spec);
                    if let Some(group) = local_subtasks
                        .iter_mut()
                        .find(|g| g.refiners_ids == refiners && g.tasks.iter().all(|t| t.scope != sub.scope))
                    {
                        debug_assert!(group.tasks.iter().all(|t| pb.model.state.exclusive(t.scope, sub.scope)));
                        // the task can be merged into an existing group of of local subtasks
                        group.tasks.push(sub);
                    } else {
                        local_subtasks.push(SubtaskGroup {
                            parent_scope: task_group.shared_scope(),
                            tasks: vec![sub],
                            refiners_ids: refiners,
                        })
                    }
                }
            }

            new_subtasks.extend(local_subtasks);
        }
        subtasks = new_subtasks;
    }
    Ok(())
}

fn add_decomposition_constraints(pb: &FiniteProblem, model: &mut Model, encoding: &mut Encoding) {
    for (instance_id, ch) in pb.chronicles.iter().enumerate() {
        if let ChronicleOrigin::Refinement { refined, .. } = &ch.origin {
            // chronicle is a refinement of some task.
            let refined_tasks: Vec<_> = refined.iter().map(|tid| get_task_ref(pb, *tid)).collect();

            for task in refined {
                encoding.tag(ch.chronicle.presence, Tag::Decomposition(*task, instance_id));
            }

            // prez(ch) => prez(refined[0]) || prez(refined[1]) || ...
            let clause: Vec<Lit> = refined_tasks.iter().map(|t| t.presence).collect();
            if let &[single] = clause.as_slice() {
                model.state.add_implication(ch.chronicle.presence, single);
            } else {
                model.enforce(or(clause), [ch.chronicle.presence]);
            }
        }

        for (task_id, task) in ch.chronicle.subtasks.iter().enumerate() {
            let subtask = TaskRef {
                presence: ch.chronicle.presence,
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
    let clause: Vec<Lit> = supporters.iter().map(|s| s.presence).collect();
    model.enforce(or(clause), [t.presence]);

    // if a supporter is present, then all others are absent
    for (i, s1) in supporters.iter().enumerate() {
        for (j, s2) in supporters.iter().enumerate() {
            if i != j {
                model.enforce(or([!s1.presence, !s2.presence]), [t.presence]);
            }
        }
    }

    // if a supporter is present, then all its parameters are unified with the ones of the supported task
    for s in &supporters {
        if RELAXED_TEMPORAL_CONSTRAINT.get() {
            // Relaxed constraints in the encoding for chronicles coming from an acting system,
            // where the interval of a method is contained in the interval of the task it refines.
            model.enforce(f_leq(t.start, s.start), [s.presence, t.presence]);
            model.enforce(f_leq(s.end, t.end), [s.presence, t.presence]);
        } else {
            model.enforce(eq(s.start, t.start), [s.presence, t.presence]);
            model.enforce(eq(s.end, t.end), [s.presence, t.presence]);
        }

        assert_eq!(s.task.len(), t.task.len());
        for (a, b) in s.task.iter().zip(t.task.iter()) {
            model.enforce(eq(*a, *b), [s.presence, t.presence])
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
                        let p1 = instance1.chronicle.presence;
                        let p2 = instance2.chronicle.presence;
                        model.enforce(implies(p1, p2), []);
                        model.enforce(f_leq(instance1.chronicle.start, instance2.chronicle.start), [p1, p2]);
                    }
                }
            }
        }
    };
}

/// Encode a metric in the problem and returns an integer that should minimized in order to optimize the metric.
pub fn add_metric(pb: &FiniteProblem, model: &mut Model, metric: Metric) -> IAtom {
    match metric {
        Metric::Makespan => pb.horizon.num,
        Metric::PlanLength => {
            // retrieve the presence variable of each action
            let mut action_presence = Vec::with_capacity(8);
            for (ch_id, ch) in pb.chronicles.iter().enumerate() {
                match ch.chronicle.kind {
                    ChronicleKind::Action | ChronicleKind::DurativeAction => {
                        action_presence.push((ch_id, ch.chronicle.presence));
                    }
                    ChronicleKind::Problem | ChronicleKind::Method => {}
                }
            }

            // for each action, create an optional variable that evaluate to 1 if the action is present and 0 otherwise
            let action_costs: Vec<LinearTerm> = action_presence
                .iter()
                .map(|(ch_id, p)| {
                    model
                        .new_optional_ivar(1, 1, *p, Container::Instance(*ch_id).var(VarType::Cost))
                        .or_zero(*p)
                })
                .collect();
            let action_costs = LinearSum::of(action_costs);

            // make the sum of the action costs equal a `plan_length` variable.
            let plan_length = model.new_ivar(0, INT_CST_MAX, VarLabel(Container::Base, VarType::Cost));
            model.enforce(action_costs.clone().leq(plan_length), []);
            model.enforce(action_costs.geq(plan_length), []);
            // plan length is the metric that should be minimized.
            plan_length.into()
        }
        Metric::ActionCosts => {
            // retrieve the presence and cost of each chronicle
            let mut costs = Vec::with_capacity(8);
            for (ch_id, ch) in pb.chronicles.iter().enumerate() {
                if let Some(cost) = ch.chronicle.cost {
                    assert!(cost >= 0, "A chronicle has a negative cost");
                    costs.push((ch_id, ch.chronicle.presence, cost));
                }
            }

            // for each action, create an optional variable that evaluate to the cost if the action is present and 0 otherwise
            let action_costs: Vec<LinearTerm> = costs
                .iter()
                .map(|&(ch_id, p, cost)| {
                    model
                        .new_optional_ivar(cost, cost, p, Container::Instance(ch_id).var(VarType::Cost))
                        .or_zero(p)
                })
                .collect();
            let action_costs = LinearSum::of(action_costs);

            // make the sum of the action costs equal a `plan_cost` variable.
            let plan_cost = model.new_ivar(0, INT_CST_MAX, VarLabel(Container::Base, VarType::Cost));
            model.enforce(action_costs.clone().leq(plan_cost), []);
            model.enforce(action_costs.geq(plan_cost), []);
            // plan cost is the metric that should be minimized.
            plan_cost.into()
        }
    }
}

pub struct EncodedProblem {
    pub model: Model,
    pub objective: Option<IAtom>,
    /// Metadata associated to variables and literals in the encoded problem.
    pub encoding: Encoding,
}

/// Encodes a finite problem.
/// If a metric is given, it will return along with the model an `IAtom` that should be minimized
/// Returns an error if the encoded problem is found to be unsatisfiable.
pub fn encode(pb: &FiniteProblem, metric: Option<Metric>) -> std::result::Result<EncodedProblem, Conflict> {
    let mut encoding = Encoding::default();
    let encode_span = tracing::span!(tracing::Level::DEBUG, "ENCODING");
    let _x = encode_span.enter();

    // build a model and put it inside a solver to allow for eager propagation.
    let model = pb.model.clone();
    let mut solver = init_solver(model);

    let symmetry_breaking_tpe = SYMMETRY_BREAKING.get();

    // is the state variable numeric?
    let is_integer = |sv: &StateVar| matches!(sv.fluent.return_type().into(), Kind::Int);

    let effs: Vec<_> = effects(pb).collect();
    let conds: Vec<_> = conditions(pb).collect();
    let eff_persis_ends: HashMap<EffID, FVar> = effs
        .iter()
        .map(|(eff_id, prez, _)| {
            let var = solver.model.new_optional_fvar(
                ORIGIN * TIME_SCALE.get(),
                HORIZON * TIME_SCALE.get(),
                TIME_SCALE.get(),
                *prez,
                Container::Instance(eff_id.instance_id) / VarType::EffectEnd,
            );
            (*eff_id, var)
        })
        .collect();
    let eff_assign_ends: HashMap<EffID, FVar> = effs
        .iter()
        .filter_map(|(eff_id, prez, eff)| {
            if !solver.model.entails(!*prez)
                && matches!(eff.operation, EffectOp::Assign(_))
                && is_integer(&eff.state_var)
            {
                // Those time points are only present for numeric assignment effects
                let var = solver.model.new_optional_fvar(
                    ORIGIN * TIME_SCALE.get(),
                    HORIZON * TIME_SCALE.get(),
                    TIME_SCALE.get(),
                    *prez,
                    Container::Instance(eff_id.instance_id) / VarType::EffectEnd,
                );
                return Some((*eff_id, var));
            }
            None
        })
        .collect();

    tracing::debug!("#chronicles: {}", pb.chronicles.len());
    tracing::debug!("#effects: {}", effs.len());
    tracing::debug!("#conditions: {}", conds.len());

    // for each condition, make sure the end is after the start
    for &(_, prez_cond, cond) in &conds {
        solver.enforce(f_leq(cond.start, cond.end), [prez_cond]);
    }

    solver.propagate()?;

    {
        let span = tracing::span!(tracing::Level::TRACE, "structural constraints");
        let _span = span.enter();
        // chronicle constraints
        for instance in &pb.chronicles {
            let prez = instance.chronicle.presence;
            for constraint in &instance.chronicle.constraints {
                let value = match constraint.value {
                    // work around some dubious encoding of chronicle. The given value should have the appropriate scope
                    Some(Lit::TRUE) | None => solver.model.get_tautology_of_scope(prez),
                    Some(Lit::FALSE) => !solver.model.get_tautology_of_scope(prez),
                    Some(l) => l,
                };
                match &constraint.tpe {
                    ConstraintType::InTable(table) => {
                        let mut supported_by_a_line: Vec<Lit> = Vec::with_capacity(256);

                        let vars = &constraint.variables;
                        for values in table.lines() {
                            assert_eq!(vars.len(), values.len());
                            let mut supported_by_this_line = Vec::with_capacity(16);
                            for (&var, &val) in vars.iter().zip(values.iter()) {
                                let var = var.int_view().unwrap();
                                supported_by_this_line.push(solver.reify(leq(var, val)));
                                supported_by_this_line.push(solver.reify(geq(var, val)));
                            }
                            supported_by_a_line.push(solver.reify(and(supported_by_this_line)));
                        }
                        assert!(solver.model.entails(value)); // tricky to determine the appropriate validity scope, only support enforcing
                        solver.enforce(or(supported_by_a_line), [prez]);
                    }
                    ConstraintType::Lt => match constraint.variables.as_slice() {
                        &[a, b] => match (a, b) {
                            (Atom::Int(a), Atom::Int(b)) => solver.model.bind(lt(a, b), value),
                            (Atom::Fixed(a), Atom::Fixed(b)) if a.denom == b.denom => {
                                solver.model.bind(f_lt(a, b), value)
                            }
                            (Atom::Fixed(a), Atom::Int(b)) => {
                                let a = LinearSum::from(a + FAtom::EPSILON);
                                let b = LinearSum::from(b);
                                solver.model.bind(a.leq(b), value);
                            }
                            (Atom::Int(a), Atom::Fixed(b)) => {
                                let a = LinearSum::from(a);
                                let b = LinearSum::from(b - FAtom::EPSILON);
                                solver.model.bind(a.leq(b), value);
                            }
                            _ => panic!("Invalid LT operands: {a:?}  {b:?}"),
                        },
                        x => panic!("Invalid variable pattern for LT constraint: {:?}", x),
                    },
                    ConstraintType::Leq => match constraint.variables.as_slice() {
                        &[a, b] => match (a, b) {
                            (Atom::Int(a), Atom::Int(b)) => solver.model.bind(leq(a, b), value),
                            (Atom::Fixed(a), Atom::Fixed(b)) if a.denom == b.denom => {
                                solver.model.bind(f_leq(a, b), value)
                            }
                            (Atom::Fixed(a), Atom::Int(b)) => {
                                let a = LinearSum::from(a);
                                let b = LinearSum::from(b);
                                solver.model.bind(a.leq(b), value);
                            }
                            (Atom::Int(a), Atom::Fixed(b)) => {
                                let a = LinearSum::from(a);
                                let b = LinearSum::from(b);
                                solver.model.bind(a.leq(b), value);
                            }
                            _ => panic!("Invalid LEQ operands: {a:?}  {b:?}"),
                        },
                        x => panic!("Invalid variable pattern for LEQ constraint: {:?}", x),
                    },
                    ConstraintType::Eq => {
                        assert_eq!(
                            constraint.variables.len(),
                            2,
                            "Wrong number of parameters to equality constraint: {}",
                            constraint.variables.len()
                        );
                        solver
                            .model
                            .bind(eq(constraint.variables[0], constraint.variables[1]), value);
                    }
                    ConstraintType::Neq => {
                        assert_eq!(
                            constraint.variables.len(),
                            2,
                            "Wrong number of parameters to inequality constraint: {}",
                            constraint.variables.len()
                        );

                        solver
                            .model
                            .bind(neq(constraint.variables[0], constraint.variables[1]), value);
                    }
                    ConstraintType::Duration(dur) => {
                        let build_sum =
                            |s: LinearSum, e: LinearSum, d: &LinearSum| LinearSum::of(vec![-s, e]) - d.clone();

                        let start = LinearSum::from(instance.chronicle.start);
                        let end = LinearSum::from(instance.chronicle.end);

                        match dur {
                            Duration::Fixed(d) => {
                                let sum = build_sum(start, end, d);
                                solver.model.bind(sum.clone().leq(LinearSum::zero()), value);
                                solver.model.bind(sum.geq(LinearSum::zero()), value);
                            }
                            Duration::Bounded { lb, ub } => {
                                let lb_sum = build_sum(start.clone(), end.clone(), lb);
                                let ub_sum = build_sum(start, end, ub);
                                solver.model.bind(lb_sum.geq(LinearSum::zero()), value);
                                solver.model.bind(ub_sum.leq(LinearSum::zero()), value);
                            }
                        };
                        // Redundant constraint to enforce the precedence between start and end.
                        // This form ensures that the precedence in posted in the STN.
                        solver.enforce(
                            f_leq(instance.chronicle.start, instance.chronicle.end),
                            [instance.chronicle.presence],
                        )
                    }
                    ConstraintType::Or => {
                        let mut disjuncts = Vec::with_capacity(constraint.variables.len());
                        for v in &constraint.variables {
                            let disjunct: Lit = Lit::try_from(*v).expect("Malformed or constraint");
                            disjuncts.push(disjunct);
                        }
                        solver.model.bind(or(disjuncts), value)
                    }
                    ConstraintType::LinearEq(sum) => {
                        solver
                            .model
                            .enforce(sum.clone().leq(LinearSum::zero()), [instance.chronicle.presence]);
                        solver
                            .model
                            .enforce(sum.clone().geq(LinearSum::zero()), [instance.chronicle.presence]);
                    }
                }
            }
        }

        for ch in &pb.chronicles {
            let prez = ch.chronicle.presence;
            // chronicle finishes before the horizon and has a non negative duration
            if matches!(ch.chronicle.kind, ChronicleKind::Action | ChronicleKind::DurativeAction) {
                solver.enforce(f_lt(ch.chronicle.end, pb.horizon), [prez]);
            }
            solver.enforce(f_leq(ch.chronicle.start, ch.chronicle.end), [prez]);

            // enforce temporal coherence between the chronicle and its subtasks
            for subtask in &ch.chronicle.subtasks {
                solver.enforce(f_leq(subtask.start, subtask.end), [prez]);
                solver.enforce(f_leq(ch.chronicle.start, subtask.start), [prez]);
                solver.enforce(f_leq(subtask.end, ch.chronicle.end), [prez]);
            }
        }
        add_decomposition_constraints(pb, &mut solver.model, &mut encoding);
        add_symmetry_breaking(pb, &mut solver.model, symmetry_breaking_tpe);
        solver.propagate()?;
    }

    let mut num_removed_chronicles = 0;
    for ch in &pb.chronicles {
        let prez = ch.chronicle.presence;
        if solver.model.entails(!prez) {
            num_removed_chronicles += 1;
        }
    }
    tracing::debug!("Chronicles removed by eager propagation: {}", num_removed_chronicles);

    // for each effect, make sure the three (or four for numeric assignments) time points are ordered
    for &(eff_id, prez_eff, eff) in &effs {
        let persistence_end = eff_persis_ends[&eff_id];
        solver.enforce(f_leq(eff.persistence_start, persistence_end), [prez_eff]);
        solver.enforce(f_leq(eff.transition_start, eff.persistence_start), [prez_eff]);
        for &min_persistence_end in &eff.min_persistence_end {
            solver.enforce(f_leq(min_persistence_end, persistence_end), [prez_eff])
        }
        if eff_assign_ends.contains_key(&eff_id) {
            let assignment_end = eff_assign_ends[&eff_id];
            solver.enforce(f_leq(persistence_end, assignment_end), [prez_eff]);
        }
    }

    solver.propagate()?;

    // are two state variables unifiable?
    let unifiable_sv = |model: &Model, sv1: &StateVar, sv2: &StateVar| {
        sv1.fluent == sv2.fluent && model.unifiable_seq(&sv1.args, &sv2.args)
    };

    {
        // coherence constraints
        let span = tracing::span!(tracing::Level::TRACE, "coherence");
        let _span = span.enter();
        let mut num_coherence_constraints = 0;
        // for each pair of effects, enforce coherence constraints
        // except between two increases, they can be done in parallel
        let mut clause: Vec<Lit> = Vec::with_capacity(32);
        for &(i, p1, e1) in &effs {
            if solver.model.entails(!p1) {
                continue;
            }
            for &(j, p2, e2) in &effs {
                if i >= j {
                    continue;
                }
                if solver.model.entails(!p2) || solver.model.state.exclusive(p1, p2) {
                    continue;
                }

                // skip if they are trivially non-overlapping
                if !unifiable_sv(&solver.model, &e1.state_var, &e2.state_var) {
                    continue;
                }

                // skip if it's two increases
                if matches!(e1.operation, EffectOp::Increase(_) | EffectOp::Decrease(_))
                    && matches!(e2.operation, EffectOp::Increase(_) | EffectOp::Decrease(_))
                {
                    continue;
                }

                clause.clear();
                debug_assert_eq!(e1.state_var.fluent, e2.state_var.fluent);
                for idx in 0..e1.state_var.args.len() {
                    let a = e1.state_var.args[idx];
                    let b = e2.state_var.args[idx];
                    // enforce different : a < b || a > b
                    // if they are the same variable, there is nothing we can do to separate them
                    if a != b {
                        clause.push(solver.reify(neq(a, b)));
                    }
                }

                // For two numeric assignments, force [transition_start, assignment_end] to not overlaps.
                // Otherwise, force [transition_start, persistence_end] to not overlaps.
                if eff_assign_ends.contains_key(&i) && eff_assign_ends.contains_key(&j) {
                    clause.push(solver.reify(f_leq(eff_assign_ends[&j], e1.transition_start)));
                    clause.push(solver.reify(f_leq(eff_assign_ends[&i], e2.transition_start)));
                } else {
                    clause.push(solver.reify(f_leq(eff_persis_ends[&j], e1.transition_start)));
                    clause.push(solver.reify(f_leq(eff_persis_ends[&i], e2.transition_start)));
                }

                // add coherence constraint
                solver.enforce(or(clause.as_slice()), [p1, p2]);
                num_coherence_constraints += 1;
            }
        }
        tracing::debug!(%num_coherence_constraints);

        solver.propagate()?;
    }

    {
        // support constraints
        let span = tracing::span!(tracing::Level::TRACE, "support");
        let _span = span.enter();
        let mut num_support_constraints = 0;

        for &(cond_id, prez_cond, cond) in &conds {
            // skip conditions on numeric state variables, they are supported by resource constraints
            if is_integer(&cond.state_var) {
                continue;
            }
            if solver.model.entails(!prez_cond) {
                continue;
            }
            let mut supported: Vec<Lit> = Vec::with_capacity(128);
            for &(eff_id, prez_eff, eff) in &effs {
                if solver.model.entails(!prez_eff) {
                    continue;
                }
                if solver.model.state.exclusive(prez_cond, prez_eff) {
                    continue;
                }
                // quick check that the condition and effect are not trivially incompatible
                if !unifiable_sv(&solver.model, &cond.state_var, &eff.state_var) {
                    continue;
                }
                let EffectOp::Assign(effect_value) = eff.operation else {
                    unreachable!()
                };
                if !solver.model.unifiable(cond.value, effect_value) {
                    continue;
                }
                // vector to store the AND clause
                let mut supported_by_eff_conjunction: Vec<Lit> = Vec::with_capacity(32);
                // support only possible if the effect is present
                supported_by_eff_conjunction.push(prez_eff);
                debug_assert_eq!(cond.state_var.fluent, eff.state_var.fluent);
                // same state variable
                for idx in 0..cond.state_var.args.len() {
                    let a = cond.state_var.args[idx];
                    let b = eff.state_var.args[idx];

                    supported_by_eff_conjunction.push(solver.reify(eq(a, b)));
                }
                // same value
                let condition_value = cond.value;
                supported_by_eff_conjunction.push(solver.reify(eq(condition_value, effect_value)));

                // effect's persistence contains condition
                supported_by_eff_conjunction.push(solver.reify(f_leq(eff.persistence_start, cond.start)));
                supported_by_eff_conjunction.push(solver.reify(f_leq(cond.end, eff_persis_ends[&eff_id])));
                supported_by_eff_conjunction.push(prez_eff);

                let support_lit = solver.reify(and(supported_by_eff_conjunction));
                encoding.tag(support_lit, Tag::Support(cond_id, eff_id));

                debug_assert!(solver
                    .model
                    .state
                    .implies(prez_cond, solver.model.presence_literal(support_lit.variable())));

                // add this support expression to the support clause
                supported.push(support_lit);
                num_support_constraints += 1;
            }

            // enforce necessary conditions for condition's support
            solver.enforce(or(supported), [prez_cond]);
        }
        tracing::debug!(%num_support_constraints);

        solver.propagate()?;
    }

    {
        // mutex constraints
        let span = tracing::span!(tracing::Level::TRACE, "mutex");
        let _span = span.enter();
        let mut num_mutex_constraints = 0;
        let actions: Vec<_> = pb
            .chronicles
            .iter()
            .filter(|ch| matches!(ch.chronicle.kind, ChronicleKind::Action | ChronicleKind::DurativeAction))
            .collect();
        // mutex actions constraints: a condition from an action cannot meet the effect of another action.
        // there needs to be an epsilon separation between the time an actions requires a fluent and the time
        // at which another action changes it.
        for &act1 in &actions {
            if solver.model.entails(!act1.chronicle.presence) {
                continue;
            }
            for cond in &act1.chronicle.conditions {
                for &act2 in &actions {
                    if solver.model.entails(!act2.chronicle.presence) {
                        continue;
                    }
                    if solver
                        .model
                        .state
                        .exclusive(act1.chronicle.presence, act2.chronicle.presence)
                    {
                        continue;
                    }
                    if ptr::eq(act1, act2) {
                        continue; // an action cannot be mutex with itself
                    }
                    for eff in &act2.chronicle.effects {
                        // `cond` and `eff` are a condition and an effect from two distinct action
                        if !unifiable_sv(&solver.model, &cond.state_var, &eff.state_var) {
                            continue;
                        }

                        let mut non_overlapping: Vec<Lit> = Vec::with_capacity(32);
                        debug_assert_eq!(cond.state_var.fluent, eff.state_var.fluent);
                        // not on same state variable
                        for idx in 0..cond.state_var.args.len() {
                            let a = cond.state_var.args[idx];
                            let b = eff.state_var.args[idx];
                            non_overlapping.push(solver.reify(neq(a, b)));
                        }

                        // or does not overlap the interval `[eff.transition_start, eff.persistence_start[`
                        // note that the interval is left-inclusive to enforce the epsilon separation
                        non_overlapping.push(solver.reify(f_lt(cond.end, eff.transition_start)));
                        non_overlapping.push(solver.reify(f_leq(eff.persistence_start, cond.start)));

                        solver.enforce(or(non_overlapping), [act1.chronicle.presence, act2.chronicle.presence]);
                        num_mutex_constraints += 1;
                    }
                }
            }
        }
        tracing::debug!(%num_mutex_constraints);

        solver.propagate()?;
    }

    {
        // Resource constraints
        let span = tracing::span!(tracing::Level::TRACE, "resources");
        let _span = span.enter();
        let mut num_resource_constraints = 0;

        // Get the effects and the conditions on numeric state variables.
        // Filter those who are not present.
        let assignments: Vec<_> = effs
            .iter()
            .filter(|(_, prez, eff)| {
                !solver.model.entails(!*prez)
                    && matches!(eff.operation, EffectOp::Assign(_))
                    && is_integer(&eff.state_var)
            })
            .collect();
        let increases: Vec<_> = effs
            .iter()
            .filter(|(_, prez, eff)| {
                !solver.model.entails(!*prez)
                    && matches!(eff.operation, EffectOp::Increase(_) | EffectOp::Decrease(_))
                    && is_integer(&eff.state_var)
            })
            .collect();
        let mut conditions: Vec<_> = conds
            .iter()
            .filter(|(_, prez, cond)| !solver.model.entails(!*prez) && is_integer(&cond.state_var))
            .map(|&(_, prez, cond)| (prez, cond.clone()))
            .collect();

        // Force the new assigned values to be in the state variable domain.
        for &&(_, prez, eff) in &assignments {
            let Type::Int { lb, ub } = eff.state_var.fluent.return_type() else {
                unreachable!()
            };
            let EffectOp::Assign(val) = eff.operation else {
                unreachable!()
            };
            let val: IAtom = val.try_into().expect("Not integer assignment to an int state variable");
            solver.enforce(geq(val, lb), [prez]);
            solver.enforce(leq(val, ub), [prez]);
            num_resource_constraints += 1;
        }

        // Convert the increase effects into conditions in order to check that the new value is in the state variable domain.
        for &&(eff_id, prez, eff) in &increases {
            assert!(
                eff.transition_start + FAtom::EPSILON == eff.persistence_start && eff.min_persistence_end.is_empty(),
                "Only instantaneous effects are supported"
            );
            // Get the bounds of the state variable.
            let (lb, ub) = if let Type::Int { lb, ub } = eff.state_var.fluent.return_type() {
                (lb, ub)
            } else {
                (INT_CST_MIN, INT_CST_MAX)
            };
            // Create a new variable with those bounds.
            let var = solver
                .model
                .new_ivar(lb, ub, Container::Instance(eff_id.instance_id) / VarType::Reification);
            // Check that the state variable value is equals to that new variable.
            // It will force the state variable value to be in the bounds of the new variable.
            conditions.push((
                prez,
                Condition {
                    start: eff.persistence_start,
                    end: eff.persistence_start,
                    state_var: eff.state_var.clone(),
                    value: var.into(),
                },
            ));
        }

        /*
         * For each condition `R == z`, a set of linear sum constraints of the form
         * `la_j*ca_j + li_jk*ci_jk + ... + li_jm*ci_jm - la_j*z == 0` are created.
         *
         * The `la_j` literal is true if and only if the associated assignment effect `e_j` is the last one for the condition.
         * A disjunctive clause will force to have at least one `la_j`.
         * The `ca_j` value is the value of the assignment effect `e_j`.
         * The `li_j*` literals are true if and only if:
         *  - `la_j` is true
         *  - the associated increase effect `e_*` is between `e_j` and the condition
         * The `ci_j*` values are the value of the increase effects `e_*`.
         * The `z` variable is the value of the condition.
         *
         * With all these literals, only one constraint will be taken into account: that associated with the last assignment.
         */
        for (prez_cond, cond) in conditions {
            assert_eq!(cond.start, cond.end, "Only the instantaneous conditions are supported");

            // Whether the effect is likely to support the condition.
            let eff_compatible_with_cond = |solver: &Solver<VarLabel>, prez_eff: Lit, eff: &Effect| {
                !solver.model.state.exclusive(prez_eff, prez_cond)
                    && unifiable_sv(&solver.model, &cond.state_var, &eff.state_var)
            };

            let compatible_assignments = assignments
                .iter()
                .filter(|(_, prez, eff)| eff_compatible_with_cond(&solver, *prez, eff))
                .collect::<Vec<_>>();
            let compatible_increases = increases
                .iter()
                .filter(|(_, prez, eff)| eff_compatible_with_cond(&solver, *prez, eff))
                .collect::<Vec<_>>();

            // Vector to store the `la_j` literals, `ca_j` values and the start persistence timepoint of the effect `e_j`.
            let use_assign_end_timepoint = EnvParam::<bool>::new("ARIES_RESOURCE_LA_USE_ASSIGN_END_TIMEPOINT", "false");
            let la_ca_ta = if use_assign_end_timepoint.get() {
                create_la_vector_with_timepoints(
                    compatible_assignments,
                    &cond,
                    prez_cond,
                    &eff_assign_ends,
                    &mut solver,
                )
            } else {
                create_la_vector_without_timepoints(
                    compatible_assignments,
                    &cond,
                    prez_cond,
                    &eff_persis_ends,
                    &mut solver,
                )
            };

            // Force to have at least one assignment.
            let la_disjuncts = la_ca_ta.iter().map(|(la, _, _)| *la).collect::<Vec<_>>();
            solver.enforce(or(la_disjuncts), [prez_cond]);

            /*
             * Matrix to store the `li_j*` literals and `ci_j*` values.
             *
             * `li_j*` is true if and only if the associated increase effect `e_*`:
             *  - is present
             *  - is before the condition
             *  - is after the assignment effect `e_j`
             *  - has the same state variable as the condition
             *  - `la_j` is true
             */
            let m_li_ci = la_ca_ta
                .iter()
                .map(|(la, _, ta)| {
                    compatible_increases
                        .iter()
                        .map(|(_, prez_eff, eff)| {
                            let mut li_conjunction: Vec<Lit> = Vec::with_capacity(12);
                            // `la_j` is true
                            li_conjunction.push(*la);
                            // is present
                            li_conjunction.push(*prez_eff);
                            // is before the condition
                            li_conjunction.push(solver.reify(f_leq(eff.persistence_start, cond.start)));
                            // is after the assignment effect `e_j`
                            li_conjunction.push(solver.reify(f_lt(*ta, eff.persistence_start)));
                            // has the same state variable as the condition
                            debug_assert_eq!(cond.state_var.fluent, eff.state_var.fluent);
                            for idx in 0..cond.state_var.args.len() {
                                let a = cond.state_var.args[idx];
                                let b = eff.state_var.args[idx];
                                li_conjunction.push(solver.reify(eq(a, b)));
                            }
                            // Create the `li_j*` literal.
                            let li_lit = solver.reify(and(li_conjunction));
                            debug_assert!(solver
                                .model
                                .state
                                .implies(prez_cond, solver.model.presence_literal(li_lit.variable())));

                            // Get the `ci_j*` value.
                            let (ci, sign) = match eff.operation {
                                EffectOp::Increase(eff_val) => (eff_val, 1),
                                EffectOp::Decrease(eff_val) => (eff_val, -1),
                                _ => unreachable!(),
                            };
                            // let ci: IAtom = eff_val.into();
                            (li_lit, ci, sign)
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();

            // Create the linear sum constraints.
            for (&(la, ca, _), li_ci) in la_ca_ta.iter().zip(m_li_ci) {
                // Create the sum.
                let mut sum = LinearSum::zero();
                sum += LinearSum::with_lit(ca, la);
                for &(li, ci, sign) in li_ci.iter() {
                    match sign {
                        -1 => sum -= LinearSum::with_lit(ci, li),
                        1 => sum += LinearSum::with_lit(ci, li),
                        _ => unreachable!(),
                    }
                }
                let cond_val =
                    IAtom::try_from(cond.value).expect("Condition value is not numeric for a numeric fluent");
                sum -= LinearSum::with_lit(cond_val, la);
                // Force the sum to be equals to 0.
                solver.enforce(sum.clone().geq(0), [prez_cond]);
                solver.enforce(sum.leq(0), [prez_cond]);
                num_resource_constraints += 1;
            }
        }
        tracing::debug!(%num_resource_constraints);

        solver.propagate()?;
    }

    let metric = metric.map(|metric| add_metric(pb, &mut solver.model, metric));

    tracing::debug!("Done.");
    Ok(EncodedProblem {
        model: solver.model,
        objective: metric,
        encoding,
    })
}

/* ======================= Resource `la` Vector Creation ====================== */

/**
Vector to store the `la_j` literals, `ca_j` values and the start persistence timepoint of the effect `e_j`.

`la_j` is true if and only if the associated assignment effect `e_j`:
 - is present
 - is before the condition
 - has the same state variable as the condition
 - for each other assignment effect `e_k` meeting the above conditions, `e_k` is before `e_j`
**/
fn create_la_vector_without_timepoints(
    assignments: Vec<&&(EffID, Lit, &Effect)>,
    cond: &Condition,
    prez_cond: Lit,
    eff_persis_ends: &HashMap<EffID, FVar>,
    solver: &mut Solver<VarLabel>,
) -> Vec<(Lit, IAtom, FAtom)> {
    assignments
        .iter()
        .map(|(eff_id, prez_eff, eff)| {
            let mut la_conjunction: Vec<Lit> = Vec::with_capacity(32);
            // is present
            la_conjunction.push(*prez_eff);
            // is before the condition
            la_conjunction.push(solver.reify(f_leq(eff.persistence_start, cond.start)));
            // has the same state variable as the condition
            debug_assert_eq!(cond.state_var.fluent, eff.state_var.fluent);
            for idx in 0..cond.state_var.args.len() {
                let a = cond.state_var.args[idx];
                let b = eff.state_var.args[idx];
                la_conjunction.push(solver.reify(eq(a, b)));
            }
            // for each other assignment effect `e_k` meeting the above conditions, `e_k` is before `e_j`
            // This implies constraint is expressed as a disjunction.
            for (other_eff_id, prez_other_eff, other_eff) in assignments.iter() {
                // same effect: continue
                if eff_id == other_eff_id {
                    continue;
                }
                let mut disjunction: Vec<Lit> = Vec::with_capacity(12);
                // is not present
                disjunction.push(!*prez_other_eff);
                // is after the condition
                disjunction.push(solver.reify(f_lt(cond.end, other_eff.persistence_start)));
                // has a state variable different from the condition
                debug_assert_eq!(cond.state_var.fluent, other_eff.state_var.fluent);
                for idx in 0..cond.state_var.args.len() {
                    let a = cond.state_var.args[idx];
                    let b = other_eff.state_var.args[idx];
                    disjunction.push(solver.reify(neq(a, b)));
                }
                // is before the effect `e_j`
                disjunction.push(solver.reify(f_lt(eff_persis_ends[other_eff_id], eff.persistence_start)));

                let disjunction_lit = solver.reify(or(disjunction.clone()));
                la_conjunction.push(disjunction_lit);
            }

            // Create the `la_j` literal.
            let la_lit = solver.reify(and(la_conjunction.clone()));
            debug_assert!(solver
                .model
                .state
                .implies(prez_cond, solver.model.presence_literal(la_lit.variable())));

            // Get the `ca_j` variable.
            let EffectOp::Assign(eff_val) = eff.operation else {
                unreachable!()
            };
            let ca = IAtom::try_from(eff_val).expect("Try to assign a non-numeric value to a numeric fluent");

            // Get the persistence timepoint of the effect `e_j`.
            let ta = eff.persistence_start;
            (la_lit, ca, ta)
        })
        .collect::<Vec<_>>()
}

/**
Vector to store the `la_j` literals, `ca_j` values and the start persistence timepoint of the effect `e_j`.

`la_j` is true if and only if the associated assignment effect `e_j`:
 - is present
 - has the same state variable as the condition
 - the condition is between the start persistence and the assignment end (new timepoint)
**/
fn create_la_vector_with_timepoints(
    assignments: Vec<&&(EffID, Lit, &Effect)>,
    cond: &Condition,
    prez_cond: Lit,
    eff_assign_ends: &HashMap<EffID, FVar>,
    solver: &mut Solver<VarLabel>,
) -> Vec<(Lit, IAtom, FAtom)> {
    assignments
        .iter()
        .map(|(eff_id, prez_eff, eff)| {
            let mut la_conjunction: Vec<Lit> = Vec::with_capacity(32);
            // is present
            la_conjunction.push(*prez_eff);
            // has the same state variable as the condition
            debug_assert_eq!(cond.state_var.fluent, eff.state_var.fluent);
            for idx in 0..cond.state_var.args.len() {
                let a = cond.state_var.args[idx];
                let b = eff.state_var.args[idx];
                la_conjunction.push(solver.reify(eq(a, b)));
            }
            // the condition is between the start persistence and the assignment end
            la_conjunction.push(solver.reify(f_leq(eff.persistence_start, cond.start)));
            la_conjunction.push(solver.reify(f_leq(cond.end, eff_assign_ends[eff_id])));

            // Create the `la_j` literal.
            let la_lit = solver.reify(and(la_conjunction.clone()));
            debug_assert!(solver
                .model
                .state
                .implies(prez_cond, solver.model.presence_literal(la_lit.variable())));

            // Get the `ca_j` variable.
            let EffectOp::Assign(eff_val) = eff.operation else {
                unreachable!()
            };
            let ca = IAtom::try_from(eff_val).expect("Try to assign a non-numeric value to a numeric fluent");

            // Get the persistence timepoint of the effect `e_j`.
            let ta = eff.persistence_start;
            (la_lit, ca, ta)
        })
        .collect::<Vec<_>>()
}
