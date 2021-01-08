pub mod pddl;
pub mod sexpr;

use crate::chronicles::*;
use crate::classical::state::{SVId, World};
use crate::parsing::pddl::{PddlFeature, TypedSymbol};

use crate::parsing::sexpr::SExpr;
use anyhow::*;
use aries_model::lang::*;
use aries_model::symbols::SymbolTable;
use aries_model::types::TypeHierarchy;
use aries_utils::input::Sym;
use std::collections::HashSet;
use std::convert::TryFrom;
use std::ops::Deref;
use std::sync::Arc;

type Pb = Problem;

pub fn pddl_to_chronicles(dom: &pddl::Domain, prob: &pddl::Problem) -> Result<Pb> {
    // determine the top object type, this is typically "object" by convention but might something else (e.g. "obj" in some hddl problems.
    let top_object_type = {
        let all_types: HashSet<&Sym> = dom.types.iter().map(|tpe| &tpe.symbol).collect();
        let top_types: HashSet<&Sym> = dom
            .types
            .iter()
            .filter_map(|tpe| tpe.tpe.as_ref())
            .filter(|tpe| !all_types.contains(tpe))
            .collect();
        if top_types.len() > 1 {
            bail!("More than one top types in problem definition: {:?}", &top_types);
        } else {
            match top_types.iter().next() {
                None => Sym::new("object"),
                Some(&top) => top.clone(),
            }
        }
    };

    // top types in pddl
    let mut types: Vec<(Sym, Option<Sym>)> = vec![
        ("predicate".into(), None),
        ("action".into(), None),
        (top_object_type.clone(), None),
    ];
    for t in &dom.types {
        types.push((t.symbol.clone(), t.tpe.clone()));
    }

    let ts: TypeHierarchy<Sym> = TypeHierarchy::new(types)?;
    let mut symbols: Vec<TypedSymbol> = prob.objects.clone();
    // predicates are symbols as well, add them to the table
    for p in &dom.predicates {
        symbols.push(TypedSymbol::new(&p.name, "predicate"));
    }
    for a in &dom.actions {
        symbols.push(TypedSymbol::new(&a.name, "action"));
    }
    let symbols = symbols
        .drain(..)
        .map(|ts| (ts.symbol, ts.tpe.unwrap_or_else(|| top_object_type.clone())))
        .collect();
    let symbol_table = SymbolTable::new(ts, symbols)?;

    let mut state_variables = Vec::with_capacity(dom.predicates.len());
    for pred in &dom.predicates {
        let sym = symbol_table
            .id(&pred.name)
            .with_context(|| format!("Unknown symbol {}", &pred.name))?;
        let mut args = Vec::with_capacity(pred.args.len() + 1);
        for a in &pred.args {
            let tpe = a.tpe.as_ref().unwrap_or(&top_object_type);
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
        presence: true.into(),
        start: context.origin(),
        end: context.horizon(),
        name: vec![],
        conditions: vec![],
        effects: vec![],
        constraints: vec![],
    };

    // Transforms atoms of an s-expression into the corresponding representation for chronicles
    let as_model_atom = |atom: &sexpr::SAtom| -> Result<SAtom> {
        let atom = context
            .model
            .symbols
            .id(atom.as_str())
            .ok_or_else(|| atom.invalid("Unknown atom"))?;
        let atom = context.typed_sym(atom);
        Ok(atom.into())
    };
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

    let init_ch = ChronicleInstance {
        parameters: vec![],
        origin: ChronicleOrigin::Original,
        chronicle: init_ch,
    };

    let mut templates = Vec::new();
    for a in &dom.actions {
        let template = read_action(a, &mut context, &top_object_type)?;
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
fn read_action(pddl_action: &pddl::Action, context: &mut Ctx, top_object_type: &Sym) -> Result<ChronicleTemplate> {
    let mut params: Vec<Variable> = Vec::new();
    let prez = context.model.new_bvar("present");
    params.push(prez.into());
    let start = context.model.new_optional_ivar(0, IntCst::MAX, prez, "start");
    params.push(start.into());

    // name of the chronicle : name of the action + parameters
    let mut name: Vec<SAtom> = Vec::with_capacity(1 + pddl_action.args.len());
    name.push(
        context
            .typed_sym(context.model.symbols.id(&pddl_action.name).unwrap())
            .into(),
    );

    // Process, the arguments of the action, adding them to the parameters of the chronicle and to the name of the action
    for arg in &pddl_action.args {
        let tpe = arg.tpe.as_ref().unwrap_or(top_object_type);
        let tpe = context.model.symbols.types.id_of(tpe).unwrap(); // TODO: error message
        let arg = context.model.new_optional_sym_var(tpe, prez, &arg.symbol);
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

    // Transforms atoms of an s-expression into the corresponding representation for chronicles
    let as_chronicle_atom = |atom: &sexpr::SAtom| -> Result<SAtom> {
        match pddl_action
            .args
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

    for eff in &pddl_action.eff {
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

    for cond in &pddl_action.pre {
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
    let template = ChronicleTemplate {
        label: Some(pddl_action.name.to_string()),
        parameters: params,
        chronicle: ch,
    };
    Ok(template)
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

fn read_sv(e: &SExpr, desc: &World<Sym, Sym>) -> Result<SVId> {
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
