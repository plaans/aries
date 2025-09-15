use derive_more::derive::Display;
use thiserror::Error;

use crate::*;

#[derive(Clone, Display, Debug)]
#[display("{}", name)]
pub struct Object {
    name: Sym,
    tpe: UserType,
}

impl Object {
    pub fn new(name: impl Into<Sym>, tpe: UserType) -> Self {
        Self { name: name.into(), tpe }
    }

    pub fn name(&self) -> &Sym {
        &self.name
    }

    pub fn tpe(&self) -> &UserType {
        &self.tpe
    }
}

#[derive(Error, Debug)]
pub enum ObjectError {
    #[error("duplicate object : {0} and {1}")]
    DuplicateObjectDeclaration(Sym, Sym),
    #[error("unknown object {0}")]
    UnknownObject(Sym),
}

#[derive(Clone, Debug, Default)]
pub struct Objects {
    objects: hashbrown::HashMap<Sym, UserType>,
}

impl Display for Objects {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Objects:")?;
        for o in self.iter() {
            write!(f, "\n  {}: {}", o.name, o.tpe)?;
        }
        writeln!(f)
    }
}

impl Objects {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_object(&mut self, name: impl Into<Sym>, tpe: UserType) -> Result<(), ObjectError> {
        let name = name.into();
        if let Some((previous, previous_tpe)) = self.objects.get_key_value(&name) {
            if previous_tpe == &tpe {
                // objects are exactly the same, ignore as some PDDL domain contain such patterns
                Ok(())
            } else {
                Err(ObjectError::DuplicateObjectDeclaration(name, previous.clone()))
            }
        } else {
            self.objects.insert(name, tpe);
            Ok(())
        }
    }

    pub fn get(&self, name: impl Into<Sym>) -> Result<Object, ObjectError> {
        let name = name.into();
        match self.objects.get(&name) {
            Some(tpe) => Ok(Object::new(name, tpe.clone())),
            None => Err(ObjectError::UnknownObject(name)),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Object> + '_ {
        self.objects.iter().map(|(k, v)| Object::new(k.clone(), v.clone()))
    }
}
