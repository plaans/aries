use crate::chronicles::constraints::ConstraintType;
use crate::chronicles::{concrete, TimeConstant};
use aries_model::lang::{Atom, BAtom, ConversionError, IAtom, SAtom};
use std::cmp::Ordering;
use std::convert::{TryFrom, TryInto};

// pub trait Template {
//     type Concrete;
//
//     fn bind(&self, params: &[Atom]) -> Result<Self::Concrete, ConversionError>;
// }
//
// impl<T: Template> Template for Vec<T> {
//     type Concrete = Vec<T::Concrete>;
//
//     fn bind(&self, params: &[Atom]) -> Result<Self::Concrete, ConversionError> {
//         self.iter().map(|e| e.bind(params)).collect()
//     }
// }
//
// /// Representation for a value that might be either already known (the hole is full)
// /// or unknown. When unknown the hole is empty and remains to be filled.
// /// This corresponds to the `Param` variant that specifies the ID of the parameter
// /// from which the value should be taken.
// #[derive(Copy, Clone, Ord, PartialOrd, PartialEq, Eq)]
// pub enum Holed<A> {
//     /// Value is specified
//     Full(A),
//     /// Value is not present yet and should be the one of the n^th parameter
//     Param(usize),
// }
//
// impl<T: TryFrom<Atom, Error = ConversionError> + Copy> Template for Holed<T> {
//     type Concrete = Atom;
//
//     fn bind(&self, params: &[Atom]) -> Result<Self::Concrete, ConversionError> {
//         match self {
//             Holed::Full(a) => Atom::try_from(*a),
//             Holed::Param(i) => Ok(params[i]),
//         }
//     }
// }
//
// #[derive(Copy, Clone, Debug)]
// pub struct Time {
//     pub time_var: Holed<IAtom>,
//     pub delay: TimeConstant,
// }
// impl Time {
//     pub fn new(reference: Holed<IAtom>) -> Self {
//         Time {
//             time_var: reference,
//             delay: 0,
//         }
//     }
//     pub fn shifted(reference: Holed<IAtom>, delay: TimeConstant) -> Self {
//         Time {
//             time_var: reference,
//             delay,
//         }
//     }
// }
//
// impl PartialOrd for Time {
//     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
//         if self.time_var == other.time_var {
//             Some(self.delay.cmp(&other.delay))
//         } else {
//             None
//         }
//     }
// }
// impl PartialEq for Time {
//     fn eq(&self, other: &Self) -> bool {
//         self.time_var == other.time_var && self.delay == other.delay
//     }
// }
// impl Template for Time {
//     type Concrete = IAtom;
//
//     fn bind(&self, params: &[Atom]) -> Result<Self::Concrete, ConversionError> {
//         match self.time_var {
//             Holed::Full(v) => Ok(v + self.delay),
//             Holed::Param(i) => params[i].try_into().map(|v| v + self.delay),
//         }
//     }
// }
//
// pub type SV = Vec<Holed<SAtom>>;
//
// #[derive(Clone, Debug)]
// pub struct Effect {
//     pub transition_start: Time,
//     pub persistence_start: Time,
//     pub state_var: SV,
//     pub value: Holed<Atom>,
// }
//
// impl Effect {
//     pub fn effective_start(&self) -> Time {
//         self.persistence_start
//     }
//     pub fn transition_start(&self) -> Time {
//         self.transition_start
//     }
//     pub fn variable(&self) -> &[Holed<SAtom>] {
//         self.state_var.as_slice()
//     }
//     pub fn value(&self) -> Holed<Atom> {
//         self.value
//     }
// }
//
// impl Template for Effect {
//     type Concrete = concrete::Effect;
//
//     fn bind(&self, params: &[Atom]) -> Result<Self::Concrete, ConversionError> {
//         Ok(concrete::Effect {
//             transition_start: self.transition_start.bind(params)?,
//             persistence_start: self.persistence_start.bind(params)?,
//             state_var: self.state_var.bind(params)?,
//             value: self.value.bind(params)?,
//         })
//     }
// }
//
// #[derive(Clone)]
// pub struct Condition {
//     pub start: Time,
//     pub end: Time,
//     pub state_var: SV,
//     pub value: Holed<Atom>,
// }
//
// impl Condition {
//     pub fn start(&self) -> Time {
//         self.start
//     }
//     pub fn end(&self) -> Time {
//         self.end
//     }
//     pub fn variable(&self) -> &[Holed<SAtom>] {
//         self.state_var.as_slice()
//     }
//     pub fn value(&self) -> Holed<Atom> {
//         self.value
//     }
// }
//
// impl Template for Condition {
//     type Concrete = concrete::Condition;
//
//     fn bind(&self, params: &[Atom]) -> Result<Self::Concrete, ConversionError> {
//         Ok(concrete::Condition {
//             start: self.start.bind(params)?,
//             end: self.end.bind(params)?,
//             state_var: self.state_var.bind(params)?,
//             value: self.value.bind(params)?,
//         })
//     }
// }
//
// /// Generic representation of a constraint on a set of variables
// #[derive(Debug, Clone)]
// pub struct Constraint {
//     pub variables: Vec<Holed<Atom>>,
//     pub tpe: ConstraintType,
// }
//
// impl Template for Constraint {
//     type Concrete = concrete::Constraint;
//
//     fn bind(&self, params: &[Atom]) -> Result<Self::Concrete, ConversionError> {
//         Ok(concrete::Constraint {
//             variables: self.variables.bind(params)?,
//             tpe: self.tpe,
//         })
//     }
// }
//
// #[derive(Clone)]
// pub struct Chronicle {
//     pub presence: Holed<BAtom>,
//     pub start: Time,
//     pub end: Time,
//     pub name: SV,
//     pub conditions: Vec<Condition>,
//     pub effects: Vec<Effect>,
//     pub constraints: Vec<Constraint>,
// }
//
// impl Template for Chronicle {
//     type Concrete = concrete::Chronicle;
//
//     fn bind(&self, params: &[Atom]) -> Result<Self::Concrete, ConversionError> {
//         Ok(concrete::Chronicle {
//             presence: self.presence.bind(params)?,
//             start: self.start.bind(params)?,
//             end: self.end.bind(params)?,
//             name: self.name.bind(params)?,
//             conditions: self.conditions.bind(params)?,
//             effects: self.effects.bind(params)?,
//             constraints: self.constraints.bind(params)?,
//         })
//     }
// }
