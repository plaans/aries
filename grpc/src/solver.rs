use crate::grpc::serialize::*;
use crate::grpc::serialize::{Answer_, Problem_};
use crate::parsing::pddl::TypedSymbol;

use anyhow::Error;

use crate::chronicles::*;

use anyhow::Result;
use aries_model::bounds::Lit;
use aries_model::lang::*;
use aries_model::symbols::SymbolTable;
use aries_model::types::TypeHierarchy;
use aries_utils::input::Sym;
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

// TODO: Replace panic with error

pub fn solve(problem: Problem_) -> Result<Answer_, Error> {
    let answer = Answer_::default();

    //Convert to chronicles
    let chronicles = problem_to_chronicles(problem)?;
    Ok(answer)
}

//Convert Problem_ to chronicles
pub fn problem_to_chronicles(problem: Problem_) -> Result<Problem> {
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
    let symbols: Vec<TypedSymbol> = vec![];
    {
        // TODO: Check if they are of top types in user hierrachy
        //Check if types are already in types
        for Obj in problem.objects {
            let type_ = Sym::from(Obj.type_.clone());
            let type_symbol = Sym::from(Obj.name.clone());

            //check if type is already in types
            if !types.iter().any(|(t, _)| t == &type_) {
                types.push((type_, Some(top_type)));
            }

            //add type to symbols
            symbols.push(TypedSymbol {
                symbol: type_symbol,
                tpe: Some(OBJECT_TYPE.into()),
            });
        }
    }

    let ts = TypeHierarchy::new(types)?;
    for fluent in problem.fluents {
        let predicate = Sym::from(fluent.name.clone());
        symbols.push(TypedSymbol {
            symbol: predicate,
            tpe: Some(PREDICATE_TYPE.into()),
        });
    }
    //TODO: Add function name are symbols too

    // actions are symbols as well, add them to the table
    for action in problem.actions {
        let action_symbol = Sym::from(action.name.clone());
        symbols.push(TypedSymbol {
            symbol: action_symbol,
            tpe: Some(ACTION_TYPE.into()),
        });
    }

    //TODO: Durative actions are symbols as well, add them to the table
    //TODO: Methods and tasks are symbols as well, add them to the table

    let symbols = symbols
        .drain(..)
        .map(|ts| (ts.symbol, ts.tpe.unwrap_or_else(|| OBJECT_TYPE.into())))
        .collect();
    let symbol_table = SymbolTable::new(ts, symbols)?;

    let mut state_variables = Vec::with_capacity(problem.fluents.len() + problem.objects.len() + problem.actions.len());
    for fluent in problem.fluents {
        let sym = symbol_table
            .id(&Sym::from(fluent.name.clone()))
            .unwrap_or_else(|| panic!("Fluent {} not found in symbol table", fluent.name));
        let mut args = Vec::with_capacity(1 + fluent.signature.len());

        for arg in fluent.signature {
            let arg_sym = ts
                .id_of(&Sym::from(arg.clone()))
                .unwrap_or_else(|| panic!("Fluent type {} not found in symbol table", arg));

            args.push(Type::Sym(arg_sym));
        }

        if fluent.value == "bool" {
            args.push(Type::Bool);
        } else if fluent.value == "int" {
            args.push(Type::Int);
        } else {
            //TODO: Add other types
            panic!(
                "Fluent {} has unknown type {} is not supported",
                fluent.name, fluent.value
            );
        }
        state_variables.push(StateFun { sym, tpe: args });
    }

    for obj in problem.objects {
        let sym = symbol_table
            .id(&Sym::from(obj.name.clone()))
            .unwrap_or_else(|| panic!("Object {} not found in symbol table", obj.name));
        let tpe = ts
            .id_of(&Sym::from(obj.type_.clone()))
            .unwrap_or_else(|| panic!("Object type {} not found in symbol table", obj.type_));
        let args = vec![Type::Sym(tpe)];
        // TODO: Recover duplicated object types

        state_variables.push(StateFun { sym, tpe: args });
    }

    for action in problem.actions {
        let sym = symbol_table
            .id(&Sym::from(action.name.clone()))
            .unwrap_or_else(|| panic!("Action {} not found in symbol table", action.name));
        let mut args = Vec::with_capacity(action.parameters.len());

        // Map parameters types to types
        for tpe in action.parameter_types {
            let tpe = ts
                .id_of(&Sym::from(tpe.clone()))
                .unwrap_or_else(|| panic!("Action parameter type {} not found in symbol table", tpe));
            args.push(Type::Sym(tpe));
        }

        state_variables.push(StateFun { sym, tpe: args });
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
    };

    for goal in problem.goals {
        let goals = read_expressions(goal)?;
        for goal in goals {
            match goal {
                Term::Binding(sv, value) => init_ch.conditions.push(Condition {
                    start: init_ch.end,
                    end: init_ch.end,
                    state_var: sv,
                    value,
                }),
                _ => return Err(Error::new(format!("Goal {} is not a binding", goal))),
            }
        }
    }

    // TODO: Task networks?

    let init_ch = ChronicleInstance {
        parameters: vec![],
        origin: ChronicleOrigin::Original,
        chronicle: init_ch,
    };

    let mut templates = Vec::new();
    // for a in problem.actions {
    //     let cont = Container::Template(templates.len());
    //     let template = read_chronicle_template(cont, &a, &mut context)?;
    //     templates.push(template);
    // }

    //TODO: Add methods and durative actions to the templates

    let problem = Problem {
        context,
        templates,
        chronicles: vec![init_ch],
    };

    Ok(problem)
}

//Convert chronicles to Answer_

pub fn read_expressions(expr: Expression_) -> Result<Vec<Term>, Error> {
    let terms: Vec<Term> = vec![];
    Ok(terms)
}

enum Term {
    Binding(Sv, Atom),
    Eq(Atom, Atom),
    Neq(Atom, Atom),
}
