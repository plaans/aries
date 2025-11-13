use std::collections::BTreeMap;

use aries_sched::IntCst;
use planx::{ActionRef, Model, Res, Sym, errors::Span};

#[derive(Debug, Clone)]
pub struct LiftedPlan {
    /// A set of operations: actions instances with arguments, start times and durations
    pub operations: Vec<Operation>,
    /// All variables apprearing in the lifted plan, together with an inferred type (most specific one from all their appearances.)
    pub variables: BTreeMap<Sym, planx::UserType>,
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub start: IntCst,
    pub duration: IntCst,
    pub action_ref: ActionRef,
    pub arguments: Vec<OperationArg>,
    #[allow(unused)]
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub enum OperationArg {
    Ground(planx::Object),
    Variable { name: Sym },
}

/// Parse a lifted plan into our own representation.
///
/// Note: we take as input a `planx::pddl::Plan` that is essentially a set of SExpr.
/// We cannot use the [`planx::Plan`] completly processed one that does not support having variables in it.
pub fn parse_lifted_plan(plan: &planx::pddl::Plan, model: &Model) -> Res<LiftedPlan> {
    let top_type = model.env.types.top_user_type();
    use planx::errors::*;
    let planx::pddl::Plan::ActionSequence(plan) = plan;

    // all actions in the plan
    let mut operations = Vec::with_capacity(plan.len());

    // all variables appearing in the plan
    let mut variables = BTreeMap::new();

    for (aid, a) in plan.iter().enumerate() {
        let action = model
            .actions
            .get_action(&a.name)
            .ok_or_else(|| a.name.invalid("Unknown action"))?;
        if a.arguments.len() != action.parameters.len() {
            return Err(a.invalid(format!(
                "Wrong number of parameters. Expected {}, provided: {}",
                action.parameters.len(),
                a.arguments.len()
            )));
        }
        let mut arguments = Vec::with_capacity(a.arguments.len());
        for (arg, param) in a.arguments.iter().zip(action.parameters.iter()) {
            let arg = if let Ok(obj) = model.env.objects.get(arg) {
                // an object: type check and return
                if !planx::Type::from(obj.tpe()).is_subtype_of(param.tpe()) {
                    return Err(arg.invalid(format!(
                        "Object has type `{} ` that is incompatible with the expected type for parameter`{}`",
                        obj.tpe(),
                        param
                    )));
                }
                OperationArg::Ground(obj)
            } else if arg.canonical_str().starts_with("?") {
                // variable: compute its as the most specific between its previous one and the parameter of the action
                let prev_type = variables.get(arg).unwrap_or(&top_type);
                let planx::Type::User(new_type) = param.tpe() else {
                    return Err(arg.invalid("expected a user type"));
                };
                let new_type = new_type
                    .to_single_type()
                    .ok_or_else(|| arg.invalid("Do not support union types"))?;
                let tpe = if new_type.is_subtype_of(prev_type) {
                    new_type
                } else {
                    prev_type.clone()
                };
                // reinsert the variable with the new type
                variables.insert(arg.clone(), tpe.clone());
                OperationArg::Variable { name: arg.clone() }
            } else {
                return Err(arg.invalid("cannot interpret argument: not a known object nor a variable"));
            };
            arguments.push(arg);
        }
        operations.push(Operation {
            start: aid as IntCst, // start time is the index of the action in the sequence
            duration: 0,          // action is instantaneous
            action_ref: a.name.clone(),
            arguments,
            span: a.span().cloned(),
        });
    }
    Ok(LiftedPlan { operations, variables })
}
