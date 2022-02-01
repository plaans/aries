use crate::serialize::*;
use crate::serialize::{Answer_, Problem_};
use std::collections::HashSet;

use anyhow::{anyhow, Context, Error};
use core::fmt::Formatter;

use aries_model::extensions::Shaped;
use aries_planning::chronicles::*;
use aries_planning::parsing::pddl::TypedSymbol;

use anyhow::Result;
use aries_model::bounds::Lit;
use aries_model::lang::*;
use aries_model::symbols::{SymbolTable, TypedSym};
use aries_model::types::TypeHierarchy;
use aries_utils::input::Sym;
use std::fmt::Display;
use std::sync::Arc;

/// Names for built in types. They contain UTF-8 symbols for sexiness (and to avoid collision with user defined symbols)
static TASK_TYPE: &str = "★task★";
static ABSTRACT_TASK_TYPE: &str = "★abstract_task★";
static ACTION_TYPE: &str = "★action★";
static DURATIVE_ACTION_TYPE: &str = "★durative-action★";
static METHOD_TYPE: &str = "★method★";
static FLUENT_TYPE: &str = "★fluent★";
static OBJECT_TYPE: &str = "★object★";

// TODO: Replace panic with error

pub fn solve(problem: Problem_) -> Result<Answer_, Error> {
    let answer = Answer_::default();

    //Convert to chronicles
    let _chronicles = problem_to_chronicles(problem)?;
    Ok(answer)
}

//Convert Problem_ to chronicles
fn problem_to_chronicles(problem: Problem_) -> Result<Problem> {
    // top types in pddl
    let mut types: Vec<(Sym, Option<Sym>)> = vec![
        (TASK_TYPE.into(), None),
        (ABSTRACT_TASK_TYPE.into(), Some(TASK_TYPE.into())),
        (ACTION_TYPE.into(), Some(TASK_TYPE.into())),
        (DURATIVE_ACTION_TYPE.into(), Some(TASK_TYPE.into())),
        (METHOD_TYPE.into(), None),
        (FLUENT_TYPE.into(), None),
        (OBJECT_TYPE.into(), None),
    ];
    // let top_type = OBJECT_TYPE.into();

    // determine the top types in the user-defined hierarchy.
    // this is typically "object" by convention but might something else (e.g. "obj" in some hddl problems).
    let mut symbols: Vec<TypedSymbol> = vec![];
    {
        // TODO: Check if they are of top types in user hierarchy
        //Check if types are already in types
        for obj in problem.objects.clone() {
            let type_ = Sym::from(obj.type_.clone());
            let type_symbol = Sym::from(obj.name.clone());

            //check if type is already in types
            if !types.iter().any(|(t, _)| t == &type_) {
                types.push((type_, Some(OBJECT_TYPE.into())));
            }

            //add type to symbols
            symbols.push(TypedSymbol {
                symbol: type_symbol,
                tpe: Some(OBJECT_TYPE.into()),
            });
        }
    }

    let ts = TypeHierarchy::new(types)?;
    // TODO: currently, the protobuf does not allow defining a type hierarchy as in PDDL
    //       We should fix this in the protobuf and then import each type's parent un the hierarchy

    {
        // record all symbols representing fluents
        for fluent in problem.fluents.clone() {
            let predicate = Sym::from(fluent.name.clone());
            symbols.push(TypedSymbol {
                symbol: predicate,
                tpe: Some(FLUENT_TYPE.into()),
            });
        }

        // actions are symbols as well, add them to the table
        for action in problem.actions.clone() {
            let action_symbol = Sym::from(action.name.clone());
            symbols.push(TypedSymbol {
                symbol: action_symbol,
                tpe: Some(ACTION_TYPE.into()),
            });
        }
    }

    let symbols = symbols
        .drain(..)
        .map(|ts| (ts.symbol, ts.tpe.unwrap_or_else(|| OBJECT_TYPE.into())))
        .collect();
    let symbol_table = SymbolTable::new(ts.clone(), symbols)?;

    let from_upf_type = |name: &str| {
        if name == "bool" {
            Ok(Type::Bool)
        } else if name == "int" {
            Ok(Type::Int)
        } else if let Some(tpe) = ts.id_of(name) {
            Ok(Type::Sym(tpe))
        } else {
            Err(anyhow!("Unsupported type {}", name))
        }
    };

    let mut state_variables = vec![];
    for fluent in problem.fluents.clone() {
        let sym = symbol_table
            .id(&Sym::from(fluent.name.clone()))
            .unwrap_or_else(|| panic!("Fluent {} not found in symbol table", fluent.name));
        let mut args = Vec::with_capacity(1 + fluent.signature.len());

        for arg in &fluent.signature {
            args.push(
                from_upf_type(arg.as_str())
                    .with_context(|| format!("Invalid parameter type for fluent {}", fluent.name))?,
            );
        }

        args.push(
            from_upf_type(&fluent.value).with_context(|| format!("Invalid return type for fluent {}", fluent.name))?,
        );

        state_variables.push(StateFun { sym, tpe: args });
    }

    let mut context = Ctx::new(Arc::new(symbol_table.clone()), state_variables);

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
    };

    // Initial state translates as effect at the global start time
    for init_state in problem.initial_state {
        let expr = init_state
            .x
            .unwrap_or_else(|| panic!("Initial state has no valid expression"));
        let value = init_state
            .v
            .unwrap_or_else(|| panic!("Initial state has no valid value"));
        let expr = read_abstract(expr, symbol_table.clone())?;
        let value = read_abstract(value, symbol_table.clone())?;
        init_ch.effects.push(Effect {
            transition_start: init_ch.start,
            persistence_start: init_ch.start,
            state_var: expr.sv,
            value: value.output_value.unwrap(),
        })
    }

    // goals translate as condition at the global end time
    for goal in problem.goals {
        let goal = read_abstract(goal, symbol_table.clone())?;
        init_ch.conditions.push(Condition {
            start: init_ch.end,
            end: init_ch.end,
            state_var: goal.sv,
            value: goal.output_value.unwrap(),
        })
    }

    // TODO: Task networks?

    let init_ch = ChronicleInstance {
        parameters: vec![],
        origin: ChronicleOrigin::Original,
        chronicle: init_ch,
    };

    let mut templates = Vec::new();
    for a in problem.actions {
        let cont = Container::Template(templates.len());
        let template = read_chronicle_template(cont, a, &mut context)?;
        templates.push(template);
    }

    //TODO: Add methods and durative actions to the templates

    let problem = Problem {
        context,
        templates,
        chronicles: vec![init_ch],
    };

    Ok(problem)
}

//Convert chronicles to Answer_

pub enum AbstractType {
    Predicate(Abstract),
    Function(Abstract),
}

pub struct Abstract {
    sv: Vec<SAtom>,
    symbol: SAtom,
    operator: Option<Atom>,
    output_type: Option<Type>,
    output_value: Option<Atom>,
}

impl Display for Abstract {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "\nAbstract:\n")?;
        write!(f, "SV: {:?} ", self.sv)?;
        write!(f, "Operator: {:?}", self.operator)
    }
}

//TODO: Rewrite lame code
fn read_abstract(expr: Expression_, symbol_table: SymbolTable) -> Result<Abstract, Error> {
    //Parse expression in the format of abstract syntax tree
    let mut sv = Vec::new();
    let mut operator: Option<Atom> = None;
    let mut tpe: Option<Type> = None;
    let mut value: Option<Atom> = None;

    let payload_type = expr.clone().payload.unwrap().type_;
    let payload = expr.payload.unwrap().value.clone();
    let symbol = Sym::from(payload.clone());
    let symbol_atom = SAtom::from(TypedSym {
        sym: symbol_table.id(&symbol).unwrap(),
        // BUG: thread 'tokio-runtime-worker' panicked at 'called `Option::unwrap()` on a `None` value'
        tpe: symbol_table.types.id_of(&symbol).unwrap(),
    });
    sv.push(symbol_atom);

    //Check if symbol in symbol table
    for arg in expr.args {
        let abstract_ = read_abstract(arg, symbol_table.clone())?;
        sv.push(abstract_.symbol)
    }

    if !symbol_table.symbols.contains(&symbol) {
        if payload_type == "bool" {
            tpe = Some(Type::Bool);
            value = if symbol == (Sym::from("true")) {
                Some(Atom::Bool(true.into()))
            } else {
                Some(Atom::Bool(false.into()))
            };
        } else if payload_type == "int" {
            tpe = Some(Type::Int);
            value = Some(Atom::Int(payload.parse::<i32>().unwrap().into()));
        } else {
            operator = Some(Atom::Sym(symbol_atom));
        }
    }

    Ok(Abstract {
        sv,
        symbol: symbol_atom,
        operator,
        output_type: tpe,
        output_value: value,
    })
}

// TODO: Replace Action_ with Enum of Action, Method, and DurativeAction
fn read_chronicle_template(c: Container, action: Action_, context: &mut Ctx) -> Result<ChronicleTemplate> {
    let mut params: Vec<Variable> = Vec::new();
    let prez_var = context.model.new_bvar(c / VarType::Presence);
    params.push(prez_var.into());
    let prez = prez_var.true_lit();

    let start = context
        .model
        .new_optional_fvar(0, INT_CST_MAX, TIME_SCALE, prez, c / VarType::ChronicleStart);
    params.push(start.into());
    let start = FAtom::from(start);

    let end: FAtom = match action.kind() {
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

    let mut name: Vec<SAtom> = Vec::with_capacity(1 + action.parameters.len());
    let base_name = &Sym::from(action.name.clone());
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
    for arg in action.parameters {
        let arg = Sym::from(arg.clone());
        let tpe = context
            .model
            .get_symbol_table()
            .types
            .id_of(&arg)
            .ok_or_else(|| arg.invalid("Unknown atom"))?;
        let arg = context.model.new_optional_sym_var(tpe, prez, c / VarType::Parameter); // arg.symbol
        params.push(arg.into());
        name.push(arg.into());
    }

    let mut ch = Chronicle {
        kind: action.kind(),
        presence: prez,
        start,
        end,
        name: name.clone(),
        task: None,
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
        subtasks: vec![],
    };

    // Process the effects of the action
    for _eff in action.effects {
        let eff = _eff.x.unwrap();
        let eff = read_abstract(eff, context.model.get_symbol_table().clone())?;
        let eff_value = _eff.v.unwrap();
        let eff_value = read_abstract(eff_value, context.model.get_symbol_table().clone())?;
        ch.effects.push(Effect {
            transition_start: start,
            persistence_start: end,
            state_var: eff.sv,
            value: eff_value.output_value.unwrap(),
        });
    }

    let positive_effects: HashSet<Sv> = ch
        .effects
        .iter()
        .filter(|e| e.value == Atom::from(true))
        .map(|e| e.state_var.clone())
        .collect();
    ch.effects
        .retain(|e| e.value != Atom::from(false) || !positive_effects.contains(&e.state_var));

    for condition in action.preconditions {
        let condition = read_abstract(condition, context.model.get_symbol_table().clone())?;
        ch.conditions.push(Condition {
            start: start,
            end: end,
            state_var: condition.sv,
            value: condition.output_value.unwrap(),
        })
    }

    Ok(ChronicleTemplate {
        label: Some(action.name),
        parameters: params,
        chronicle: ch,
    })
}
