use std::io::Write;
use std::{collections::BTreeMap, fmt::Display, path::Path};

use aries_solver::{core::state::Evaluable, prelude::*};
use itertools::Itertools;
use planx::{ActionRef, Message, errors::Ctx};
use planx::{RealValue, Res};
use timelines::{ConstraintID, IntTerm, Sym, Time, symbols::ObjectDecoder};

use crate::{
    encode::{required_values::RequiredValues, tags::Tag},
    plans::Operation,
};

/// Representation of the encoding that allows reconstructing a solution plan from a valid assignment.
#[derive(Default)]
pub struct Encoding {
    /// All actions instances that may appear in the plan.
    pub actions: Vec<ActionInstance>,
    /// Variable encoding the objective value (minimization)
    pub objectives: Vec<LinTerm>,
    /// for each relaxable constraint, stores a constraint tag so that we can later decide if it should be relaxed.
    pub constraints_tags: BTreeMap<ConstraintID, Tag>,
    /// Associates each preference name with a list of literals that are true iff the preference hold.
    /// Note that there may be more than one literal because a single name may refer to more than one preference
    /// (this is notably the case for universally quantified ones).
    pub preferences: BTreeMap<String, Vec<Lit>>,
    /// Tracks the values that may be required in problem.
    pub required_values: RequiredValues,
}

impl Encoding {
    pub fn new() -> Self {
        Encoding {
            actions: vec![],
            objectives: vec![],
            constraints_tags: Default::default(),
            preferences: Default::default(),
            required_values: RequiredValues::new(),
        }
    }

    pub fn add_action(&mut self, instance: ActionInstance) {
        self.actions.push(instance);
    }

    pub fn add_objective(&mut self, objective: impl Into<LinTerm>) {
        self.objectives.push(objective.into())
    }

    /// Extracts the plan corresponding to this solution.
    pub fn plan<'a>(&'a self, solution: &'a Solution) -> Plan<'a> {
        Plan {
            encoding: self,
            solution,
        }
    }
}

/// A plan that can be formatted in the PDDL format.
pub struct Plan<'a> {
    encoding: &'a Encoding,
    solution: &'a Solution,
}

impl<'a> Plan<'a> {
    /// Write the plan to a file if the option contains a file path.
    /// This is no-op if the option is empty
    pub fn write_to_file<File: AsRef<Path>>(&self, out_file: Option<File>) -> Res<()> {
        if let Some(path) = out_file {
            let path = path.as_ref();
            let mut file = std::fs::File::create(path)
                .map_err(Message::from)
                .title(format!("Cannot create output file {}", path.display()))?;
            writeln!(file, "{self}")
                .map_err(Message::from)
                .title(format!("Cannot write output file {}", path.display()))?;
        }
        Ok(())
    }

    /// Export a solution plan for the original model (before any transformation was applied to it).
    pub fn export(&self, model: &planx::Model) -> Res<planx::SolutionPlan> {
        let mut operations = Vec::new();
        for a in &self.encoding.actions {
            let Some(a) = a.evaluate(self.solution) else {
                continue;
            };
            let mut arguments = Vec::with_capacity(a.arguments.len());
            for arg in a.arguments {
                let arg = model.env.objects.get(arg.as_str())?;
                arguments.push(arg);
            }
            let op = planx::Operation {
                start: RealValue::from_integer(a.start as i64),
                duration: RealValue::from_integer(a.duration as i64),
                action_ref: a.action_ref,
                arguments,
            };
            operations.push(op);
        }
        operations.sort_by_key(|o| o.start);
        let mut plan = planx::SolutionPlan {
            operations,
            metrics: self
                .encoding
                .objectives
                .iter()
                .map(|obj| RealValue::from_integer(obj.evaluate(self.solution).unwrap() as i64))
                .collect_vec(),
        };

        // we have the plan for the model as it is now. We need to revert all transforamtion
        // to get a valid original plan.
        for transformation in model.transformations.iter().rev() {
            plan = transformation.revert(plan);
        }

        Ok(plan)
    }
}

impl<'a> Display for Plan<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "{}",
            self.encoding
                .actions
                .iter()
                .filter_map(|a| a.evaluate(self.solution))
                .sorted_by_key(|a| a.start)
                .format("\n")
        )
    }
}

/// An action instance in the encoding, containing all variables necessary
/// to determine whether it appears in the solution and its timings and parameters.
pub struct ActionInstance {
    pub action_ref: ActionRef,
    pub prez: Lit,
    pub start: Time,
    pub end: Time,
    pub arguments: Vec<ObjectVar>,
}

impl Evaluable for ActionInstance {
    type Value = Operation<Sym>;

    fn evaluate(&self, solution: &Solution) -> Option<Self::Value> {
        if !solution.entails(self.prez) {
            return None;
        }
        // the presence variable is true, so we can mindlessly evaluate all the sub-expression
        // that are guaranteed to be present.
        let start = solution.eval(self.start).unwrap();
        let duration = solution.eval(self.end).unwrap() - start;
        Some(Operation {
            start,
            duration,
            action_ref: self.action_ref.clone(),
            arguments: self
                .arguments
                .iter()
                .map(|arg_var| arg_var.evaluate(solution).unwrap())
                .collect(),
            span: None,
        })
    }
}

/// A variable whose domain is a subset of the objects in the problem.
#[derive(Clone)]
pub struct ObjectVar {
    var: IntTerm,
    decoder: ObjectDecoder,
}

impl ObjectVar {
    pub fn new(var: impl Into<IntTerm>, decoder: &ObjectDecoder) -> Self {
        Self {
            var: var.into(),
            decoder: decoder.clone(),
        }
    }
}

impl Evaluable for ObjectVar {
    type Value = Sym;

    fn evaluate(&self, solution: &Solution) -> Option<Self::Value> {
        self.var
            .evaluate(solution)
            .map(|val| self.decoder.decode(val).unwrap().clone())
    }
}
