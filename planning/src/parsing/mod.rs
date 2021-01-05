mod ddl;
mod sexpr;

use crate::chronicles::*;
use crate::classical::state::{Lit, SVId, World};
use crate::classical::{ActionTemplate, Arg, Holed, ParameterizedPred};
use crate::parsing::ddl::{parse_pddl_domain, parse_pddl_problem, Expression};
use crate::parsing::sexpr::Expr;
use anyhow::*;
use aries_model::lang::*;
use aries_model::symbols::{SymId, SymbolTable};
use aries_model::types::TypeHierarchy;
use std::borrow::Borrow;
use std::collections::HashSet;
use std::sync::Arc;

type Pb = Problem;

// TODO: this function still has some leftovers and passes through a classical representation
//       for some processing steps
pub fn pddl_to_chronicles(dom: &str, prob: &str) -> Result<Pb> {
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

    let ts: TypeHierarchy<String> = TypeHierarchy::new(types)?;
    let mut symbols: Vec<(String, String)> = prob
        .objects
        .iter()
        .map(|(name, tpe)| (name.clone(), tpe.clone().unwrap_or_else(|| "object".to_string())))
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
            .with_context(|| format!("Unknown symbol {}", &pred.name))?;
        let mut args = Vec::with_capacity(pred.args.len() + 1);
        for a in &pred.args {
            let tpe = symbol_table
                .types
                .id_of(&a.tpe)
                .with_context(|| format!("Unknown type {}", &a.tpe))?;
            args.push(Type::Sym(tpe));
        }
        args.push(Type::Bool); // return type (last one) is a boolean
        state_variables.push(StateFun { sym, tpe: args })
    }

    let state_desc = World::new(symbol_table.clone(), &state_variables)?;
    let mut context = Ctx::new(Arc::new(symbol_table), state_variables);

    let mut s = state_desc.make_new_state();
    for init in prob.init.iter() {
        let pred = read_sv(init, &state_desc)?;
        s.add(pred);
    }

    //    println!("Initial state: {}", s.displayable(&state_desc));

    let mut actions: Vec<ActionTemplate> = Vec::new();
    for a in &dom.actions {
        let params: Vec<_> = a.args.iter().map(|a| a.symbol.clone()).collect();
        let mut pre = Vec::new();
        for p in &a.pre {
            read_lits(p, params.as_slice(), &state_desc, &mut pre)?;
        }
        let mut eff = Vec::new();
        for e in &a.eff {
            read_lits(e, params.as_slice(), &state_desc, &mut eff)?;
        }
        let template = ActionTemplate {
            name: a.name.clone(),
            params: a
                .args
                .iter()
                .map(|a| Arg {
                    name: a.symbol.clone(),
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

    let sv_to_sv = |sv| -> Vec<SAtom> {
        state_desc
            .sv_of(sv)
            .iter()
            .map(|&sym| context.typed_sym(sym).into())
            .collect()
    };
    // Initial chronicle construction
    let mut init_ch = Chronicle {
        presence: true.into(),
        start: context.origin(),
        end: context.horizon(),
        name: vec![],
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
    };
    for lit in s.literals() {
        let sv = sv_to_sv(lit.var());

        init_ch.effects.push(Effect {
            transition_start: init_ch.start,
            persistence_start: init_ch.start,
            state_var: sv,
            value: lit.val().into(),
        });
    }
    for &lit in &goals {
        let sv = sv_to_sv(lit.var());

        init_ch.conditions.push(Condition {
            start: init_ch.end,
            end: init_ch.end,
            state_var: sv,
            value: lit.val().into(),
        });
    }
    let init_ch = ChronicleInstance {
        parameters: vec![],
        origin: ChronicleOrigin::Original,
        chronicle: init_ch,
    };
    let mut templates = Vec::new();
    let types = &state_desc.table.types;
    for a in &actions {
        let mut params: Vec<Variable> = Vec::new();
        let prez = context.model.new_bvar("present");
        params.push(prez.into());
        let start = context.model.new_optional_ivar(0, IntCst::MAX, prez, "start");
        params.push(start.into());

        let mut name: Vec<SAtom> = Vec::with_capacity(1 + a.params.len());
        name.push(context.typed_sym(state_desc.table.id(&a.name).unwrap()).into());

        for arg in &a.params {
            let tpe = types.id_of(&arg.tpe).unwrap();
            let arg = context.model.new_optional_sym_var(tpe, prez, &arg.name);
            params.push(arg.into());
            name.push(arg.into());
        }
        let mut ch = Chronicle {
            presence: prez.into(),
            start: start.into(),
            end: start + 1,
            name: name.clone(),
            conditions: vec![],
            effects: vec![],
            constraints: vec![],
        };
        let from_sexpr = |sexpr: &[Holed<SymId>]| -> Vec<SAtom> {
            sexpr
                .iter()
                .map(|x| match x {
                    Holed::Param(i) => name[*i as usize + 1],
                    Holed::Full(sym) => context.typed_sym(*sym).into(),
                })
                .collect()
        };

        for eff in &a.eff {
            let sv = from_sexpr(eff.sexpr.as_slice());
            let val = eff.positive.into();
            ch.effects.push(Effect {
                transition_start: ch.start,
                persistence_start: ch.end,
                state_var: sv,
                value: val,
            });
        }

        // a common pattern in PDDL is to have two effect (not x) et (x) on the same state variable.
        // this is to force mutual exclusion on x. The semantics of PDDL have the negative effect applied first.
        // This is already enforced by our translation of a positive effect on x as `]start, end] x = true`
        // Thus if we have both a positive effect and a negative effect on the same state variable,
        // we remove the negative one
        let positive_effects: HashSet<_> = ch
            .effects
            .iter()
            .filter(|e| e.value == Atom::from(true))
            .map(|e| e.state_var.clone())
            .collect();
        ch.effects
            .retain(|e| e.value != Atom::from(false) || !positive_effects.contains(&e.state_var));

        for cond in &a.pre {
            let sv = from_sexpr(cond.sexpr.as_slice());
            let val = Atom::from(cond.positive);
            // end time of this conditions, depends on the presence of an effect
            let end = if ch
                .effects
                .iter()
                .map(|e| e.state_var.as_slice())
                .any(|x| x == sv.as_slice())
            {
                // there is corresponding effect
                ch.start
            } else {
                // no effect, condition needs to persist until the end of the action
                ch.end
            };
            ch.conditions.push(Condition {
                start: ch.start,
                end,
                state_var: sv,
                value: val,
            });
        }
        let template = ChronicleTemplate {
            label: Some(a.name.clone()),
            parameters: params,
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

/// Extract literals that appear in a conjunctive form in `e` and writes them to
/// the output vector `out`
fn read_lits(
    e: &Expression,
    params: &[String],
    desc: &World<String, String>,
    out: &mut Vec<ParameterizedPred>,
) -> Result<()> {
    if let Some(conjuncts) = e.as_application_args("and") {
        for c in conjuncts.iter() {
            read_lits(c, params, desc, out)?;
        }
    } else if let Some([negated]) = e.as_application_args("not") {
        let mut x = as_parameterized_pred(negated, params, desc)?;
        x.positive = !x.positive;
        out.push(x);
    } else {
        // should be directly a predicate
        let x = as_parameterized_pred(e, params, desc)?;
        out.push(x);
    }
    Ok(())
}

fn first_index<T, X: Eq + ?Sized>(slice: &[T], elem: &X) -> Option<usize>
where
    T: Borrow<X>,
{
    slice
        .iter()
        .enumerate()
        .filter_map(|(i, e)| if e.borrow() == elem { Some(i) } else { None })
        .next()
}

fn as_parameterized_pred<'a>(
    init: &Expression,
    params: &[String],
    desc: &World<String, String>,
) -> Result<ParameterizedPred> {
    let mut res = Vec::new();
    let p = init.as_list().context("Expected s-expression")?;
    let atoms = p.iter().map(|e| e.as_atom().expect("Expected atom")); // TODO: we might throw here
    for a in atoms {
        let cur = match first_index(params, a) {
            Some(arg_index) => Holed::Param(arg_index),
            None => Holed::Full(desc.table.id(a).with_context(|| format!("Unknown atom: {}", &a))?),
        };
        res.push(cur)
    }

    Ok(ParameterizedPred {
        positive: true,
        sexpr: res,
    })
}

fn read_goal(e: &Expression, desc: &World<String, String>) -> Result<Vec<Lit>> {
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

fn read_sv<'a>(e: &Expression, desc: &World<String, String>) -> Result<SVId> {
    let p = e.as_list().context("Expected s-expression")?;
    let atoms: Result<Vec<_>, _> = p.iter().map(|e| e.as_atom().context("Expected atom")).collect();
    let atom_ids: Result<Vec<_>> = atoms?
        .iter()
        .map(|atom| desc.table.id(*atom).with_context(|| format!("Unknown atom {}", atom)))
        .collect();
    let atom_ids = atom_ids?;
    desc.sv_id(atom_ids.as_slice()).with_context(|| {
        format!(
            "Unknown predicate {} (wrong number of arguments or badly typed args ?)",
            desc.table.format(&atom_ids)
        )
    })
}
