use std::fmt::Display;

use itertools::Itertools;

use crate::{Env, ExprId, Param, RealValue, Sym, TimeInterval, Timestamp};

#[derive(Debug, Clone)]
pub struct Goal {
    universal_quantification: Vec<Param>,
    goal_expression: SimpleGoal,
}

impl<'a> Display for Env<'a, &Goal> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.env / &self.elem.goal_expression)?;
        if !self.elem.universal_quantification.is_empty() {
            write!(
                f,
                " | forall ({}) ",
                self.elem.universal_quantification.iter().join(", ")
            )?;
        }
        Ok(())
    }
}

/// Represents a goal statement (in PDDL goal or constraints).
///
///  - Regular PDDL goals (expressions that msut hold in the final state) are encoded as
///    `  HoldDuring([HORIZON,HORIZON], expression)`
///  - Other construct match the constraints of PDDL 3, possibly merged into more general ones
#[derive(Clone, Debug)]
pub enum SimpleGoal {
    /// A statement that must be true for the entire interval.
    /// Notably used for regular (interval: [horizon,horizon])
    HoldsDuring(TimeInterval, ExprId),
    /// A statement that must be true at least once during the given interval
    SometimeDuring(TimeInterval, ExprId),
    /// A statement that must be true at most once during the interval.
    /// It is interpreted as the statement may not be true, then become false and then true again.
    /// But it may remain true for more than one time unit.
    AtMostOnceDuring(TimeInterval, ExprId),
    /// Specifies that if the first expression is true, then the second should have been true some time earlier
    SometimeBefore { when: ExprId, then: ExprId },
    /// Specifies that if the first expression is true, then the second should have been true some time earlier
    SometimeAfter { when: ExprId, then: ExprId },
    /// Specifies that if the first expression becomes true at time `t`, then the second should be true with `t+delta` time units
    AlwaysWithin {
        delay: RealValue,
        when: ExprId,
        then: ExprId,
    },
}

impl SimpleGoal {
    /// Expression that must hold at a given timepoint
    pub fn at(tp: impl Into<Timestamp>, expr: ExprId) -> SimpleGoal {
        SimpleGoal::HoldsDuring(TimeInterval::at(tp), expr)
    }

    /// Universally qualify this goal expression over the given variables.
    ///
    /// If the set of variables is empty, the result is equivalent.
    /// If the set of variables is non empy, the variables should correspond to the ones already present in the goal expression.
    pub fn forall(self, vars: Vec<Param>) -> Goal {
        Goal {
            universal_quantification: vars,
            goal_expression: self,
        }
    }
}

impl<'a> Display for Env<'a, &SimpleGoal> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.elem {
            SimpleGoal::HoldsDuring(time_interval, expr_id) => {
                write!(f, "{} {}", time_interval, self.env / *expr_id)
            }
            SimpleGoal::SometimeDuring(time_interval, expr_id) => {
                write!(f, "sometime-during({} {})", time_interval, self.env / *expr_id)
            }
            SimpleGoal::AtMostOnceDuring(time_interval, expr_id) => {
                write!(f, "at-most-once-during({} {})", time_interval, self.env / *expr_id)
            }
            SimpleGoal::SometimeBefore { when, then } => {
                write!(f, "sometime-before({}, {})", self.env / *when, self.env / *then)
            }
            SimpleGoal::SometimeAfter { when, then } => {
                write!(f, "sometime-after({}, {})", self.env / *when, self.env / *then)
            }
            SimpleGoal::AlwaysWithin { delay, when, then } => {
                write!(
                    f,
                    "always-wihthin({}, {}, {})",
                    delay,
                    self.env / *when,
                    self.env / *then
                )
            }
        }
    }
}

pub type RefId = Sym;

/// A preference expressed with an identifier (not necessarily) and a goal statement.
#[derive(Clone, Debug)]
pub struct Preference {
    /// Universal quantification (forall (?x - object ?y - loc))
    /// May be left empty which should be interpreted as the absence of quantification.
    /// Note that a non-empty universal quantification will yield several preferences with the same identifier.
    pub universal_quantification: Vec<Param>,
    /// Name of the preference which may by used to check its satisfaction.
    /// Several preferences with the same identifier may be defined, in which the violation count might be
    /// greater than one.
    pub name: RefId,
    /// Goal expression associated to the preference.
    pub goal: Goal,
}

impl Preference {
    pub fn new(name: impl Into<RefId>, goal: Goal) -> Self {
        Preference {
            universal_quantification: Vec::new(),
            name: name.into(),
            goal,
        }
    }

    /// Builds a new preference that is universally qualified for the given vars
    pub fn forall(self, vars: Vec<Param>) -> Self {
        Self {
            universal_quantification: vars,
            name: self.name,
            goal: self.goal,
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct Preferences {
    prefs: Vec<Preference>,
}

impl Preferences {
    pub fn add(&mut self, pref: Preference) {
        self.prefs.push(pref);
    }
}

impl Display for Env<'_, Preference> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.elem.universal_quantification.is_empty() {
            write!(f, "forall ({}) ", self.elem.universal_quantification.iter().join(", "))?;
        }
        write!(f, "{}", self.env / &self.elem.goal)
    }
}
