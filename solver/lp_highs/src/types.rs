pub type AriesSignedVar = aries_solver::prelude::SignedVar;
pub type AriesVar = aries_solver::prelude::Var;
pub type AriesVarIdx = u32;
pub type AriesLit = aries_solver::prelude::Lit;
pub type AriesModelEvent = aries_solver::core::state::Event;

pub use aries_solver::core::LongCst;
pub use aries_solver::prelude::{INT_CST_MAX, INT_CST_MIN, IntCst};

pub type LpCol = highs::Col;
pub type LpRow = highs::Row;
pub type LpSolution = highs::Solution;
pub type LpIis = highs::Iis;
pub type LpModel = highs::Model;
pub type LpObjectiveSense = highs::Sense;

pub type FloatCst = f64;

pub fn long_cst_as_float(value: LongCst) -> FloatCst {
    // [`LongCst::MIN`] and [`LongCst::MAX`] stand for an unbounded column
    match value {
        LongCst::MIN => FloatCst::NEG_INFINITY,
        LongCst::MAX => FloatCst::INFINITY,
        value => value as FloatCst,
    }
}

pub fn int_cst_as_float(value: IntCst) -> FloatCst {
    value as FloatCst
}

/// Widening of a bound of the main model into the type column bounds are held in.
pub fn int_cst_as_long(value: IntCst) -> LongCst {
    value as LongCst
}

/// Analogous to a literal in the main aries solver, but on a column of the LP.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct LpLit {
    pub col: LpCol,
    pub tpe: LpLitType,
    pub val: LongCst,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum LpLitType {
    GEQ,
    LEQ,
}

impl LpLit {
    pub fn new(col: LpCol, tpe: LpLitType, val: LongCst) -> Self {
        Self { col, tpe, val }
    }
    pub fn leq(col: LpCol, val: LongCst) -> Self {
        Self {
            col,
            tpe: LpLitType::LEQ,
            val,
        }
    }
    pub fn geq(col: LpCol, val: LongCst) -> Self {
        Self {
            col,
            tpe: LpLitType::GEQ,
            val,
        }
    }
    pub fn entails(&self, other: Self) -> bool {
        if self.tpe == other.tpe {
            match self.tpe {
                LpLitType::GEQ => self.val >= other.val,
                LpLitType::LEQ => self.val <= other.val,
            }
        } else {
            false
        }
    }
    pub fn strictly_entails(&self, other: Self) -> bool {
        self.entails(other) && self.val != other.val
    }
}

/// Store all the necessary information for backtracking after modifying a bound
///
/// Only the old value and cause are necessary as they will overwrite the current ones
#[derive(Debug, Clone)]
pub(super) struct LpEvent {
    /// Column affected by the bound change
    pub col: LpCol,
    /// Bound modified
    pub bound: LpLitType,
    /// Value the bound had before this change, restored when backtracking
    pub old_val: LongCst,
    /// Cause that justified `old_val`, restored along with it
    pub old_cause: BoundCause,
}

/// Justification of a bound of an LP column: the reason why that bound was set.
/// In other words, it corresponds to a sufficient condition for the bound to hold.
///
/// - `Some { scope, trigger }`: the bound holds as long as **both** the scope and trigger literal are entailed in the main model.
///   `scope` is [`AriesLit::TRUE`] for the bounds that are not guarded by a scope, which is the common case.
/// - `None`: the bound holds unconditionally (initial bound of a column, or bound entailed at the root)
///   (it is functionally equivalent to `Some { scope: AriesLit::TRUE, trigger: AriesLit::TRUE }`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundCause {
    Some { scope: AriesLit, trigger: AriesLit },
    None,
}

impl BoundCause {
    /// Outputs the literals that must be entailed for a bound with this cause to hold.
    /// An unconditional bound outputs nothing, and neither does a tautological literal.
    pub(super) fn explain(&self, out: &mut impl FnMut(AriesLit)) {
        if let BoundCause::Some { scope, trigger } = self {
            if !scope.tautological() {
                out(*scope);
            }
            if !trigger.tautological() {
                out(*trigger);
            }
        }
    }
}

pub(super) enum LpBoundUpdate {
    Unchanged,
    Tightened,
    /// Inconsistent update (lower bound higher than upper bound). Cause is handed back to explain the conflict.
    Emptied(BoundCause),
}
