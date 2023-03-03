pub mod pddl;
pub mod sexpr;

use crate::chronicles::*;
use crate::classical::state::{SvId, World};
use crate::parsing::pddl::{PddlFeature, TypedSymbol};

use crate::chronicles::constraints::Constraint;
use crate::parsing::sexpr::SExpr;
use anyhow::{Context, Result};
use aries::core::*;
use aries_model::extensions::Shaped;
use aries_model::lang::*;
use aries_model::symbols::SymbolTable;
use aries_model::types::TypeHierarchy;
use aries::utils::input::{ErrLoc, Loc, Sym};
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;

/// Names for built in types. They contain UTF-8 symbols for sexiness (and to avoid collision with user defined symbols)
static TASK_TYPE: &str = "★task★";
static ABSTRACT_TASK_TYPE: &str = "★abstract_task★";
static ACTION_TYPE: &str = "★action★";
static DURATIVE_ACTION_TYPE: &str = "★durative-action★";
static METHOD_TYPE: &str = "★method★";
static PREDICATE_TYPE: &str = "★predicate★";
static OBJECT_TYPE: &str = "★object★";
static FUNCTION_TYPE: &str = "★function★";

type Pb = Problem;

pub fn pddl_to_chronicles(dom: &pddl::Domain, prob: &pddl::Problem) -> Result<Pb> {
    // top types in pddl
    let mut types: Vec<(Sym, Option<Sym>)> = vec![
        (TASK_TYPE.into(), None),
        (ABSTRACT_TASK_TYPE.into(), Some(TASK_TYPE.into())),
        (ACTION_TYPE.into(), Some(TASK_TYPE.into())),
        (DURATIVE_ACTION_TYPE.into(), Some(TASK_TYPE.into())),
        (METHOD_TYPE.into(), None),
        (PREDICATE_TYPE.into(), None),
        (FUNCTION_TYPE.into(), None),
        (OBJECT_TYPE.into(), None),
    ];
    let top_type = OBJECT_TYPE.into();

    // determine the top types in the user-defined hierarchy.
    // this is typically "object" by convention but might something else (e.g. "obj" in some hddl problems).
    {
        let all_types: HashSet<&Sym> = dom.types.iter().map(|tpe| &tpe.symbol).collect();
        let top_types = dom
            .types
            .iter()
            .filter_map(|tpe| tpe.tpe.as_ref())
            .filter(|tpe| !all_types.contains(tpe))
            .unique();
        for t in top_types {
            types.push((t.clone(), Some(OBJECT_TYPE.into())));
        }
    }

    for t in &dom.types {
        types.push((t.symbol.clone(), t.tpe.clone()));
    }

    let ts = TypeHierarchy::new(types)?;
    let mut symbols: Vec<TypedSymbol> = prob.objects.clone();
    for c in &dom.constants {
        symbols.push(c.clone());
    }
    // predicates are symbols as well, add them to the table
    for p in &dom.predicates {
        symbols.push(TypedSymbol::new(&p.name, PREDICATE_TYPE));
    }
    for a in &dom.actions {
        symbols.push(TypedSymbol::new(&a.name, ACTION_TYPE));
    }
    for a in &dom.durative_actions {
        symbols.push(TypedSymbol::new(&a.name, DURATIVE_ACTION_TYPE));
    }
    for t in &dom.tasks {
        symbols.push(TypedSymbol::new(&t.name, ABSTRACT_TASK_TYPE));
    }
    for m in &dom.methods {
        symbols.push(TypedSymbol::new(&m.name, METHOD_TYPE));
    }
    //Add function name are symbols too
    for f in &dom.functions {
        symbols.push(TypedSymbol::new(&f.name, FUNCTION_TYPE));
    }

    let symbols = symbols
        .drain(..)
        .map(|ts| (ts.symbol, ts.tpe.unwrap_or_else(|| OBJECT_TYPE.into())))
        .collect();
    let symbol_table = SymbolTable::new(ts, symbols)?;

    let mut state_variables = Vec::with_capacity(dom.predicates.len() + dom.functions.len());
    for pred in &dom.predicates {
        let sym = symbol_table
            .id(&pred.name)
            .ok_or_else(|| pred.name.invalid("Unknown symbol"))?;
        let mut args = Vec::with_capacity(pred.args.len() + 1);
        for a in &pred.args {
            let tpe = a.tpe.as_ref().unwrap_or(&top_type);
            let tpe = symbol_table
                .types
                .id_of(tpe)
                .ok_or_else(|| tpe.invalid("Unknown type"))?;
            args.push(Type::Sym(tpe));
        }
        args.push(Type::Bool); // return type (last one) is a boolean
        state_variables.push(StateFun { sym, tpe: args })
    }
    for fun in &dom.functions {
        let sym = symbol_table
            .id(&fun.name)
            .ok_or_else(|| fun.name.invalid("Unknown symbol"))?;
        let mut args = Vec::with_capacity(fun.args.len() + 1);
        for a in &fun.args {
            let tpe = a.tpe.as_ref().unwrap_or(&top_type);
            let tpe = symbol_table
                .types
                .id_of(tpe)
                .ok_or_else(|| tpe.invalid("Unknown type"))?;
            args.push(Type::Sym(tpe));
        }
        // TODO: set to a fixed-point numeral of appropriate precision
        args.push(Type::Int); // return type (last one) is a int value
        state_variables.push(StateFun { sym, tpe: args })
    }

    let mut context = Ctx::new(Arc::new(symbol_table), state_variables);

    let init_container = Container::Instance(0);
    // Initial chronicle construction
    let mut init_ch = Chronicle {
        kind: ChronicleKind::Problem,
        presence: Lit::TRUE,
        start: context.origin(),
        end: context.horizon(),
        name: vec![],
        task: None,
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
        cost: None,
    };

    // Transforms atoms of an s-expression into the corresponding representation for chronicles
    let as_model_atom_no_borrow = |atom: &sexpr::SAtom, context: &Ctx| -> Result<SAtom> {
        let atom = context
            .model
            .get_symbol_table()
            .id(atom.canonical_str())
            .ok_or_else(|| atom.invalid("Unknown atom"))?;
        let atom = context.typed_sym(atom);
        Ok(atom.into())
    };
    let as_model_atom = |atom: &sexpr::SAtom| as_model_atom_no_borrow(atom, &context);
    for goal in &prob.goal {
        // goal is expected to be a conjunction of the form:
        //  - `(and (= sv1 v1) (= sv2 = v2))`
        //  - `(= sv1 v1)`
        //  - `()`
        let goals = read_conjunction(goal, as_model_atom)?;
        for TermLoc(goal, loc) in goals {
            match goal {
                Term::Binding(sv, value) => init_ch.conditions.push(Condition {
                    start: init_ch.end,
                    end: init_ch.end,
                    state_var: sv,
                    value,
                }),
                _ => return Err(loc.invalid("Unsupported in goal expression").into()),
            }
        }
    }
    // If we have negative preconditions, we need to assume a closed world assumption.
    // Indeed, some preconditions might rely on initial facts being false
    let closed_world = dom.features.contains(&PddlFeature::NegativePreconditions);
    for (sv, val) in read_init(&prob.init, closed_world, as_model_atom, &context)? {
        init_ch.effects.push(Effect {
            transition_start: init_ch.start,
            persistence_start: init_ch.start,
            min_persistence_end: Vec::new(),
            state_var: sv,
            value: val,
        });
    }

    if let Some(ref task_network) = &prob.task_network {
        read_task_network(
            init_container,
            task_network,
            &as_model_atom_no_borrow,
            &mut init_ch,
            None,
            &mut context,
        )?;
    }

    let init_ch = ChronicleInstance {
        parameters: vec![],
        origin: ChronicleOrigin::Original,
        chronicle: init_ch,
    };

    let mut templates = Vec::new();
    for a in &dom.actions {
        let cont = Container::Template(templates.len());
        let template = read_chronicle_template(cont, a, &mut context)?;
        templates.push(template);
    }
    for a in &dom.durative_actions {
        let cont = Container::Template(templates.len());
        let template = read_chronicle_template(cont, a, &mut context)?;
        templates.push(template);
    }
    for m in &dom.methods {
        let cont = Container::Template(templates.len());
        let template = read_chronicle_template(cont, m, &mut context)?;
        templates.push(template);
    }

    let problem = Problem {
        context,
        templates,
        chronicles: vec![init_ch],
    };

    Ok(problem)
}

/// Transforms PDDL initial facts into binding of state variables to their values
/// If `closed_world` is true, then all predicates that are not given a true value will be set to false.
fn read_init(
    initial_facts: &[SExpr],
    closed_world: bool,
    as_model_atom: impl Fn(&sexpr::SAtom) -> Result<SAtom>,
    context: &Ctx,
) -> Result<Vec<(Sv, Atom)>> {
    let mut facts = Vec::new();
    if closed_world {
        // closed world, every predicate that is not given a true value should be given a false value
        // to do this, we rely on the classical classical planning state
        let state_desc = World::new(
            context.model.get_symbol_table().deref().clone(),
            &context.state_functions,
        )?;
        let mut s = state_desc.make_new_state();
        for init in initial_facts {
            let pred = read_sv(init, &state_desc)?;
            s.add(pred);
        }

        let sv_to_sv = |sv| -> Vec<SAtom> {
            state_desc
                .sv_of(sv)
                .iter()
                .map(|&sym| context.typed_sym(sym).into())
                .collect()
        };

        for literal in s.literals() {
            let sv = sv_to_sv(literal.var());
            let val: Atom = literal.val().into();
            facts.push((sv, val));
        }
    } else {
        // open world, we only add to the initial facts the one explicitly given in the problem definition
        for e in initial_facts {
            match read_init_state(e, &as_model_atom)? {
                TermLoc(Term::Binding(sv, val), _) => facts.push((sv, val)),
                TermLoc(_, loc) => return Err(loc.invalid("Unsupported in initial facts").into()),
            }
        }
    }
    Ok(facts)
}

/// Transforms a PDDL action into a Chronicle template
///
/// # Parameters
///
/// - `c`: Identifier of the container that will be associated with the chronicle
/// - `pddl`: A view of a PDDL construct to be instantiated as a chronicle.
///   Can be, e.g., an instantaneous action, a method, ...
/// - `context`: Context in which the chronicle appears. Used to create new variables.
fn read_chronicle_template(
    c: Container,
    pddl: impl ChronicleTemplateView,
    context: &mut Ctx,
) -> Result<ChronicleTemplate> {
    let top_type = OBJECT_TYPE.into();

    // All parameters of the chronicle (!= from parameters of the action)
    // Must contain all variables that were created for this chronicle template
    // and should be replaced when instantiating the chronicle
    let mut params: Vec<Variable> = Vec::new();
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();
    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);
    let end: FAtom = match pddl.kind() {
        ChronicleKind::Problem => panic!("unsupported case"),
        ChronicleKind::Method | ChronicleKind::DurativeAction => {
            let end = context
                .model
                .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleEnd);
            params.push(end.into());
            end.into()
        }
        ChronicleKind::Action => start + FAtom::EPSILON,
    };

    // name of the chronicle : name of the action + parameters
    let mut name: Vec<SAtom> = Vec::with_capacity(1 + pddl.parameters().len());
    let base_name = pddl.base_name();
    name.push(
        context
            .typed_sym(
                context
                    .model
                    .get_symbol_table()
                    .id(base_name)
                    .ok_or_else(|| base_name.invalid("Unknown atom"))?,
            )
            .into(),
    );
    // Process, the arguments of the action, adding them to the parameters of the chronicle and to the name of the action
    for arg in pddl.parameters() {
        let tpe = arg.tpe.as_ref().unwrap_or(&top_type);
        let tpe = context
            .model
            .get_symbol_table()
            .types
            .id_of(tpe)
            .ok_or_else(|| tpe.invalid("Unknown atom"))?;
        let arg = context
            .model
            .new_optional_sym_var(tpe, prez, c / VarType::Parameter(arg.symbol.to_string()));
        params.push(arg.into());
        name.push(arg.into());
    }
    // Transforms atoms of an s-expression into the corresponding representation for chronicles
    let as_chronicle_atom_no_borrow = |atom: &sexpr::SAtom, context: &Ctx| -> Result<SAtom> {
        match pddl
            .parameters()
            .iter()
            .position(|arg| arg.symbol.canonical_str() == atom.canonical_str())
        {
            Some(i) => Ok(name[i + 1]),
            None => {
                let atom = context
                    .model
                    .get_symbol_table()
                    .id(atom.canonical_str())
                    .ok_or_else(|| atom.invalid("Unknown atom"))?;
                let atom = context.typed_sym(atom);
                Ok(atom.into())
            }
        }
    };
    let as_chronicle_atom = |atom: &sexpr::SAtom| -> Result<SAtom> { as_chronicle_atom_no_borrow(atom, context) };

    let task = if let Some(task) = pddl.task() {
        let mut task_name = Vec::with_capacity(task.arguments.len() + 1);
        task_name.push(as_chronicle_atom(&task.name)?);
        for task_arg in &task.arguments {
            task_name.push(as_chronicle_atom(task_arg)?);
        }
        task_name
    } else {
        // no explicit task (typical for a primitive action), use the name as the task
        name.clone()
    };

    // TODO: here the cost is simply 1 for any primitive action
    let cost = match pddl.kind() {
        ChronicleKind::Problem | ChronicleKind::Method => None,
        ChronicleKind::Action | ChronicleKind::DurativeAction => Some(1),
    };

    let mut ch = Chronicle {
        kind: pddl.kind(),
        presence: prez,
        start,
        end,
        name: name.clone(),
        task: Some(task),
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
        cost,
    };

    for eff in pddl.effects() {
        if pddl.kind() != ChronicleKind::Action && pddl.kind() != ChronicleKind::DurativeAction {
            return Err(eff.invalid("Unexpected instantaneous effect").into());
        }
        let effects = read_conjunction(eff, as_chronicle_atom)?;
        for TermLoc(term, loc) in effects {
            match term {
                Term::Binding(sv, val) => ch.effects.push(Effect {
                    transition_start: ch.start,
                    persistence_start: ch.end,
                    min_persistence_end: Vec::new(),
                    state_var: sv,
                    value: val,
                }),
                _ => return Err(loc.invalid("Unsupported in action effects").into()),
            }
        }
    }

    for eff in pddl.timed_effects() {
        if pddl.kind() != ChronicleKind::Action && pddl.kind() != ChronicleKind::DurativeAction {
            return Err(eff.invalid("Unexpected effect").into());
        }
        // conjunction of effects of the form `(and (at-start (= sv1 v1)) (at-end (= sv2 v2)))`
        let effects = read_temporal_conjunction(eff, as_chronicle_atom)?;
        for TemporalTerm(qualification, term) in effects {
            match term.0 {
                Term::Binding(state_var, value) => match qualification {
                    TemporalQualification::AtStart => {
                        ch.effects.push(Effect {
                            transition_start: ch.start,
                            persistence_start: ch.start + FAtom::EPSILON,
                            min_persistence_end: Vec::new(),
                            state_var,
                            value,
                        });
                    }
                    TemporalQualification::AtEnd => {
                        ch.effects.push(Effect {
                            transition_start: ch.end,
                            persistence_start: ch.end + FAtom::EPSILON,
                            min_persistence_end: Vec::new(),
                            state_var,
                            value,
                        });
                    }
                    TemporalQualification::OverAll => {
                        return Err(term.1.invalid("Unsupported in action effects").into())
                    }
                },
                Term::Eq(_a, _b) => return Err(term.1.invalid("Unsupported in action effects").into()),
                Term::Neq(_a, _b) => return Err(term.1.invalid("Unsupported in action effects").into()),
            }
        }
    }

    // a common pattern in PDDL is to have two effect (not x) and (x) on the same state variable.
    // This is to force mutual exclusion on x. The semantics of PDDL have the negative effect applied first.
    // This is already enforced by our translation of a positive effect on x as `]start, end] x <- true`
    // Thus if we have both a positive effect and a negative effect on the same state variable,
    // we remove the negative one
    let positive_effects: HashSet<_> = ch
        .effects
        .iter()
        .filter(|e| e.value == Atom::from(true))
        .map(|e| (e.state_var.clone(), e.persistence_start, e.transition_start))
        .collect();
    ch.effects.retain(|e| {
        e.value != Atom::from(false)
            || !positive_effects.contains(&(e.state_var.clone(), e.persistence_start, e.transition_start))
    });

    // TODO : check if work around still needed
    for cond in pddl.preconditions() {
        let conditions = read_conjunction(cond, as_chronicle_atom)?;
        for TermLoc(term, _) in conditions {
            match term {
                Term::Binding(sv, val) => {
                    let has_effect_on_same_state_variable = ch
                        .effects
                        .iter()
                        .map(|e| e.state_var.as_slice())
                        .any(|x| x == sv.as_slice());

                    // end time of the effect, if it is a method, or there is an effect of the same state variable,
                    // then we have an instantaneous start condition.
                    // Otherwise, the condition spans the entire action
                    let end = if has_effect_on_same_state_variable || pddl.kind() == ChronicleKind::Method {
                        ch.start // there is corresponding effect
                    } else {
                        ch.end // no effect, condition needs to persist until the end of the action
                    };
                    ch.conditions.push(Condition {
                        start: ch.start,
                        end,
                        state_var: sv,
                        value: val,
                    });
                }
                Term::Eq(a, b) => ch.constraints.push(Constraint::eq(a, b)),
                Term::Neq(a, b) => ch.constraints.push(Constraint::neq(a, b)),
            }
        }
    }

    // handle duration element from durative actions
    if let Some(dur) = pddl.duration() {
        // currently, we only support constraint of the form `(= ?duration <i32>)`
        // TODO: extend durations constraints, to support the full PDDL spec
        let mut dur = dur.as_list_iter().unwrap();
        //Check for first two elements
        dur.pop_known_atom("=")?;
        dur.pop_known_atom("?duration")?;

        let dur_atom = dur.pop_atom()?;
        let duration = dur_atom
            .canonical_str()
            .parse::<i32>()
            .map_err(|_| dur_atom.invalid("Expected an integer"))
            .unwrap();
        ch.constraints.push(Constraint::duration(duration));
        if let Ok(x) = dur.pop() {
            return Err(x.invalid("Unexpected").into());
        }
    }

    //Handling temporal conditions
    for cond in pddl.timed_conditions() {
        let conditions = read_temporal_conjunction(cond, as_chronicle_atom)?;
        //let duration = read_duration()?;

        for TemporalTerm(qualification, term) in conditions {
            match term.0 {
                Term::Binding(state_var, value) => match qualification {
                    TemporalQualification::AtStart => {
                        ch.conditions.push(Condition {
                            start: ch.start,
                            end: ch.start,
                            state_var,
                            value,
                        });
                    }
                    TemporalQualification::AtEnd => {
                        ch.conditions.push(Condition {
                            start: ch.end,
                            end: ch.end,
                            state_var,
                            value,
                        });
                    }
                    TemporalQualification::OverAll => {
                        ch.conditions.push(Condition {
                            start: ch.start,
                            end: ch.end,
                            state_var,
                            value,
                        });
                    }
                },
                Term::Eq(a, b) => ch.constraints.push(Constraint::eq(a, b)),
                Term::Neq(a, b) => ch.constraints.push(Constraint::neq(a, b)),
            }
        }
    }

    if let Some(tn) = pddl.task_network() {
        read_task_network(c, tn, &as_chronicle_atom_no_borrow, &mut ch, Some(&mut params), context)?
    }

    let template = ChronicleTemplate {
        label: Some(pddl.base_name().to_string()),
        parameters: params,
        chronicle: ch,
    };
    Ok(template)
}

/// An adapter to allow treating pddl actions and hddl methods identically
trait ChronicleTemplateView {
    fn kind(&self) -> ChronicleKind;
    fn base_name(&self) -> &Sym;
    fn parameters(&self) -> &[TypedSymbol];
    fn task(&self) -> Option<&pddl::Task>;
    fn duration(&self) -> Option<&SExpr>;
    fn preconditions(&self) -> &[SExpr];
    fn timed_conditions(&self) -> &[SExpr];
    fn effects(&self) -> &[SExpr];
    fn timed_effects(&self) -> &[SExpr];
    fn task_network(&self) -> Option<&pddl::TaskNetwork>;
}
impl ChronicleTemplateView for &pddl::Action {
    fn kind(&self) -> ChronicleKind {
        ChronicleKind::Action
    }
    fn base_name(&self) -> &Sym {
        &self.name
    }
    fn parameters(&self) -> &[TypedSymbol] {
        &self.args
    }
    fn task(&self) -> Option<&pddl::Task> {
        None
    }
    fn duration(&self) -> Option<&SExpr> {
        None
    }
    fn preconditions(&self) -> &[SExpr] {
        &self.pre
    }
    fn timed_conditions(&self) -> &[SExpr] {
        &[]
    }
    fn effects(&self) -> &[SExpr] {
        &self.eff
    }
    fn timed_effects(&self) -> &[SExpr] {
        &[]
    }
    fn task_network(&self) -> Option<&pddl::TaskNetwork> {
        None
    }
}
impl ChronicleTemplateView for &pddl::DurativeAction {
    fn kind(&self) -> ChronicleKind {
        ChronicleKind::DurativeAction
    }
    fn base_name(&self) -> &Sym {
        &self.name
    }
    fn parameters(&self) -> &[TypedSymbol] {
        &self.args
    }
    fn task(&self) -> Option<&pddl::Task> {
        None
    }
    fn duration(&self) -> Option<&SExpr> {
        Some(&self.duration)
    }
    fn preconditions(&self) -> &[SExpr] {
        &[]
    }
    fn timed_conditions(&self) -> &[SExpr] {
        &self.conditions
    }
    fn effects(&self) -> &[SExpr] {
        &[]
    }
    fn timed_effects(&self) -> &[SExpr] {
        &self.effects
    }
    fn task_network(&self) -> Option<&pddl::TaskNetwork> {
        None
    }
}
impl ChronicleTemplateView for &pddl::Method {
    fn kind(&self) -> ChronicleKind {
        ChronicleKind::Method
    }
    fn base_name(&self) -> &Sym {
        &self.name
    }
    fn parameters(&self) -> &[TypedSymbol] {
        &self.parameters
    }
    fn task(&self) -> Option<&pddl::Task> {
        Some(&self.task)
    }
    fn duration(&self) -> Option<&SExpr> {
        None
    }
    fn preconditions(&self) -> &[SExpr] {
        &self.precondition
    }
    fn timed_conditions(&self) -> &[SExpr] {
        &[]
    }
    fn effects(&self) -> &[SExpr] {
        &[]
    }
    fn timed_effects(&self) -> &[SExpr] {
        &[]
    }
    fn task_network(&self) -> Option<&pddl::TaskNetwork> {
        Some(&self.subtask_network)
    }
}

/// Parses a task network and adds its components (subtasks and constraints) to the target `chronicle.
/// All newly created variables (timepoints of the subtasks) are added to the new_variables buffer.
fn read_task_network(
    c: Container,
    tn: &pddl::TaskNetwork,
    as_chronicle_atom: &impl Fn(&sexpr::SAtom, &Ctx) -> Result<SAtom>,
    chronicle: &mut Chronicle,
    mut new_variables: Option<&mut Vec<Variable>>,
    context: &mut Ctx,
) -> Result<()> {
    // stores the start/end timepoints of each named task
    let mut named_task: HashMap<String, (FAtom, FAtom)> = HashMap::new();

    let presence = chronicle.presence;
    // creates a new subtask. This will create new variables for the start and end
    // timepoints of the task and push the `new_variables` vector, if any.
    let mut make_subtask = |t: &pddl::Task, task_id: u32| -> Result<SubTask> {
        let id = t.id.as_ref().map(|id| id.canonical_string());
        // get the name + parameters of the task
        let mut task_name = Vec::with_capacity(t.arguments.len() + 1);
        task_name.push(as_chronicle_atom(&t.name, context)?);
        for param in &t.arguments {
            task_name.push(as_chronicle_atom(param, context)?);
        }
        // create timepoints for the subtask
        let start =
            context
                .model
                .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, presence, c / VarType::TaskStart(task_id));
        let end = context
            .model
            .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, presence, c / VarType::TaskEnd(task_id));
        if let Some(ref mut params) = new_variables {
            params.push(start.into());
            params.push(end.into());
        }
        let start = FAtom::from(start);
        let end = FAtom::from(end);
        if let Some(name) = id.as_ref() {
            named_task.insert(name.clone(), (start, end));
        }
        Ok(SubTask {
            id,
            start,
            end,
            task_name,
        })
    };
    let mut task_id = 0;
    for t in &tn.unordered_tasks {
        let t = make_subtask(t, task_id)?;
        chronicle.subtasks.push(t);
        task_id += 1;
    }

    // parse all ordered tasks, adding precedence constraints between subsequent ones
    let mut previous_end = None;
    for t in &tn.ordered_tasks {
        let t = make_subtask(t, task_id)?;

        if let Some(previous_end) = previous_end {
            chronicle.constraints.push(Constraint::lt(previous_end, t.start))
        }
        previous_end = Some(t.end);
        chronicle.subtasks.push(t);
        task_id += 1;
    }
    for ord in &tn.orderings {
        let first_end = named_task
            .get(ord.first_task_id.canonical_str())
            .ok_or_else(|| ord.first_task_id.invalid("Unknown task id"))?
            .1;
        let second_start = named_task
            .get(ord.second_task_id.canonical_str())
            .ok_or_else(|| ord.second_task_id.invalid("Unknown task id"))?
            .0;
        chronicle.constraints.push(Constraint::lt(first_end, second_start));
    }

    Ok(())
}

enum Term {
    Binding(Sv, Atom),
    Eq(Atom, Atom),
    Neq(Atom, Atom),
}

/// A Term, with its location in the input file (for error handling).
struct TermLoc(Term, Loc);
struct TemporalTerm(TemporalQualification, TermLoc);

/// Temporal qualification that can be applied to an expression.
enum TemporalQualification {
    AtStart,
    OverAll,
    AtEnd,
}

impl std::str::FromStr for TemporalQualification {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "at start" => Ok(TemporalQualification::AtStart),
            "over all" => Ok(TemporalQualification::OverAll),
            "at end" => Ok(TemporalQualification::AtEnd),
            _ => Err(format!("Unknown temporal qualification: {s}")),
        }
    }
}

fn read_conjunction(e: &SExpr, t: impl Fn(&sexpr::SAtom) -> Result<SAtom>) -> Result<Vec<TermLoc>> {
    let mut result = Vec::new();
    read_conjunction_impl(e, &t, &mut result)?;
    Ok(result)
}

fn read_conjunction_impl(e: &SExpr, t: &impl Fn(&sexpr::SAtom) -> Result<SAtom>, out: &mut Vec<TermLoc>) -> Result<()> {
    if let Some(l) = e.as_list_iter() {
        if l.is_empty() {
            return Ok(()); // empty conjunction
        }
    }
    if let Some(conjuncts) = e.as_application("and") {
        for c in conjuncts.iter() {
            read_conjunction_impl(c, t, out)?;
        }
    } else {
        // should be directly a predicate
        out.push(read_possibly_negated_term(e, t)?);
    }
    Ok(())
}

fn read_temporal_conjunction(e: &SExpr, t: impl Fn(&sexpr::SAtom) -> Result<SAtom>) -> Result<Vec<TemporalTerm>> {
    let mut result = Vec::new();
    read_temporal_conjunction_impl(e, &t, &mut result)?;
    Ok(result)
}

// So for a temporal conjunctions of syntax
// (and (at start ?x) (at start ?y))
// we want to place in `out`:
//  - (TemporalQualification::AtStart, term_of(?x))
//  - (TemporalQualification::AtStart, term_of(?y))
// Vector(TemporalQualification, respective sv, respective atom)
fn read_temporal_conjunction_impl(
    e: &SExpr,
    t: &impl Fn(&sexpr::SAtom) -> Result<SAtom>,
    out: &mut Vec<TemporalTerm>,
) -> Result<()> {
    if let Some(l) = e.as_list_iter() {
        if l.is_empty() {
            return Ok(()); // empty conjunction
        }
    }
    if let Some(conjuncts) = e.as_application("and") {
        for c in conjuncts.iter() {
            read_temporal_conjunction_impl(c, t, out)?;
        }
    } else {
        // should be directly a temporaly qualified predicate
        out.push(read_temporal_term(e, t)?);
    }
    Ok(())
}

// Parses something of the form: (at start ?x)
// To retrieve the term (`?x`) and its temporal qualification (`at start`)
fn read_temporal_term(expr: &SExpr, t: impl Fn(&sexpr::SAtom) -> Result<SAtom>) -> Result<TemporalTerm> {
    let mut expr = expr
        .as_list_iter()
        .ok_or_else(|| expr.invalid("Expected a valid term"))?;
    let atom = expr.pop_atom()?.canonical_str(); // "at" or "over"
    let atom = atom.to_owned() + " " + expr.pop_atom()?.canonical_str(); // "at start", "at end", or "over all"

    let qualification = TemporalQualification::from_str(atom.as_str()).map_err(|e| expr.invalid(e))?;
    // Read term here
    let term = expr.pop()?; // the "term" in (at start "term")
    let term = read_possibly_negated_term(term, t)?;
    Ok(TemporalTerm(qualification, term))
}

fn read_possibly_negated_term(e: &SExpr, t: impl Fn(&sexpr::SAtom) -> Result<SAtom>) -> Result<TermLoc> {
    if let Some([to_negate]) = e.as_application("not") {
        let TermLoc(t, _) = read_term(to_negate, &t)?;
        let negated = match t {
            Term::Binding(sv, value) => {
                if let Ok(value) = Lit::try_from(value) {
                    Term::Binding(sv, Atom::from(!value))
                } else {
                    return Err(to_negate.invalid("Could not apply 'not' to this expression").into());
                }
            }
            Term::Eq(a, b) => Term::Neq(a, b),
            Term::Neq(a, b) => Term::Eq(a, b),
        };
        Ok(TermLoc(negated, e.loc()))
    } else {
        // should be directly a predicate
        Ok(read_term(e, &t)?)
    }
}

fn read_init_state(expr: &SExpr, t: impl Fn(&sexpr::SAtom) -> Result<SAtom>) -> Result<TermLoc> {
    let mut l = expr.as_list_iter().ok_or_else(|| expr.invalid("Expected a term"))?;
    if let Some(head) = l.peek() {
        let head = head.as_atom().ok_or_else(|| head.invalid("Expected an atom"))?;
        let term = match head.canonical_str() {
            "=" => {
                l.pop_known_atom("=")?;
                let expr = l.pop()?.as_list_iter().unwrap();
                let mut sv = Vec::with_capacity(l.len());
                for e in expr {
                    let atom = e.as_atom().ok_or_else(|| e.invalid("Expected an atom"))?;
                    let atom = t(atom)?;
                    sv.push(atom);
                }
                let value = l
                    .pop_atom()?
                    .clone()
                    .canonical_str()
                    .parse::<i32>()
                    .map_err(|_| l.invalid("Expected an integer"))?;
                if let Some(unexpected) = l.next() {
                    return Err(unexpected.invalid("Unexpected expr").into());
                }
                Term::Binding(sv, Atom::Int(value.into()))
            }
            _ => {
                let mut sv = Vec::with_capacity(l.len());
                for e in l {
                    let atom = e.as_atom().ok_or_else(|| e.invalid("Expected an atom"))?;
                    let atom = t(atom)?;
                    sv.push(atom);
                }
                Term::Binding(sv, true.into())
            }
        };
        Ok(TermLoc(term, expr.loc()))
    } else {
        Err(l.loc().end().invalid("Expected a term").into())
    }
}

fn read_term(expr: &SExpr, t: impl Fn(&sexpr::SAtom) -> Result<SAtom>) -> Result<TermLoc> {
    let mut l = expr.as_list_iter().ok_or_else(|| expr.invalid("Expected a term"))?;
    if let Some(head) = l.peek() {
        let head = head.as_atom().ok_or_else(|| head.invalid("Expected an atom"))?;
        let term = match head.canonical_str() {
            "=" => {
                l.pop_known_atom("=")?;
                let a = l.pop_atom()?.clone();
                let b = l.pop_atom()?.clone();
                if let Some(unexpected) = l.next() {
                    return Err(unexpected.invalid("Unexpected expr").into());
                }
                Term::Eq(t(&a)?.into(), t(&b)?.into())
            }
            _ => {
                let mut sv = Vec::with_capacity(l.len());
                for e in l {
                    let atom = e.as_atom().ok_or_else(|| e.invalid("Expected an atom"))?;
                    let atom = t(atom)?;
                    sv.push(atom);
                }
                Term::Binding(sv, true.into())
            }
        };
        Ok(TermLoc(term, expr.loc()))
    } else {
        Err(l.loc().end().invalid("Expected a term").into())
    }
}

fn read_sv(e: &SExpr, desc: &World) -> Result<SvId> {
    let p = e.as_list().context("Expected s-expression")?;
    let atoms: Result<Vec<_>, ErrLoc> = p
        .iter()
        .map(|e| e.as_atom().ok_or_else(|| e.invalid("Expected atom")))
        .collect();
    let atom_ids: Result<Vec<_>, ErrLoc> = atoms?
        .iter()
        .map(|atom| {
            desc.table
                .id(atom.canonical_str())
                .ok_or_else(|| atom.invalid("Unknown atom"))
        })
        .collect();
    let atom_ids = atom_ids?;
    desc.sv_id(atom_ids.as_slice()).with_context(|| {
        format!(
            "Unknown predicate {} (wrong number of arguments or badly typed args ?)",
            desc.table.format(&atom_ids)
        )
    })
}
