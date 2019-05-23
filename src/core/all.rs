use std::num::NonZeroU32;

use crate::collection::index_map::*;
use crate::Decision;
use std::fmt::{Display, Error, Formatter};
use std::ops::{Not, RangeInclusive};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct DecisionLevel(u32);

pub const GROUND_LEVEL: DecisionLevel = DecisionLevel(0);

impl DecisionLevel {
    pub fn offset(&self) -> u32 {
        self.0
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct BVar {
    pub id: NonZeroU32,
}
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct Lit {
    pub id: NonZeroU32,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
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
        assert!(self != BVal::Undef);
        match self {
            BVal::Undef => panic!(),
            BVal::True => true,
            BVal::False => false,
        }
    }
    pub fn neg(self) -> Self {
        match self {
            BVal::Undef => BVal::Undef,
            BVal::True => BVal::False,
            BVal::False => BVal::True,
        }
    }
}

impl ToIndex for BVar {
    fn to_index(&self) -> usize {
        self.to_bits() as usize
    }
}
impl ToIndex for Lit {
    fn to_index(&self) -> usize {
        self.to_bits() as usize
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

impl Display for BVar {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "{}", self.to_bits())
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
        assert!(i != 0);
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

#[derive(Clone, Copy, Debug)]
struct VarState {
    value: BVal,
    decision_level: DecisionLevel,
}
impl VarState {
    pub const INIT: VarState = VarState {
        value: BVal::Undef,
        decision_level: GROUND_LEVEL,
    };
}

pub struct Assignments {
    ass: IndexMap<BVar, VarState>,
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
    pub fn set_lit(&mut self, l: Lit) {
        if l.is_negative() {
            self.set(l.variable(), false)
        } else {
            self.set(l.variable(), true)
        }
    }
    pub fn set(&mut self, var: BVar, value: bool) {
        debug_assert!(self.ass[var].value == BVal::Undef);
        let lvl = self.decision_level();
        self.ass[var].value = BVal::from_bool(value);
        self.ass[var].decision_level = self.decision_level();

        self.trail.push(var.lit(value))
    }
    pub fn is_set(&self, var: BVar) -> bool {
        match self.ass[var].value {
            BVal::Undef => false,
            _ => true,
        }
    }
    pub fn get(&self, var: BVar) -> BVal {
        self.ass[var].value
    }
    pub fn add_backtrack_point(&mut self, dec: Decision) {
        self.levels.push((dec, self.trail.len()));
    }
    pub fn backtrack(&mut self) -> Option<Decision> {
        match self.levels.pop() {
            Some((backtrack_decision, backtrack_point)) => {
                for i in backtrack_point..self.trail.len() {
                    let lit = self.trail[i];
                    self.ass[lit.variable()] = VarState::INIT;
                }
                self.trail.truncate(backtrack_point);
                Some(backtrack_decision)
            }
            None => None,
        }
    }
    pub fn backtrack_to(&mut self, lvl: DecisionLevel) -> Option<Decision> {
        debug_assert!(
            self.decision_level() >= lvl,
            "{:?} > {:?}",
            self.decision_level(),
            lvl
        );
        loop {
            match self.backtrack() {
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
    pub fn num_assigned(&self) -> usize {
        self.trail.len()
    }
}

pub struct Clause {
    pub disjuncts: Vec<Lit>,
}
impl Clause {
    pub fn from(lits: &[i32]) -> Self {
        let mut x = vec![];
        for &l in lits {
            let lit = if l > 0 {
                BVar::from_bits(l as u32).true_lit()
            } else if l < 0 {
                BVar::from_bits((-l) as u32).false_lit()
            } else {
                panic!()
            };
            x.push(lit);
        }
        Clause { disjuncts: x }
    }
}
impl Display for Clause {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "[")?;
        for i in 0..self.disjuncts.len() {
            if i != 0 {
                write!(f, " ")?;
            }
            write!(f, "{}", self.disjuncts[i])?;
        }
        write!(f, "]")
    }
}

#[derive(PartialOrd, PartialEq, Debug, Clone, Copy)]
pub struct ClauseId(pub usize);

impl ClauseId {
    pub fn next(self) -> Self {
        ClauseId(self.0 + 1)
    }
}

impl ToIndex for ClauseId {
    fn to_index(&self) -> usize {
        self.0
    }
}
