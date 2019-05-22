use std::num::NonZeroU32;

use crate::collection::index_map::*;
use std::ops::{Not, RangeInclusive};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct DecisionLevel(usize);

pub const GROUND_LEVEL: DecisionLevel = DecisionLevel(0);

impl DecisionLevel {
    pub fn offset(&self) -> usize {
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
    Undef = 0,
    True = 1,
    False = 2,
}
impl BVal {
    pub fn from_bool(v: bool) -> Self {
        if v {
            BVal::True
        } else {
            BVal::False
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

impl Lit {
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
        BVar::from_bits(self.id.get() >> 1)
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

pub struct Assignments {
    ass: IndexMap<BVar, BVal>,
    trail: Vec<BVar>,
    levels: Vec<usize>,
}

impl Assignments {
    pub fn new(num_vars: u32) -> Self {
        Assignments {
            ass: IndexMap::new((num_vars + 1) as usize, BVal::Undef),
            trail: Vec::new(),
            levels: Vec::new(),
        }
    }
    pub fn set_lit(&mut self, l: Lit) {
        if l.is_negative() {
            self.set(l.variable(), BVal::False)
        } else {
            self.set(l.variable(), BVal::True)
        }
    }
    pub fn set(&mut self, var: BVar, value: BVal) {
        debug_assert!(self.ass[var] == BVal::Undef);
        self.ass.write(var, value);
        self.trail.push(var)
    }
    pub fn is_set(&self, var: BVar) -> bool {
        match self.ass[var] {
            BVal::Undef => false,
            _ => true,
        }
    }
    pub fn get(&self, var: BVar) -> BVal {
        self.ass[var]
    }
    pub fn add_backtrack_point(&mut self) {
        self.levels.push(self.trail.len())
    }
    pub fn backtrack(&mut self) {
        let backtrack_point = self.levels.pop().unwrap();
        for i in backtrack_point..self.trail.len() {
            let var = self.trail[i];
            self.ass.write(var, BVal::Undef);
        }
        self.trail.truncate(backtrack_point);
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
