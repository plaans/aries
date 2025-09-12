use derive_more::derive::Display;
use thiserror::Error;
use utils::disp_slice;

use crate::{env::Environment, *};

#[derive(Error, Debug)]
pub enum FluentError {
    #[error("Duplicate fluent")]
    DuplicateFluent(Fluent, Fluent),
    #[error("Unknown fluent")]
    UnkonwnFluent(Sym),
}

#[derive(Clone, Debug, Default)]
pub struct Fluents {
    fluents: Vec<Fluent>,
}

impl Display for Fluents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fluents:\n  ")?;
        disp_slice(f, &self.fluents, "\n  ")
    }
}

impl Fluents {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, name: impl Into<Sym>) -> Result<&Fluent, FluentError> {
        let name = name.into();
        self.fluents
            .iter()
            .find(|f| f.name == name)
            .ok_or(FluentError::UnkonwnFluent(name))
    }

    pub fn add_fluent(
        &mut self,
        name: impl Into<Sym>,
        parameters: Vec<Param>,
        return_type: Type,
        origin: impl Into<Span>,
    ) -> Result<&Fluent, FluentError> {
        let fluent = Fluent {
            name: name.into(),
            parameters,
            return_type,
            origin: origin.into(),
        };
        if let Some(other) = self.fluents.iter().find(|f| f.name() == fluent.name()) {
            Err(FluentError::DuplicateFluent(fluent, other.clone()))
        } else {
            self.fluents.push(fluent);
            Ok(self.fluents.last().unwrap())
        }
    }
}

#[derive(Clone, Debug, Display)]
#[display("{}{:?} -> {}", name, parameters, return_type)]
pub struct Fluent {
    pub name: Sym,
    pub parameters: Vec<Param>,
    pub return_type: Type,
    pub origin: Span,
}

impl Fluent {
    pub fn name(&self) -> &Sym {
        &self.name
    }

    pub fn return_type(&self, args: &[ExprId], env: &Environment) -> Result<Type, TypeError> {
        if args.len() < self.parameters.len() {
            return Err(TypeError::MissingParameter(self.parameters[args.len()].clone()));
        } else if args.len() > self.parameters.len() {
            return Err(TypeError::UnexpectedArgument(args[self.parameters.len()]));
        }
        for (i, arg) in args.iter().enumerate() {
            self.parameters[i].tpe.accepts(*arg, env)?;
        }
        Ok(self.return_type.clone())
    }
}
