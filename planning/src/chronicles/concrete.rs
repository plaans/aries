use crate::chronicles::constraints::Constraint;
use aries_model::lang::*;
use std::convert::TryFrom;

pub type SV = Vec<SAtom>;
type Time = IAtom;

pub trait Substitution {
    fn sub_var(&self, atom: Variable) -> Variable;

    fn sub(&self, atom: Atom) -> Atom {
        todo!()
    }

    fn isub(&self, i: IAtom) -> Result<IAtom, ConversionError> {
        todo!()
    }
    fn bsub(&self, b: BAtom) -> Result<BAtom, ConversionError> {
        todo!()
    }

    fn sbsub(&self, s: SAtom) -> Result<SAtom, ConversionError> {
        todo!()
    }
}

pub struct Sub<'a> {
    params: &'a [Variable],
    instances: &'a [Variable],
}
impl<'a> Sub<'a> {
    pub fn new(params: &'a [Variable], instances: &'a [Variable]) -> Self {
        Sub { params, instances }
    }
}

impl<'a> Substitution for Sub<'a> {
    fn sub_var(&self, atom: Variable) -> Variable {
        match self.params.iter().position(|&x| x == atom) {
            Some(i) => self.instances[i],
            None => atom,
        }
    }
}

pub trait Substitute
where
    Self: Sized,
{
    fn substitute(&self, substitution: &impl Substitution) -> Result<Self, ConversionError>;
}

impl<T> Substitute for Vec<T>
where
    T: TryFrom<Atom, Error = ConversionError> + Copy,
    Atom: From<T>,
{
    fn substitute(&self, substitution: &impl Substitution) -> Result<Self, ConversionError> {
        self.iter()
            .copied()
            .map(|t| {
                let atom = Atom::from(t);
                let substituted = substitution.sub(atom);
                T::try_from(substituted)
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct Effect {
    pub transition_start: Time,
    pub persistence_start: Time,
    pub state_var: SV,
    pub value: Atom,
}

impl Effect {
    pub fn effective_start(&self) -> Time {
        self.persistence_start
    }
    pub fn transition_start(&self) -> Time {
        self.transition_start
    }
    pub fn variable(&self) -> &[SAtom] {
        self.state_var.as_slice()
    }
    pub fn value(&self) -> Atom {
        self.value
    }
}
impl Substitute for Effect {
    fn substitute(&self, s: &impl Substitution) -> Result<Self, ConversionError> {
        Ok(Effect {
            transition_start: s.isub(self.transition_start)?,
            persistence_start: s.isub(self.persistence_start)?,
            state_var: self.state_var.substitute(s)?,
            value: s.sub(self.value),
        })
    }
}

#[derive(Clone)]
pub struct Condition {
    pub start: Time,
    pub end: Time,
    pub state_var: SV,
    pub value: Atom,
}

impl Condition {
    pub fn start(&self) -> Time {
        self.start
    }
    pub fn end(&self) -> Time {
        self.end
    }
    pub fn variable(&self) -> &[SAtom] {
        self.state_var.as_slice()
    }
    pub fn value(&self) -> Atom {
        self.value
    }
}

#[derive(Clone)]
pub struct Chronicle {
    pub presence: BAtom,
    pub start: Time,
    pub end: Time,
    pub name: SV,
    pub conditions: Vec<Condition>,
    pub effects: Vec<Effect>,
    pub constraints: Vec<Constraint>,
}
