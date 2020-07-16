use std::num::NonZeroU32;

use crate::clause::ClauseId;
use crate::Decision;
use aries_collections::index_map::*;
use aries_collections::{MinVal, Next};
use std::convert::{TryFrom, TryInto};
use std::fmt::{Debug, Display, Error, Formatter};
use std::ops::Not;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct DecisionLevel(u32);

// TODO: move to associated constant
pub const GROUND_LEVEL: DecisionLevel = DecisionLevel(0);

impl DecisionLevel {
    pub const MAX: DecisionLevel = DecisionLevel(u32::MAX);

    pub fn offset(&self) -> u32 {
        self.0
    }
    pub fn prev(&self) -> Self {
        debug_assert!(self > &GROUND_LEVEL);
        DecisionLevel(self.offset() - 1)
    }
    pub fn next(&self) -> Self {
        DecisionLevel(self.offset() + 1)
    }
    pub fn ground() -> Self {
        GROUND_LEVEL
    }
}

impl Display for DecisionLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "DecLvl({})", self.0)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct BVar {
    pub id: NonZeroU32,
}
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Lit {
    pub id: NonZeroU32,
}

impl From<i32> for Lit {
    fn from(i: i32) -> Self {
        let lit = Lit::new(BVar::from_bits(i.abs() as u32), i > 0);
        debug_assert_eq!(format!("{}", i), format!("{}", lit));
        lit
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[repr(u8)] // from minisat-rust, not sure if this buys us anything
pub enum BVal {
    Undef = 2,
    True = 1,
    False = 0,
}

impl BVal {
    pub fn from_bool(v: bool) -> Self {
        if v {
            BVal::True
        } else {
            BVal::False
        }
    }
    pub fn to_bool(self) -> bool {
        assert_ne!(self, BVal::Undef);
        match self {
            BVal::Undef => unreachable!(),
            BVal::True => true,
            BVal::False => false,
        }
    }

    pub fn to_char(self) -> char {
        self.into()
    }
}

impl std::ops::Not for BVal {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            BVal::Undef => BVal::Undef,
            BVal::True => BVal::False,
            BVal::False => BVal::True,
        }
    }
}

impl Display for BVal {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}", self.to_char())
    }
}

impl Debug for BVal {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}", self.to_char())
    }
}

impl Into<char> for BVal {
    fn into(self) -> char {
        match self {
            BVal::Undef => '?',
            BVal::True => '⊤',
            BVal::False => '⊥',
        }
    }
}
impl TryInto<bool> for BVal {
    type Error = ();

    fn try_into(self) -> Result<bool, Self::Error> {
        match self {
            BVal::Undef => Result::Err(()),
            BVal::True => Result::Ok(true),
            BVal::False => Result::Ok(false),
        }
    }
}
impl ToIndex for BVar {
    fn to_index(&self) -> usize {
        self.to_bits() as usize
    }
    fn first_index() -> usize {
        1
    }
}
impl ToIndex for Lit {
    fn to_index(&self) -> usize {
        self.to_bits() as usize
    }
    fn first_index() -> usize {
        1
    }
}

impl BVar {
    pub fn from_bits(id: u32) -> BVar {
        debug_assert!(id > 0, "Zero is not allowed. First valid ID is 1.");
        debug_assert!(id <= (std::u32::MAX >> 1), "The ID should fit on 31 bits.");
        BVar {
            id: NonZeroU32::new(id).unwrap(),
        }
    }
    pub fn to_bits(self) -> u32 {
        self.id.get()
    }

    pub fn next(self) -> Self {
        BVar::from_bits(self.to_bits() + 1)
    }

    pub fn false_lit(self) -> Lit {
        Lit::new(self, false)
    }
    pub fn true_lit(self) -> Lit {
        Lit::new(self, true)
    }
    pub fn lit(self, value: bool) -> Lit {
        Lit::new(self, value)
    }
}

impl Into<usize> for BVar {
    fn into(self) -> usize {
        self.to_bits() as usize
    }
}
impl TryFrom<usize> for BVar {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match NonZeroU32::new(value as u32) {
            Some(i) => Result::Ok(BVar { id: i }),
            None => Result::Err(()),
        }
    }
}

impl Next for BVar {
    fn next_n(self, n: usize) -> Self {
        BVar::from_bits(self.to_bits() + n as u32)
    }
}

impl MinVal for BVar {
    fn min_value() -> Self {
        BVar::from_bits(1)
    }
}

impl Display for BVar {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "{}", self.to_bits())
    }
}

impl Debug for BVar {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "{}", self)
    }
}

impl Lit {
    /// A structurally valid literal that should not overlap with any valid one.
    /// This is typically to be used as a place holder.
    /// In most cases, your would be interested in an Option<Lit> that has no representation overhead
    pub fn dummy() -> Self {
        Lit::from_bits(u32::max_value())
    }
    fn new(var: BVar, val: bool) -> Lit {
        let bits = (var.to_bits() << 1) | (val as u32);
        Lit::from_bits(bits)
    }
    fn from_bits(bits: u32) -> Self {
        Lit {
            id: NonZeroU32::new(bits).unwrap(),
        }
    }
    fn to_bits(self) -> u32 {
        self.id.get()
    }
    pub fn from_signed_int(i: i32) -> Lit {
        assert_ne!(i, 0);
        let v = BVar::from_bits(i.abs() as u32);
        if i > 0 {
            v.true_lit()
        } else {
            v.false_lit()
        }
    }
    pub fn variable(&self) -> BVar {
        BVar::from_bits(self.to_bits() >> 1)
    }
    pub fn negate(self) -> Lit {
        Lit::from_bits(self.to_bits() ^ 1)
    }
    pub fn is_positive(&self) -> bool {
        !self.is_negative()
    }
    pub fn is_negative(&self) -> bool {
        self.to_bits() & 1 == 0
    }
}
impl Not for Lit {
    type Output = Lit;

    fn not(self) -> Self::Output {
        self.negate()
    }
}
impl Display for Lit {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        if self.is_negative() {
            write!(f, "-")?;
        }
        write!(f, "{}", self.variable())
    }
}
impl Debug for Lit {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}", self)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VarState {
    pub value: BVal,
    decision_level: DecisionLevel,
    reason: Option<ClauseId>,
}
impl VarState {
    pub const INIT: VarState = VarState {
        value: BVal::Undef,
        decision_level: GROUND_LEVEL,
        reason: None,
    };
}

pub struct Assignments {
    pub(crate) ass: IndexMap<BVar, VarState>,
    trail: Vec<Lit>,
    levels: Vec<(Decision, usize)>,
}

impl Assignments {
    pub fn new(num_vars: u32) -> Self {
        Assignments {
            ass: IndexMap::new((num_vars + 1) as usize, VarState::INIT),
            trail: Vec::new(),
            levels: Vec::new(),
        }
    }
    pub fn set_lit(&mut self, l: Lit, reason: Option<ClauseId>) {
        if l.is_negative() {
            self.set(l.variable(), false, reason)
        } else {
            self.set(l.variable(), true, reason)
        }
    }
    pub fn set(&mut self, var: BVar, value: bool, reason: Option<ClauseId>) {
        debug_assert_eq!(self.ass[var].value, BVal::Undef);
        self.ass[var].value = BVal::from_bool(value);
        self.ass[var].decision_level = self.decision_level();
        self.ass[var].reason = reason;

        self.trail.push(var.lit(value))
    }
    pub fn is_set(&self, var: BVar) -> bool {
        self.ass[var].value != BVal::Undef
    }
    pub fn get(&self, var: BVar) -> BVal {
        self.ass[var].value
    }
    pub fn value_of(&self, lit: Lit) -> BVal {
        let var_value = self.get(lit.variable());
        if lit.is_positive() {
            var_value
        } else {
            !var_value
        }
    }
    pub fn add_backtrack_point(&mut self, dec: Decision) {
        self.levels.push((dec, self.trail.len()));
    }
    pub fn backtrack<F: FnMut(BVar) -> ()>(&mut self, on_restore: &mut F) -> Option<Decision> {
        match self.levels.pop() {
            Some((backtrack_decision, backtrack_point)) => {
                for i in backtrack_point..self.trail.len() {
                    let lit = self.trail[i];
                    self.ass[lit.variable()] = VarState::INIT;
                    on_restore(lit.variable());
                }
                self.trail.truncate(backtrack_point);
                Some(backtrack_decision)
            }
            None => None,
        }
    }
    pub fn backtrack_to<F: FnMut(BVar) -> ()>(&mut self, lvl: DecisionLevel, on_restore: &mut F) -> Option<Decision> {
        debug_assert!(self.decision_level() > lvl);
        loop {
            match self.backtrack(on_restore) {
                Some(dec) => {
                    if self.decision_level() == lvl {
                        return Some(dec);
                    }
                }
                None => return None,
            }
        }
    }
    pub fn last_assignment(&self, past_time: usize) -> Lit {
        self.trail[self.trail.len() - 1 - past_time]
    }
    pub fn undo_one(&mut self) {
        unimplemented!();
    }
    pub fn root_level(&self) -> DecisionLevel {
        // TODO
        GROUND_LEVEL
    }
    pub fn decision_level(&self) -> DecisionLevel {
        DecisionLevel(self.levels.len() as u32)
    }
    pub fn level(&self, var: BVar) -> DecisionLevel {
        self.ass[var].decision_level
    }
    pub fn reason(&self, var: BVar) -> Option<ClauseId> {
        self.ass[var].reason
    }
    pub fn num_assigned(&self) -> usize {
        self.trail.len()
    }
}
