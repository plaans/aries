
use aries::chronicles::ddl::*;
use aries::chronicles::typesystem::TypeHierarchy;
use aries::chronicles::state::{StateDesc, PredicateDesc, Operator};
use aries::chronicles::strips::{SymbolTable, ActionTemplate, ParameterizedPred, ParamOrSym};
use aries::chronicles::sexpr::Expr;


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
        ("object".to_string(), None)
    ];
    for t in &dom.types {
        types.push((t.parent.clone(), Some(t.name.clone())));
    }


    let ts: TypeHierarchy<String> = TypeHierarchy::new(types).unwrap();
    let mut symbols: Vec<(String,String)> = prob.objects.iter()
        .map(|(name, tpe)| (name.clone(), tpe.clone().unwrap_or("object".to_string())))
        .collect();
    for p in &dom.predicates {
        symbols.push((p.name.clone(), "predicate".to_string()));
    }
    let symbol_table = SymbolTable::new(ts, symbols)?;

    let preds = dom.predicates.iter().map(|pred| {
        PredicateDesc {
            name: pred.name.clone(),
            types: pred.args.iter().map(|a| a.tpe.clone()).collect()
        }
    }).collect();

    let state_desc = StateDesc::new(symbol_table, preds)?;

    let mut s = state_desc.make_new_state();
    for init in prob.init.clone() {
        let mut p = init.into_sexpr().expect("Expected s-expression");
        let atoms :Vec<String> = p.drain(..).map(|e| e.into_atom().expect("Expected atom")).collect();
        let atom_ids: Vec<_> = atoms.iter()
            .map(|atom| state_desc.table.id(atom).expect("Unknown atom"))
            .collect();
        let pred = state_desc.sv_id(atom_ids.as_slice()).expect("Unknwon predicate (wrong number of arguments or badly typed args ?)");
        s.add(pred);
    }

    println!("Initial state: {}", s.displayable(&state_desc));


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

    for template in &actions {
        let ops = ground(template, &state_desc)?;
        for op in &ops {
            println!("{}", op.name);
        }
    }




    println!("OK");
    Ok(())
}

use Expr::{Leaf,SExpr};
use aries::chronicles::enumerate::enumerate;
use streaming_iterator::StreamingIterator;

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
        let mut subs = conjuncts.iter().map(|c| read_lits(c, params, desc));
        for mut sub_res in subs {
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
    let mut p = init.as_sexpr().expect("Expected s-expression");
    let atoms = p.iter().map(|e| e.as_atom().expect("Expected atom"));
    for a in atoms {
        let cur = match first_index(params, &a) {
            Some(arg_index) => ParamOrSym::Param(arg_index as u32),
            None => ParamOrSym::Sym(
                desc.table.id(&a)
                    .ok_or(format!("Unknown atom: {}", &a))?)
        };
        res.push(cur)
    }

    Ok(ParameterizedPred {
        positive: true,
        sexpr: res
    })
}