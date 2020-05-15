
use aries::planning::ddl::*;
use aries::planning::typesystem::TypeHierarchy;
use aries::planning::state::{StateDesc, PredicateDesc, Operator, Operators, SV, Lit};
use aries::planning::strips::{SymbolTable, ActionTemplate, ParameterizedPred, ParamOrSym};
use aries::planning::sexpr::Expr;
use aries::planning::heuristics::ConjunctiveCost;

fn main() -> Result<(), String> {
    let dom = std::fs::read_to_string("problems/pddl/gripper/domain.pddl")
        .map_err(|o| format!("{}", o))?;
    let dom = parse_pddl_domain(dom.as_str())?;


    let prob = std::fs::read_to_string("problems/pddl/gripper/problem.pddl")
        .map_err(|o| format!("{}", o))?;
    let prob = parse_pddl_problem(prob.as_str())?;

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
    let mut symbols: Vec<(String,String)> = prob.objects.iter()
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
    let context = Ctx::new(symbol_table.clone());

    let preds = dom.predicates.iter().map(|pred| {
        PredicateDesc {
            name: pred.name.clone(),
            types: pred.args.iter().map(|a| a.tpe.clone()).collect()
        }
    }).collect();

    let state_desc = StateDesc::new(symbol_table, preds)?;

    let mut s = state_desc.make_new_state();
    for init in prob.init.iter() {
        let pred = read_sv(init, &state_desc)?;
        s.add(pred);
    }

//    println!("Initial state: {}", s.displayable(&state_desc));


    let mut actions :Vec<ActionTemplate> = Vec::new();
    for a in &dom.actions {
        let params: Vec<_> = a.args.iter().map(|a| a.name.clone()).collect();
        let pre = read_lits(&a.pre, params.as_slice(), &state_desc)?;
        let eff = read_lits(&a.eff, params.as_slice(), &state_desc)?;
        let template = ActionTemplate {
            name: a.name.clone(),
            params: a.args.clone(),
            pre,
            eff
        };
        actions.push(template);
    }

    let mut operators = Operators::new();

    for template in &actions {
        let ops = ground(template, &state_desc)?;
        for op in ops {
//            println!("{}", op.name);
            operators.push(op);
        }
    }

    let hadd = hadd(&s, &operators);

//    for &op in hadd.applicable_operators() {
//        println!("{}", operators.name(op));
//    }

    let mut goals = Vec::new();
    for sub_goal in prob.goal.iter() {
        goals.append(&mut read_goal(sub_goal, &state_desc)?);
    }

    let sv_to_sv = |sv| -> Vec<Var> {
        state_desc.sv_of(sv).iter()
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
        effects: vec![]
    };
    for lit in s.literals() {
        let trans = Interval(init_ch.start, init_ch.start);
        let sv: Vec<Var> = sv_to_sv(lit.var());
        let val = if lit.val() { context.tautology() } else { context.contradiction() };
        init_ch.effects.push(Effect(trans, sv, val))
    }
    for &lit in &goals {
        let trans = Interval(init_ch.end, init_ch.end);
        let sv: Vec<Var> = sv_to_sv(lit.var());
        let val = if lit.val() { context.tautology() } else { context.contradiction() };
        init_ch.conditions.push(Condition(trans, sv, val))
    }
    let init_ch = ChronicleInstance {
        params: vec![],
        chronicle: init_ch
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
        name.push(Holed::Full(context.variable_of(state_desc.table.id(&a.name).unwrap())));
        for i in 0..a.params.len() {
            name.push(Holed::Param(i + 2));
        }
        let mut ch = Chronicle {
            prez: Holed::Param(0),
            start: Time::new(start),
            end: Time::shifted(start, 1i64),
            name,
            conditions: vec![],
            effects: vec![]
        };
        let from_sexpr = |sexpr: &[ParamOrSym]| -> Vec<_> {
            sexpr.iter().map(|x| match x {
                ParamOrSym::Param(i) => Holed::Param(*i as usize + 2),
                ParamOrSym::Sym(sym) => Holed::Full(context.variable_of(*sym))
            }).collect()
        };
        for cond in &a.pre {
            let trans = Interval(ch.start, ch.start);
            let sv = from_sexpr(cond.sexpr.as_slice());
            let val = if cond.positive { context.tautology() } else { context.contradiction() };
            ch.conditions.push(Condition(trans, sv, Holed::Full(val)))
        }
        for eff in &a.eff {
            let trans = Interval(ch.start, ch.end);
            let sv = from_sexpr(eff.sexpr.as_slice());
            let val = if eff.positive { context.tautology() } else { context.contradiction() };
            ch.effects.push(Effect(trans, sv, Holed::Full(val)))
        }
        let template = ChronicleTemplate {
            label: Some(a.name.clone()),
            params: params,
            chronicle: ch
        };
        templates.push(template);
    }

    let problem = aries::planning::chronicles::Problem {
        context,
        templates,
        chronicles: vec![init_ch]
    };

    let h = hadd.conjunction_cost(goals.as_slice());
    println!("Initial heuristic value (hadd): {}", h);

    match plan_search(&s, &operators, goals.as_slice()) {
        Some(plan) => {
            println!("Got plan: {} actions", plan.len());
            println!("=============");
            for &op in &plan {
                println!("{}", operators.name(op));
            }
        },
        None => println!("Infeasible")
    }

    Ok(())
}

use aries::planning::enumerate::enumerate;
use streaming_iterator::StreamingIterator;
use aries::planning::heuristics::hadd;
use aries::planning::search::plan_search;
use aries::planning::chronicles::{Ctx, Chronicle, Time, Effect, Var, Interval, Condition, Holed, Type, ChronicleTemplate, ChronicleInstance};

fn ground(template: &ActionTemplate, desc: &StateDesc<String,String>) -> Result<Vec<Operator>, String> {
    let mut res = Vec::new();

    let mut arg_instances = Vec::with_capacity(template.params.len());
    for arg in &template.params {
        let x = desc.table.types.id_of(&arg.tpe).ok_or(format!("Unknown type: {}", &arg.tpe))?;
        arg_instances.push(desc.table.instances_of_type(x));
    }
    let mut params_iter = enumerate(arg_instances);
    while let Some(params) = params_iter.next() {
        let mut name = "(".to_string();
        name.push_str(&template.name);
        for &arg in params {
            name.push(' '); name.push_str(desc.table.symbol(arg));
        }
        name.push(')');
        
        let mut op = Operator {
            name,
            precond: Vec::new(),
            effects: Vec::new()
        };

        let mut working = Vec::new();

        for p in &template.pre {
            let lit = p.bind(desc, params, &mut working).unwrap();
            op.precond.push(lit);
        }
        for eff in &template.eff {
            let lit = eff.bind(desc, params, &mut working).unwrap();
            op.effects.push(lit);
        }
        res.push(op);
    }

    Ok(res)
}

fn read_lits(e: &Expr<String>, params: &[String], desc: &StateDesc<String,String>) -> Result<Vec<ParameterizedPred>, String> {
    let mut res = Vec::new();
    if let Some(conjuncts) = e.as_application_args("and") {
        let subs = conjuncts.iter().map(|c| read_lits(c, params, desc));
        for sub_res in subs {
            res.append(&mut sub_res?);
        }
    } else if let Some([negated]) = e.as_application_args("not"){
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
    slice.iter().enumerate()
        .filter_map(|(i, e)| if e == elem { Some(i) } else {None})
        .next()
}

fn as_parameterized_pred(init: &Expr<String>, params: &[String], desc: &StateDesc<String,String>) -> Result<ParameterizedPred, String> {
    let mut res = Vec::new();
    let p = init.as_sexpr().expect("Expected s-expression");
    let atoms = p.iter().map(|e| e.as_atom().expect("Expected atom"));
    for a in atoms {
        let cur = match first_index(params, &a) {
            Some(arg_index) => ParamOrSym::Param(arg_index as u32),
            None => ParamOrSym::Sym(
                desc.table.id(a)
                    .ok_or(format!("Unknown atom: {}", &a))?)
        };
        res.push(cur)
    }

    Ok(ParameterizedPred {
        positive: true,
        sexpr: res
    })
}

fn read_goal(e: &Expr<String>, desc: &StateDesc<String,String>) -> Result<Vec<Lit>, String> {
    let mut res = Vec::new();
    if let Some(conjuncts) = e.as_application_args("and") {
        let subs = conjuncts.iter().map(|c| read_goal(c, desc));
        for sub_res in subs {
            res.append(&mut sub_res?);
        }
    } else if let Some([negated]) = e.as_application_args("not"){
        let x = read_sv(negated, desc)?;

        res.push(Lit::new(x, false));
    } else {
        // should be directly a predicate
        let x = read_sv(e, desc)?;
        res.push(Lit::new(x, true));
    }
    Ok(res)
}

// TODO: many exception throw here
fn read_sv(e :&Expr<String>, desc: &StateDesc<String,String>) -> Result<SV, String> {
    let p = e.as_sexpr().expect("Expected s-expression");
    let atoms = p.iter().map(|e| e.as_atom().expect("Expected atom"));
    let atom_ids: Vec<_> = atoms
        .map(|atom| desc.table.id(atom).expect("Unknown atom"))
        .collect();
    let pred = desc.sv_id(atom_ids.as_slice()).expect("Unknwon predicate (wrong number of arguments or badly typed args ?)");

    Ok(pred)
}


