use std::num::NonZeroU32;

use crate::clause::ClauseId;
use aries_collections::ref_store::RefStore;
use std::convert::TryInto;
use std::fmt::{Debug, Display, Error, Formatter};
use std::ops::Not;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct DecisionLevel(u32);

impl DecisionLevel {
    pub const INFINITY: DecisionLevel = DecisionLevel(i32::MAX as u32);
    pub const GROUND: DecisionLevel = DecisionLevel(0);

    pub fn offset(&self) -> u32 {
        self.0
    }
    pub fn prev(&self) -> Self {
        debug_assert!(self > &DecisionLevel::GROUND);
        DecisionLevel(self.offset() - 1)
    }
    pub fn next(&self) -> Self {
        DecisionLevel(self.offset() + 1)
    }
    pub fn ground() -> Self {
        DecisionLevel::GROUND
    }
}

impl std::ops::Sub for DecisionLevel {
    type Output = i32;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0 as i32 - rhs.0 as i32
    }
}

impl Display for DecisionLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "DecLvl({})", self.0)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct BVar {
    id: NonZeroU32,
}
impl BVar {
    pub fn new(id: NonZeroU32) -> BVar {
        BVar { id }
    }
}
impl From<usize> for BVar {
    fn from(u: usize) -> Self {
        unsafe {
            BVar {
                id: NonZeroU32::new_unchecked(u as u32 + 1),
            }
        }
    }
}
impl From<BVar> for usize {
    fn from(v: BVar) -> Self {
        (v.id.get() - 1) as usize
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Lit {
    pub id: NonZeroU32,
}

impl From<i32> for Lit {
    fn from(i: i32) -> Self {
        let lit = Lit::new(BVar::new(NonZeroU32::new(i.abs() as u32).unwrap()), i > 0);
        debug_assert_eq!(format!("{}", i), format!("{}", lit));
        lit
    }
}
impl From<Lit> for usize {
    fn from(l: Lit) -> Self {
        l.id.get() as usize - 2
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

impl BVar {
    pub fn next(self) -> Self {
        BVar::from(usize::from(self) + 1)
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
        write!(f, "{}", usize::from(*self) + 1)
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
    pub(crate) fn dummy() -> Self {
        Lit::from_bits(u32::max_value())
    }
    fn new(var: BVar, val: bool) -> Lit {
        let bits = ((usize::from(var) as u32 + 1) << 1) | (val as u32);
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
    pub fn from_signed_int(i: i32) -> Option<Lit> {
        NonZeroU32::new(i.abs() as u32).map(|nz| {
            if i > 0 {
                BVar::new(nz).true_lit()
            } else {
                BVar::new(nz).false_lit()
            }
        })
    }
    pub fn variable(&self) -> BVar {
        BVar::from((self.to_bits() >> 1) as usize - 1)
    }
    pub fn value(&self) -> bool {
        self.is_positive()
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
    /// Current value of the variable or `Undef`
    pub value: BVal,
    /// Decision level at which the variable was given its value. Meaningless is variable is undef.
    decision_level: DecisionLevel,
    /// Clause whose unit propagation caused the variable to take its current value.
    /// None if the variable was not set through unit propagation (i.e. an arbitrary decision).
    /// Meaningless if the variable is undef.    
    reason: Option<ClauseId>,
    /// preferred value of the variable
    pub polarity: bool,
}
impl VarState {
    pub const INIT: VarState = VarState {
        value: BVal::Undef,
        decision_level: DecisionLevel::GROUND,
        reason: None,
        polarity: false,
    };

    pub fn clear_decision(&mut self) {
        // reset all fields except for polarity.
        self.value = VarState::INIT.value;
        self.decision_level = VarState::INIT.decision_level;
        self.reason = VarState::INIT.reason;
    }
}

pub(crate) struct Assignments {
    pub ass: RefStore<BVar, VarState>,
    pub trail: Vec<Lit>,
    levels: Vec<(Lit, usize)>,
}

impl Assignments {
    pub fn new(num_vars: u32) -> Self {
        Assignments {
            ass: RefStore::initialized(num_vars as usize, VarState::INIT), // IndexMap::new((num_vars + 1) as usize, VarState::INIT),
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

    /// Creates a new backtrack point associatied with teh given decision.
    pub fn add_backtrack_point(&mut self, decision: Lit) {
        self.levels.push((decision, self.trail.len()));
    }

    /// Backtrack to the last backtrack point and returns the corresponding decision.
    /// Returns None if nothing was left to undo
    pub fn backtrack<F: FnMut(BVar)>(&mut self, on_restore: &mut F) -> Option<Lit> {
        match self.levels.pop() {
            Some((backtrack_decision, backtrack_point)) => {
                for i in backtrack_point..self.trail.len() {
                    let lit = self.trail[i];
                    self.ass[lit.variable()].clear_decision();
                    on_restore(lit.variable());
                }
                self.trail.truncate(backtrack_point);
                Some(backtrack_decision)
            }
            None => None,
        }
    }

    /// Backtracks until decision level `lvl` and returns the decision literal associated with this level
    /// (which is the last undone decision).
    /// Returns None if nothing was undone (i.e; the current decision level was already `>= lvl`)
    /// Outcome the decision level of the solver is lesser than or equel to the one requested.
    pub fn backtrack_to<F: FnMut(BVar)>(&mut self, lvl: DecisionLevel, on_restore: &mut F) -> Option<Lit> {
        let mut last_decision = None;
        while self.decision_level() > lvl {
            last_decision = self.backtrack(on_restore);
        }
        last_decision
    }
    pub fn last_assignment(&self, past_time: usize) -> Lit {
        self.trail[self.trail.len() - 1 - past_time]
    }
    pub fn root_level(&self) -> DecisionLevel {
        // TODO
        DecisionLevel::GROUND
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
