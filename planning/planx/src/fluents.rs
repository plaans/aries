use derive_more::derive::Display;
use thiserror::Error;

use crate::{env::Environment, errors::ToEnvMessage, utils::disp_iter, *};

#[derive(Error, Debug)]
pub enum FluentError {
    #[error("Duplicate fluent")]
    DuplicateFluent(Sym, Sym),
    #[error("Unknown fluent")]
    UnkonwnFluent(Sym),
}

impl ToEnvMessage for FluentError {
    fn to_message(self, _env: &Environment) -> Message {
        match self {
            FluentError::DuplicateFluent(declared, previous) => declared
                .invalid("duplicated fluent declaration")
                .info(&previous, "previous declaration"),
            FluentError::UnkonwnFluent(sym) => sym.invalid("Unknown object"),
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Hash)]
pub struct FluentId(pub(crate) u32);

impl<'a> Env<'a, FluentId> {
    pub fn get(&self) -> &'a Fluent {
        self.env.fluents.get(self.elem)
    }

    pub fn name(&self) -> &'a Sym {
        self.get().name()
    }

    pub fn tpe(&self) -> &Type {
        &self.get().return_type
    }
}

impl idmap::intid::IntegerId for FluentId {
    idmap::intid::impl_newtype_id_body!(for FluentId(u32));
}

#[derive(Clone, Debug, Default)]
pub struct Fluents {
    fluents: idmap::DirectIdMap<FluentId, Fluent>,
    next_fluent_id: u32,
}

impl Display for Fluents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fluents:\n  ")?;
        disp_iter(f, self.iter(), "\n  ")
    }
}

impl Fluents {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: FluentId) -> &Fluent {
        self.fluents.get(id).unwrap()
    }

    pub fn get_by_name(&self, name: impl AsRef<str>) -> Option<FluentId> {
        self.fluents
            .iter()
            .find(|&(_id, f)| name.as_ref() == &f.name)
            .map(|(id, _)| id)
    }

    pub fn add_fluent(
        &mut self,
        name: impl Into<Sym>,
        parameters: Vec<Param>,
        return_type: Type,
        source: Option<Span>,
    ) -> Result<FluentId, FluentError> {
        let fluent = Fluent {
            name: name.into(),
            parameters,
            return_type,
            source,
        };
        if let Some(other) = self.get_by_name(fluent.name()) {
            let other_sym = self.fluents.get(other).unwrap().name().clone();
            Err(FluentError::DuplicateFluent(fluent.name.clone(), other_sym))
        } else {
            let id = FluentId(self.next_fluent_id);
            self.next_fluent_id += 1;
            let prev = self.fluents.insert(id, fluent);
            debug_assert!(prev.is_none());
            Ok(id)
        }
    }

    pub fn remove(&mut self, func: impl Fn(FluentId, &Fluent) -> bool) {
        let mut acc = vec![];
        for (fid, f) in self.fluents.iter() {
            if func(fid, f) {
                acc.push(fid);
            }
        }
        for fid in acc {
            self.fluents.remove(fid);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Fluent> + '_ {
        self.fluents.iter().map(|(_k, v)| v)
    }
    pub fn iter_with_id(&self) -> impl Iterator<Item = (FluentId, &Fluent)> + '_ {
        self.fluents.iter()
    }
}

#[derive(Clone, Debug, Display)]
#[display("{}{:?} -> {}", name, parameters, return_type)]
pub struct Fluent {
    pub name: Sym,
    pub parameters: Vec<Param>,
    pub return_type: Type,
    // source of the declaration of the fluent
    pub source: Option<Span>,
}

impl Fluent {
    pub fn name(&self) -> &Sym {
        &self.name
    }

    pub fn return_type(&self, args: &[ExprId], env: &Environment) -> Result<Type, Box<TypeError>> {
        if args.len() < self.parameters.len() {
            return Err(Box::new(TypeError::MissingParameter(
                self.parameters[args.len()].clone(),
            )));
        } else if args.len() > self.parameters.len() {
            return Err(Box::new(TypeError::UnexpectedArgument(args[self.parameters.len()])));
        }
        for (i, arg) in args.iter().enumerate() {
            self.parameters[i].tpe.accepts(*arg, env)?;
        }
        Ok(self.return_type.clone())
    }
}

impl Spanned for &Fluent {
    fn span(&self) -> Option<&Span> {
        self.source.as_ref()
    }
}
