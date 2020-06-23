use crate::chronicles::{Ctx, Holed, Time, Type};
use crate::classical::state::{Lit, Operator, Operators, State, World};
use crate::ref_store::Ref;
use crate::symbols::SymId;
use crate::typesystem::TypeId;
use crate::utils::enumerate;
use anyhow::*;
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use streaming_iterator::StreamingIterator;

pub mod heuristics;
pub mod search;
pub mod state;

pub struct ParameterizedPred {
    pub positive: bool,
    pub sexpr: Vec<Holed<SymId>>,
}

impl ParameterizedPred {
    pub fn bind<T, S>(&self, sd: &World<T, S>, params: &[SymId], working: &mut Vec<SymId>) -> Option<Lit> {
        working.clear();
        for &x in &self.sexpr {
            let sym = match x {
                Holed::Param(i) => params[i],
                Holed::Full(s) => s,
            };
            working.push(sym);
        }
        sd.sv_id(working.as_slice()).map(|sv| Lit::new(sv, self.positive))
    }
}

#[derive(Debug, Clone)]
pub struct Arg {
    pub name: String,
    pub tpe: String,
}

// TODO : remove, superseeded by ActionSchema
pub struct ActionTemplate {
    pub name: String,
    pub params: Vec<Arg>,
    pub pre: Vec<ParameterizedPred>,
    pub eff: Vec<ParameterizedPred>,
}

pub struct ActionSchema {
    pub name: SymId,
    pub params: Vec<(TypeId, Option<String>)>,
    pub pre: Vec<ParameterizedPred>,
    pub eff: Vec<ParameterizedPred>,
}

pub struct LiftedProblem<T, I> {
    pub world: World<T, I>,
    pub initial_state: State,
    pub goals: Vec<Lit>,
    pub actions: Vec<ActionSchema>,
}

fn sv_to_lit<T, I, A: Ref>(variable: &[A], value: &A, world: &World<T, I>, ctx: &Ctx<T, I, A>) -> Result<Lit> {
    let sv: Result<Vec<SymId>, _> = variable
        .iter()
        .map(|var| ctx.sym_value_of(*var).context("Not a symbolic value"))
        .collect();
    let sv = sv?;
    let sv_id = world
        .sv_id(&sv)
        .context("No state variable identifed (maybe due to a typing error")?;
    if value == &ctx.tautology() {
        Ok(Lit::new(sv_id, true))
    } else if value == &ctx.contradiction() {
        Ok(Lit::new(sv_id, false))
    } else {
        bail!("state variable is not bound to a constant boolean")
    }
}

fn holed_sv_to_pred<T, I, A: Ref>(
    variable: &[Holed<A>],
    value: &Holed<A>,
    to_new_param: &HashMap<usize, usize>,
    ctx: &Ctx<T, I, A>,
) -> Result<ParameterizedPred> {
    let mut sv: Vec<Holed<SymId>> = Vec::new();
    for var in variable {
        let x = match var {
            Holed::Full(sym) => Holed::Full(ctx.sym_value_of(*sym).context("Not a symbolic value")?),
            Holed::Param(i) => Holed::Param(*to_new_param.get(i).context("Invalid parameter")?),
        };
        sv.push(x);
    }
    let value = if value == &Holed::Full(ctx.tautology()) {
        true
    } else if value == &Holed::Full(ctx.contradiction()) {
        false
    } else {
        bail!("state variable is not bound to a constant boolean");
    };
    Ok(ParameterizedPred {
        positive: value,
        sexpr: sv,
    })
}

pub fn from_chronicles<T, I, A: Ref>(chronicles: &crate::chronicles::Problem<T, I, A>) -> Result<LiftedProblem<T, I>>
where
    T: Clone + Eq + Hash + Display,
    I: Clone + Eq + Hash + Display,
{
    let symbols = chronicles.context.symbols.clone();

    let world = World::new(symbols, &chronicles.context.state_functions)?;
    let mut state = world.make_new_state();
    let mut goals = Vec::new();
    let ctx = &chronicles.context;
    for instance in &chronicles.chronicles {
        let ch = &instance.chronicle;
        ensure!(ch.presence == ctx.tautology(), "A chronicle instance is optional",);
        for eff in &ch.effects {
            ensure!(
                eff.effective_start() == eff.transition_start(),
                "Non instantaneous effect",
            );
            ensure!(
                *eff.effective_start() == ctx.origin(),
                "Effect not at start in initial chronicle",
            );
            let lit = sv_to_lit(eff.variable(), eff.value(), &world, ctx)?;
            state.set(lit);
        }
        for cond in &ch.conditions {
            ensure!(cond.start() == cond.end(), "Non instantaneous condition");
            ensure!(
                *cond.start() == ctx.horizon(),
                "Non final condition can not be interpreted as goal",
            );
            let lit = sv_to_lit(cond.variable(), cond.value(), &world, ctx)?;
            goals.push(lit);
        }
    }

    let mut schemas = Vec::new();
    for template in &chronicles.templates {
        let mut iter = template.chronicle.name.iter();
        let name = match iter.next() {
            Some(Holed::Full(id)) => ctx.sym_value_of(*id).context("Expected action symbol")?,
            _ => bail!("Unamed temlate"),
        };
        let global_start = Time::new(Holed::Full(ctx.origin()));
        let global_end = Time::new(Holed::Full(ctx.horizon()));
        ensure!(
            template.chronicle.start.partial_cmp(&global_start).is_none(),
            "action start is not free",
        );
        ensure!(
            template.chronicle.start.partial_cmp(&global_end).is_none(),
            "action start is not free",
        );
        ensure!(
            template.chronicle.start < template.chronicle.end,
            "More than one free timepoint in the action.",
        );

        // reconstruct parameters from chronicle name
        let mut parameters = Vec::new();
        // for each parameter of the chronicle, indicates its index in the parameters of the action
        let mut correspondance = HashMap::new();
        while let Some(x) = iter.next() {
            match x {
                Holed::Param(i) => {
                    let tpe = match template.parameters[*i].0 {
                        Type::Symbolic(tpe) => tpe,
                        _ => bail!("Non symbolic parameter"),
                    };
                    correspondance.insert(*i, parameters.len());
                    parameters.push((tpe, template.parameters[*i].1.clone()))
                }
                _ => bail!("Expected an action parameter but got an expression"),
            }
        }

        let mut schema = ActionSchema {
            name,
            params: parameters,
            pre: vec![],
            eff: vec![],
        };

        for cond in &template.chronicle.conditions {
            ensure!(cond.start() == cond.end(), "Non intantaneous condition");
            ensure!(
                *cond.start() == template.chronicle.start,
                "Non final condition can not be interpreted as goal",
            );
            let pred = holed_sv_to_pred(cond.variable(), cond.value(), &correspondance, ctx)?;
            schema.pre.push(pred);
        }
        for eff in &template.chronicle.effects {
            ensure!(
                eff.transition_start() == &template.chronicle.start,
                "Effect does not start condition with action's start",
            );
            ensure!(
                eff.effective_start() == &template.chronicle.end,
                "Effect is not active at action's end",
            );
            let pred = holed_sv_to_pred(eff.variable(), eff.value(), &correspondance, ctx)?;
            schema.eff.push(pred);
        }
        schemas.push(schema);
    }

    Ok(LiftedProblem {
        world,
        initial_state: state,
        goals,
        actions: schemas,
    })
}

pub struct GroundProblem {
    pub initial_state: State,
    pub operators: Operators,
    pub goals: Vec<Lit>,
}

pub fn grounded_problem<T, I>(lifted: &LiftedProblem<T, I>) -> Result<GroundProblem> {
    let mut operators = Operators::new();

    for template in &lifted.actions {
        let ops = ground_action_schema(template, &lifted.world)?;
        for op in ops {
            operators.push(op);
        }
    }

    Ok(GroundProblem {
        initial_state: lifted.initial_state.clone(),
        operators,
        goals: lifted.goals.clone(),
    })
}

fn ground_action_schema<T, I>(schema: &ActionSchema, desc: &World<T, I>) -> Result<Vec<Operator>> {
    let mut res = Vec::new();

    let mut arg_instances = Vec::with_capacity(schema.params.len());
    for arg in &schema.params {
        arg_instances.push(desc.table.instances_of_type(arg.0));
    }
    let mut params_iter = enumerate(arg_instances);
    while let Some(params) = params_iter.next() {
        let mut name = Vec::with_capacity(params.len() + 1);
        name.push(schema.name);
        params.iter().for_each(|p| name.push(*p));

        let mut op = Operator {
            name,
            precond: Vec::new(),
            effects: Vec::new(),
        };

        let mut working = Vec::new();

        for p in &schema.pre {
            let lit = p.bind(desc, params, &mut working).unwrap();
            op.precond.push(lit);
        }
        for eff in &schema.eff {
            let lit = eff.bind(desc, params, &mut working).unwrap();
            op.effects.push(lit);
        }
        res.push(op);
    }

    Ok(res)
}
