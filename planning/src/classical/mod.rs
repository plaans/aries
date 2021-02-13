use crate::chronicles::*;
use crate::classical::state::{Lit, Operator, Operators, State, World};
use anyhow::*;

use aries_model::lang::*;
use aries_model::symbols::SymId;
use aries_model::types::TypeId;
use aries_utils::enumerate;
use aries_utils::input::Sym;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::ops::Deref;
use streaming_iterator::StreamingIterator;

pub mod heuristics;
pub mod search;
pub mod state;

/// Representation for a value that might be either already known (the hole is full)
/// or unknown. When unknown the hole is empty and remains to be filled.
/// This corresponds to the `Param` variant that specifies the ID of the parameter
/// from which the value should be taken.
#[derive(Copy, Clone, Ord, PartialOrd, PartialEq, Eq)]
pub enum Holed<A> {
    /// Value is specified
    Full(A),
    /// Value is not present yet and should be the one of the n^th parameter
    Param(usize),
}

pub struct ParameterizedPred {
    pub positive: bool,
    pub sexpr: Vec<Holed<SymId>>,
}

impl ParameterizedPred {
    pub fn bind(&self, sd: &World, params: &[SymId], working: &mut Vec<SymId>) -> Option<Lit> {
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
    pub name: Sym,
    pub tpe: Sym,
}

pub struct ActionSchema {
    pub name: SymId,
    pub params: Vec<(TypeId, Option<String>)>,
    pub pre: Vec<ParameterizedPred>,
    pub eff: Vec<ParameterizedPred>,
}

pub struct LiftedProblem {
    pub world: World,
    pub initial_state: State,
    pub goals: Vec<Lit>,
    pub actions: Vec<ActionSchema>,
}

fn sv_to_lit(variable: &[SAtom], value: Atom, world: &World, _ctx: &Ctx) -> Result<Lit> {
    let sv: Result<Vec<SymId>, _> = variable.iter().map(|satom| SymId::try_from(*satom)).collect();
    let sv = sv?;
    let sv_id = world
        .sv_id(&sv)
        .context("No state variable identifed (maybe due to a typing error")?;
    match bool::try_from(value) {
        Ok(v) => Ok(Lit::new(sv_id, v)),
        Err(_) => bail!("state variable is not bound to a constant boolean"),
    }
}

fn holed_sv_to_pred(variable: &[SAtom], value: Atom, to_new_param: &HashMap<SVar, usize>) -> Result<ParameterizedPred> {
    let mut sv: Vec<Holed<SymId>> = Vec::new();
    for var in variable {
        let x = match var {
            SAtom::Var(svar) => Holed::Param(*to_new_param.get(svar).context("Invalid varible")?),
            SAtom::Cst(sym) => Holed::Full(sym.sym),
        };
        sv.push(x);
    }
    let value = bool::try_from(value).context("state variable not bound to a constant boolean")?;
    Ok(ParameterizedPred {
        positive: value,
        sexpr: sv,
    })
}

pub fn from_chronicles(chronicles: &crate::chronicles::Problem) -> Result<LiftedProblem> {
    let symbols = chronicles.context.model.symbols.deref().clone();

    let world = World::new(symbols, &chronicles.context.state_functions)?;
    let mut state = world.make_new_state();
    let mut goals = Vec::new();
    let ctx = &chronicles.context;
    for instance in &chronicles.chronicles {
        let ch = &instance.chronicle;
        ensure!(ch.presence == BAtom::from(true), "A chronicle instance is optional",);
        for eff in &ch.effects {
            ensure!(
                eff.effective_start() == eff.transition_start(),
                "Non instantaneous effect",
            );
            ensure!(
                eff.effective_start() == ctx.origin(),
                "Effect not at start in initial chronicle",
            );
            let lit = sv_to_lit(eff.variable(), eff.value(), &world, ctx)?;
            state.set(lit);
        }
        for cond in &ch.conditions {
            ensure!(cond.start() == cond.end(), "Non instantaneous goal condition");
            ensure!(
                cond.start() == ctx.horizon(),
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
            Some(id) => SymId::try_from(*id).context("Expected action symbol")?,
            _ => bail!("Unamed temlate"),
        };
        let global_start = ctx.origin();
        let global_end = ctx.horizon();
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

        // process all parameters (we have already removed the same
        while let Some(x) = iter.next() {
            let var = SVar::try_from(*x).context("Expected variable")?;
            let _tpe = var.tpe;

            let _ = template
                .parameter_index(var)
                .context("Not a parameter of the template.")?;
            let tpe = x.tpe();
            let label = chronicles.context.model.discrete.label(var).map(|s| s.to_string());

            correspondance.insert(var, parameters.len());
            parameters.push((tpe, label));
        }

        let mut schema = ActionSchema {
            name,
            params: parameters,
            pre: vec![],
            eff: vec![],
        };

        for cond in &template.chronicle.conditions {
            ensure!(
                cond.start() == template.chronicle.start,
                "Non final condition can not be interpreted as goal",
            );
            ensure!(
                cond.end == template.chronicle.start || cond.end == template.chronicle.end,
                "Unsupported temporal span for condition"
            );
            let pred = holed_sv_to_pred(cond.variable(), cond.value(), &correspondance)?;
            schema.pre.push(pred);
        }
        for eff in &template.chronicle.effects {
            ensure!(
                eff.transition_start() == template.chronicle.start,
                "Effect does not start condition with action's start",
            );
            ensure!(
                eff.effective_start() == template.chronicle.end,
                "Effect is not active at action's end",
            );
            let pred = holed_sv_to_pred(eff.variable(), eff.value(), &correspondance)?;
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

pub fn grounded_problem(lifted: &LiftedProblem) -> Result<GroundProblem> {
    let mut operators = Operators::new();

    for template in &lifted.actions {
        let ops = ground_action_schema(template, &lifted.world);
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

fn ground_action_schema(schema: &ActionSchema, desc: &World) -> Vec<Operator> {
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

    res
}
