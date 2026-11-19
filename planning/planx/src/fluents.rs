use std::fmt::Display;
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

    pub fn get_mut(&mut self, id: FluentId) -> &mut Fluent {
        self.fluents.get_mut(id).unwrap()
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
            annotations: Default::default(),
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

    /// Removes fluents (assumed to be known) for which `func` returns true.
    /// As a result, the internal storage of fluents could end up having "holes"
    /// (which won't be filled / reused by the current implementation of `add_fluent`).
    pub(crate) fn remove(&mut self, func: impl Fn(FluentId, &Fluent) -> bool) {
        let mut acc = vec![];
        for (fid, f) in self.fluents.iter() {
            if func(fid, f) {
                acc.push(fid);
            }
        }
        for fid in acc {
            self.fluents.remove(fid).expect("unknown fluent id");
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Fluent> + '_ {
        self.fluents.iter().map(|(_k, v)| v)
    }
    pub fn iter_with_id(&self) -> impl Iterator<Item = (FluentId, &Fluent)> + '_ {
        self.fluents.iter()
    }
}

#[derive(Clone, Debug)]
pub struct Fluent {
    pub name: Sym,
    pub parameters: Vec<Param>,
    pub return_type: Type,
    // source of the declaration of the fluent
    pub source: Option<Span>,
    /// Additional information on the fluent, typically set by preprocessing steps.
    pub annotations: FluentAnnotations,
}

impl Display for Fluent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{:?} -> {}", self.name, self.parameters, self.return_type)?;
        if let Some(props) = &self.annotations.properties {
            write!(f, "    {props}")?;
        }
        Ok(())
    }
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
            self.parameters[i].tpe().accepts(*arg, env)?;
        }
        Ok(self.return_type.clone())
    }
}

impl Spanned for &Fluent {
    fn span(&self) -> Option<&Span> {
        self.source.as_ref()
    }
}
