mod ddl;
mod sexpr;

use crate::planning::chronicles::{
    Chronicle, ChronicleInstance, ChronicleTemplate, Condition, Ctx, Effect, Holed, Interval,
    Problem, StateVar, Time, Type, Var,
};
use crate::planning::classical::state::{Lit, World, SV};
use crate::planning::classical::{ActionTemplate, Arg, ParameterizedPred};
use crate::planning::parsing::ddl::{parse_pddl_domain, parse_pddl_problem};
use crate::planning::parsing::sexpr::Expr;
use crate::planning::symbols::{SymId, SymbolTable};
use crate::planning::typesystem::TypeHierarchy;
use streaming_iterator::StreamingIterator;

type Pb = Problem<String, String, Var>;

// TODO: this function still has some leftovers and pass through a classical reprensentation
//       for some processing steps
pub fn pddl_to_chronicles(dom: &str, prob: &str) -> Result<Pb, String> {
    let dom = parse_pddl_domain(dom)?;
    let prob = parse_pddl_problem(prob)?;

    // top types in pddl
    let mut types = vec![
        ("predicate".to_string(), None),
        ("action".to_string(), None),
        ("object".to_string(), None),
    ];
    for t in &dom.types {
        types.push((t.parent.clone(), Some(t.name.clone())));
    }

    let ts: TypeHierarchy<String> = TypeHierarchy::new(types).unwrap();
    let mut symbols: Vec<(String, String)> = prob
        .objects
        .iter()
        .map(|(name, tpe)| (name.clone(), tpe.clone().unwrap_or("object".to_string())))
        .collect();
    // predicates are symbols as well, add them to the table
    for p in &dom.predicates {
        symbols.push((p.name.clone(), "predicate".to_string()));
    }
    for a in &dom.actions {
        symbols.push((a.name.clone(), "action".to_string()));
    }

    let symbol_table = SymbolTable::new(ts, symbols)?;

    let mut state_variables = Vec::with_capacity(dom.predicates.len());
    for pred in &dom.predicates {
        let sym = symbol_table
            .id(&pred.name)
            .ok_or(format!("Unknown symbol {}", &pred.name))?;
        let mut args = Vec::with_capacity(pred.args.len() + 1);
        for a in &pred.args {
            let tpe = symbol_table
                .types
                .id_of(&a.tpe)
                .ok_or(format!("Unknown type {}", &a.tpe))?;
            args.push(Type::Symbolic(tpe));
        }
        args.push(Type::Boolean); // return type (last one) is a boolean
        state_variables.push(StateVar { sym, tpe: args })
    }

    let state_desc = World::new(symbol_table.clone(), &state_variables)?;
    let context = Ctx::new(symbol_table, state_variables);

    let mut s = state_desc.make_new_state();
    for init in prob.init.iter() {
        let pred = read_sv(init, &state_desc)?;
        s.add(pred);
    }

    //    println!("Initial state: {}", s.displayable(&state_desc));

    let mut actions: Vec<ActionTemplate> = Vec::new();
    for a in &dom.actions {
        let params: Vec<_> = a.args.iter().map(|a| a.name.clone()).collect();
        let pre = read_lits(&a.pre, params.as_slice(), &state_desc)?;
        let eff = read_lits(&a.eff, params.as_slice(), &state_desc)?;
        let template = ActionTemplate {
            name: a.name.clone(),
            params: a
                .args
                .iter()
                .map(|a| Arg {
                    name: a.name.clone(),
                    tpe: a.tpe.clone(),
                })
                .collect(),
            pre,
            eff,
        };
        actions.push(template);
    }

    let mut goals = Vec::new();
    for sub_goal in prob.goal.iter() {
        goals.append(&mut read_goal(sub_goal, &state_desc)?);
    }

    let sv_to_sv = |sv| -> Vec<Var> {
        state_desc
            .sv_of(sv)
            .iter()
            .map(|&sym| context.variable_of(sym))
            .collect()
    };
    // Initial chronical construction
    let mut init_ch = Chronicle {
        prez: context.tautology(),
        start: Time::new(context.origin()),
        end: Time::new(context.horizon()),
        name: vec![],
        conditions: vec![],
        effects: vec![],
    };
    for lit in s.literals() {
        let trans = Interval::new(init_ch.start, init_ch.start);
        let sv: Vec<Var> = sv_to_sv(lit.var());
        let val = if lit.val() {
            context.tautology()
        } else {
            context.contradiction()
        };
        init_ch.effects.push(Effect(trans, sv, val))
    }
    for &lit in &goals {
        let trans = Interval::new(init_ch.end, init_ch.end);
        let sv: Vec<Var> = sv_to_sv(lit.var());
        let val = if lit.val() {
            context.tautology()
        } else {
            context.contradiction()
        };
        init_ch.conditions.push(Condition(trans, sv, val))
    }
    let init_ch = ChronicleInstance {
        params: vec![],
        chronicle: init_ch,
    };
    let mut templates = Vec::new();
    let types = &state_desc.table.types;
    for a in &actions {
        let mut params = Vec::new();
        params.push((Type::Boolean, Some("prez".to_string())));
        params.push((Type::Time, Some("start".to_string())));
        for arg in &a.params {
            let tpe = types.id_of(&arg.tpe).unwrap();
            params.push((Type::Symbolic(tpe), Some(arg.name.clone())));
        }
        let start = Holed::Param(1);
        let mut name = Vec::with_capacity(1 + a.params.len());
        name.push(Holed::Full(
            context.variable_of(state_desc.table.id(&a.name).unwrap()),
        ));
        for i in 0..a.params.len() {
            name.push(Holed::Param(i + 2));
        }
        let mut ch = Chronicle {
            prez: Holed::Param(0),
            start: Time::new(start),
            end: Time::shifted(start, 1i64),
            name,
            conditions: vec![],
            effects: vec![],
        };
        let from_sexpr = |sexpr: &[Holed<SymId>]| -> Vec<_> {
            sexpr
                .iter()
                .map(|x| match x {
                    Holed::Param(i) => Holed::Param(*i as usize + 2),
                    Holed::Full(sym) => Holed::Full(context.variable_of(*sym)),
                })
                .collect()
        };
        for cond in &a.pre {
            let trans = Interval::new(ch.start, ch.start);
            let sv = from_sexpr(cond.sexpr.as_slice());
            let val = if cond.positive {
                context.tautology()
            } else {
                context.contradiction()
            };
            ch.conditions.push(Condition(trans, sv, Holed::Full(val)))
        }
        for eff in &a.eff {
            let trans = Interval::new(ch.start, ch.end);
            let sv = from_sexpr(eff.sexpr.as_slice());
            let val = if eff.positive {
                context.tautology()
            } else {
                context.contradiction()
            };
            ch.effects.push(Effect(trans, sv, Holed::Full(val)))
        }
        let template = ChronicleTemplate {
            label: Some(a.name.clone()),
            params: params,
            chronicle: ch,
        };
        templates.push(template);
    }

    let problem = Problem {
        context,
        templates,
        chronicles: vec![init_ch],
    };

    Ok(problem)
}

fn read_lits(
    e: &Expr<String>,
    params: &[String],
    desc: &World<String, String>,
) -> Result<Vec<ParameterizedPred>, String> {
    let mut res = Vec::new();
    if let Some(conjuncts) = e.as_application_args("and") {
        let subs = conjuncts.iter().map(|c| read_lits(c, params, desc));
        for sub_res in subs {
            res.append(&mut sub_res?);
        }
    } else if let Some([negated]) = e.as_application_args("not") {
        let mut x = as_parameterized_pred(negated, params, desc)?;
        x.positive = !x.positive;
        res.push(x);
    } else {
        // should be directly a predicate
        let x = as_parameterized_pred(e, params, desc)?;
        res.push(x);
    }
    Ok(res)
}

fn first_index<T: Eq>(slice: &[T], elem: &T) -> Option<usize> {
    slice
        .iter()
        .enumerate()
        .filter_map(|(i, e)| if e == elem { Some(i) } else { None })
        .next()
}

fn as_parameterized_pred(
    init: &Expr<String>,
    params: &[String],
    desc: &World<String, String>,
) -> Result<ParameterizedPred, String> {
    let mut res = Vec::new();
    let p = init.as_sexpr().expect("Expected s-expression");
    let atoms = p.iter().map(|e| e.as_atom().expect("Expected atom"));
    for a in atoms {
        let cur = match first_index(params, &a) {
            Some(arg_index) => Holed::Param(arg_index),
            None => Holed::Full(desc.table.id(a).ok_or(format!("Unknown atom: {}", &a))?),
        };
        res.push(cur)
    }

    Ok(ParameterizedPred {
        positive: true,
        sexpr: res,
    })
}

fn read_goal(e: &Expr<String>, desc: &World<String, String>) -> Result<Vec<Lit>, String> {
    let mut res = Vec::new();
    if let Some(conjuncts) = e.as_application_args("and") {
        let subs = conjuncts.iter().map(|c| read_goal(c, desc));
        for sub_res in subs {
            res.append(&mut sub_res?);
        }
    } else if let Some([negated]) = e.as_application_args("not") {
        let x = read_sv(negated, desc)?;

        res.push(Lit::new(x, false));
    } else {
        // should be directly a predicate
        let x = read_sv(e, desc)?;
        res.push(Lit::new(x, true));
    }
    Ok(res)
}

fn read_sv(e: &Expr<String>, desc: &World<String, String>) -> Result<SV, String> {
    let p = e
        .as_sexpr()
        .ok_or_else(|| "Expected s-expression".to_string())?;
    let atoms: Result<Vec<_>, _> = p
        .iter()
        .map(|e| e.as_atom().ok_or_else(|| "Expected atom".to_string()))
        .collect();
    let atom_ids: Result<Vec<_>, _> = atoms?
        .iter()
        .map(|atom| {
            desc.table
                .id(atom.as_str())
                .ok_or(format!("Unknown atom {}", atom))
        })
        .collect();

    desc.sv_id(atom_ids?.as_slice()).ok_or_else(|| {
        "Unknwon predicate (wrong number of arguments or badly typed args ?)".to_string()
    })
}
