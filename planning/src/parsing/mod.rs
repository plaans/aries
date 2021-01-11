pub mod pddl;
pub mod sexpr;

use crate::chronicles::*;
use crate::classical::state::{SVId, World};
use crate::parsing::pddl::{PddlFeature, TypedSymbol};

use crate::chronicles::constraints::Constraint;
use crate::parsing::sexpr::SExpr;
use anyhow::*;
use aries_model::lang::*;
use aries_model::symbols::SymbolTable;
use aries_model::types::TypeHierarchy;
use aries_utils::input::Sym;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::ops::Deref;
use std::sync::Arc;

/// Names for built in types. They contain UTF-8 symbols for sexiness (and to avoid collision with user defined symbols)
static TASK_TYPE: &str = "★task★";
static ABSTRACT_TASK_TYPE: &str = "★abstract_task★";
static ACTION_TYPE: &str = "★action★";
static METHOD_TYPE: &str = "★method★";
static PREDICATE_TYPE: &str = "★predicate★";
static OBJECT_TYPE: &str = "★object★";

type Pb = Problem;

pub fn pddl_to_chronicles(dom: &pddl::Domain, prob: &pddl::Problem) -> Result<Pb> {
    // top types in pddl
    let mut types: Vec<(Sym, Option<Sym>)> = vec![
        (TASK_TYPE.into(), None),
        (ABSTRACT_TASK_TYPE.into(), Some(TASK_TYPE.into())),
        (ACTION_TYPE.into(), Some(TASK_TYPE.into())),
        (METHOD_TYPE.into(), None),
        (PREDICATE_TYPE.into(), None),
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
    for t in &dom.tasks {
        symbols.push(TypedSymbol::new(&t.name, ABSTRACT_TASK_TYPE));
    }
    for m in &dom.methods {
        symbols.push(TypedSymbol::new(&m.name, METHOD_TYPE));
    }
    let symbols = symbols
        .drain(..)
        .map(|ts| (ts.symbol, ts.tpe.unwrap_or_else(|| OBJECT_TYPE.into())))
        .collect();
    let symbol_table = SymbolTable::new(ts, symbols)?;

    let mut state_variables = Vec::with_capacity(dom.predicates.len());
    for pred in &dom.predicates {
        let sym = symbol_table
            .id(&pred.name)
            .with_context(|| format!("Unknown symbol {}", &pred.name))?;
        let mut args = Vec::with_capacity(pred.args.len() + 1);
        for a in &pred.args {
            let tpe = a.tpe.as_ref().unwrap_or(&top_type);
            let tpe = symbol_table
                .types
                .id_of(tpe)
                .with_context(|| format!("Unknown type {}", tpe))?;
            args.push(Type::Sym(tpe));
        }
        args.push(Type::Bool); // return type (last one) is a boolean
        state_variables.push(StateFun { sym, tpe: args })
    }

    let mut context = Ctx::new(Arc::new(symbol_table), state_variables);

    // Initial chronicle construction
    let mut init_ch = Chronicle {
        kind: ChronicleKind::Problem,
        presence: true.into(),
        start: context.origin(),
        end: context.horizon(),
        name: vec![],
        task: None,
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
    };

    // Transforms atoms of an s-expression into the corresponding representation for chronicles
    let as_model_atom_no_borrow = |atom: &sexpr::SAtom, context: &Ctx| -> Result<SAtom> {
        let atom = context
            .model
            .symbols
            .id(atom.as_str())
            .ok_or_else(|| atom.invalid("Unknown atom"))?;
        let atom = context.typed_sym(atom);
        Ok(atom.into())
    };
    let as_model_atom = |atom: &sexpr::SAtom| as_model_atom_no_borrow(atom, &context);
    for goal in &prob.goal {
        let goals = read_conjunction(goal, as_model_atom)?;
        for goal in goals {
            match goal {
                Term::Binding(sv, value) => init_ch.conditions.push(Condition {
                    start: init_ch.end,
                    end: init_ch.end,
                    state_var: sv,
                    value,
                }),
            }
        }
    }
    // if we have negative preconditions, we need to assume a closed world assumption.
    // indeed, some preconditions might rely on initial facts being false
    let closed_world = dom.features.contains(&PddlFeature::NegativePreconditions);
    for (sv, val) in read_init(&prob.init, closed_world, as_model_atom, &context)? {
        init_ch.effects.push(Effect {
            transition_start: init_ch.start,
            persistence_start: init_ch.start,
            state_var: sv,
            value: val,
        });
    }

    if let Some(ref task_network) = &prob.task_network {
        read_task_network(
            &task_network,
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
        let template = read_chronicle_template(a, &mut context)?;
        templates.push(template);
    }
    for m in &dom.methods {
        let template = read_chronicle_template(m, &mut context)?;
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
) -> Result<Vec<(SV, Atom)>> {
    let mut facts = Vec::new();
    if closed_world {
        // closed world, every predicate that is not given a true value should be given a false value
        // to do this, we rely on the classical classical planning state
        let state_desc = World::new(context.model.symbols.deref().clone(), &context.state_functions)?;
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
            match read_term(e, &as_model_atom)? {
                Term::Binding(sv, val) => facts.push((sv, val)),
            }
        }
    }
    Ok(facts)
}

/// Transforms a PDDL action into a Chronicle template
fn read_chronicle_template(
    // pddl_action: &pddl::Action,
    pddl: impl ChronicleTemplateView,
    context: &mut Ctx,
) -> Result<ChronicleTemplate> {
    let top_type = OBJECT_TYPE.into();
    let mut params: Vec<Variable> = Vec::new();
    let prez = context.model.new_bvar("present");
    params.push(prez.into());
    let start = context.model.new_optional_ivar(0, INT_CST_MAX, prez, "start");
    params.push(start.into());

    // name of the chronicle : name of the action + parameters
    let mut name: Vec<SAtom> = Vec::with_capacity(1 + pddl.parameters().len());
    let base_name = pddl.base_name();
    name.push(
        context
            .typed_sym(
                context
                    .model
                    .symbols
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
            .symbols
            .types
            .id_of(tpe)
            .ok_or_else(|| tpe.invalid("Unknown atom"))?;
        let arg = context.model.new_optional_sym_var(tpe, prez, &arg.symbol);
        params.push(arg.into());
        name.push(arg.into());
    }

    // Transforms atoms of an s-expression into the corresponding representation for chronicles
    let as_chronicle_atom_no_borrow = |atom: &sexpr::SAtom, context: &Ctx| -> Result<SAtom> {
        match pddl
            .parameters()
            .iter()
            .position(|arg| arg.symbol.as_str() == atom.as_str())
        {
            Some(i) => Ok(name[i as usize + 1]),
            None => {
                let atom = context
                    .model
                    .symbols
                    .id(atom.as_str())
                    .ok_or_else(|| atom.invalid("Unknown atom"))?;
                let atom = context.typed_sym(atom);
                Ok(atom.into())
            }
        }
    };
    let as_chronicle_atom = |atom: &sexpr::SAtom| -> Result<SAtom> { as_chronicle_atom_no_borrow(atom, context) };

    let task = if let Some(task) = pddl.task() {
        let mut task_name = Vec::new();
        task_name.push(as_chronicle_atom(&task.name)?);
        for task_arg in &task.arguments {
            task_name.push(as_chronicle_atom(task_arg)?);
        }
        task_name
    } else {
        // no explicit task (typical for a primitive action), use the name as the task
        name.clone()
    };

    let mut ch = Chronicle {
        kind: pddl.kind(),
        presence: prez.into(),
        start: start.into(),
        end: start + 1,
        name: name.clone(),
        task: Some(task),
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
    };

    for eff in pddl.effects() {
        let effects = read_conjunction(eff, &as_chronicle_atom)?;
        for term in effects {
            match term {
                Term::Binding(sv, val) => ch.effects.push(Effect {
                    transition_start: ch.start,
                    persistence_start: ch.end,
                    state_var: sv,
                    value: val,
                }),
            }
        }
    }

    // a common pattern in PDDL is to have two effect (not x) et (x) on the same state variable.
    // this is to force mutual exclusion on x. The semantics of PDDL have the negative effect applied first.
    // This is already enforced by our translation of a positive effect on x as `]start, end] x = true`
    // Thus if we have both a positive effect and a negative effect on the same state variable,
    // we remove the negative one
    let positive_effects: HashSet<SV> = ch
        .effects
        .iter()
        .filter(|e| e.value == Atom::from(true))
        .map(|e| e.state_var.clone())
        .collect();
    ch.effects
        .retain(|e| e.value != Atom::from(false) || !positive_effects.contains(&e.state_var));

    for cond in pddl.preconditions() {
        let effects = read_conjunction(cond, &as_chronicle_atom)?;
        for term in effects {
            match term {
                Term::Binding(sv, val) => {
                    let as_effect_on_same_state_variable = ch
                        .effects
                        .iter()
                        .map(|e| e.state_var.as_slice())
                        .any(|x| x == sv.as_slice());
                    let end = if as_effect_on_same_state_variable {
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
            }
        }
    }

    if let Some(tn) = pddl.task_network() {
        read_task_network(tn, &as_chronicle_atom_no_borrow, &mut ch, Some(&mut params), context)?
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
    fn preconditions(&self) -> &[SExpr];
    fn effects(&self) -> &[SExpr];
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
    fn preconditions(&self) -> &[SExpr] {
        &self.pre
    }
    fn effects(&self) -> &[SExpr] {
        &self.eff
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
    fn preconditions(&self) -> &[SExpr] {
        &self.precondition
    }
    fn effects(&self) -> &[SExpr] {
        &[]
    }
    fn task_network(&self) -> Option<&pddl::TaskNetwork> {
        Some(&self.subtask_network)
    }
}

/// Parses a task network and adds its components (subtasks and constraints) to the target `chronicle.
/// All newly created variables (timepoints of the subtasks) are added to the new_variables buffer.
fn read_task_network(
    tn: &pddl::TaskNetwork,
    as_chronicle_atom: &impl Fn(&sexpr::SAtom, &Ctx) -> Result<SAtom>,
    chronicle: &mut Chronicle,
    mut new_variables: Option<&mut Vec<Variable>>,
    context: &mut Ctx,
) -> Result<()> {
    // stores the start/end timepoints of each named task
    let mut named_task: HashMap<String, (IVar, IVar)> = HashMap::new();

    let presence = chronicle.presence;
    // creates a new subtask. This will create new variables for the start and end
    // timepoints of the task and push the `new_variables` vector, if any.
    let mut make_subtask = |t: &pddl::Task| -> Result<SubTask> {
        let id = t.id.as_ref().map(|id| id.to_string());
        // get the name + parameters of the task
        let mut task_name = Vec::new();
        task_name.push(as_chronicle_atom(&t.name, &context)?);
        for param in &t.arguments {
            task_name.push(as_chronicle_atom(param, &context)?);
        }
        // create timepoints for the subtask
        let start = context.model.new_optional_ivar(0, INT_CST_MAX, presence, "task_start");
        let end = context.model.new_optional_ivar(0, INT_CST_MAX, presence, "task_end");
        if let Some(ref mut params) = new_variables {
            params.push(start.into());
            params.push(end.into());
        }
        if let Some(name) = id.as_ref() {
            named_task.insert(name.to_string(), (start, end));
        }
        Ok(SubTask {
            id,
            start: start.into(),
            end: end.into(),
            task: task_name,
        })
    };
    for t in &tn.unordered_tasks {
        let t = make_subtask(t)?;
        chronicle.subtasks.push(t);
    }

    // parse all ordered tasks, adding precedence constraints between subsequent ones
    let mut previous_end = None;
    for t in &tn.ordered_tasks {
        let t = make_subtask(t)?;

        if let Some(previous_end) = previous_end {
            chronicle.constraints.push(Constraint::lt(previous_end, t.start))
        }
        previous_end = Some(t.end);
        chronicle.subtasks.push(t);
    }
    for ord in &tn.orderings {
        let first_end = named_task
            .get(ord.first_task_id.as_str())
            .ok_or_else(|| ord.first_task_id.invalid("Unknown task id"))?
            .1;
        let second_start = named_task
            .get(ord.second_task_id.as_str())
            .ok_or_else(|| ord.second_task_id.invalid("Unknown task id"))?
            .0;
        chronicle.constraints.push(Constraint::lt(first_end, second_start));
    }

    Ok(())
}

enum Term {
    Binding(SV, Atom),
}

fn read_conjunction(e: &SExpr, t: impl Fn(&sexpr::SAtom) -> Result<SAtom>) -> Result<Vec<Term>> {
    let mut result = Vec::new();
    read_conjunction_impl(e, &t, &mut result)?;
    Ok(result)
}

fn read_conjunction_impl(e: &SExpr, t: &impl Fn(&sexpr::SAtom) -> Result<SAtom>, out: &mut Vec<Term>) -> Result<()> {
    if let Some(conjuncts) = e.as_application("and") {
        for c in conjuncts.iter() {
            read_conjunction_impl(c, t, out)?;
        }
    } else if let Some([to_negate]) = e.as_application("not") {
        let negated = match read_term(to_negate, &t)? {
            Term::Binding(sv, value) => {
                if let Ok(value) = BAtom::try_from(value) {
                    Term::Binding(sv, Atom::from(!value))
                } else {
                    return Err(to_negate.invalid("Could not apply 'not' to this expression").into());
                }
            }
        };
        out.push(negated);
    } else {
        // should be directly a predicate
        out.push(read_term(e, &t)?);
    }
    Ok(())
}

fn read_term(e: &SExpr, t: impl Fn(&sexpr::SAtom) -> Result<SAtom>) -> Result<Term> {
    let l = e.as_list_iter().ok_or_else(|| e.invalid("Expeced a term"))?;
    let mut sv = Vec::with_capacity(l.len());
    for e in l {
        let atom = e.as_atom().ok_or_else(|| e.invalid("Expected an atom"))?;
        let atom = t(atom)?;
        sv.push(atom);
    }
    Ok(Term::Binding(sv, true.into()))
}

fn read_sv(e: &SExpr, desc: &World) -> Result<SVId> {
    let p = e.as_list().context("Expected s-expression")?;
    let atoms: Result<Vec<_>, _> = p.iter().map(|e| e.as_atom().context("Expected atom")).collect();
    let atom_ids: Result<Vec<_>> = atoms?
        .iter()
        .map(|atom| {
            desc.table
                .id(atom.as_str())
                .with_context(|| format!("Unknown atom {}", atom.as_str()))
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
