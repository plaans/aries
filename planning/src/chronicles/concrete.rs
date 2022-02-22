use core::fmt;
use std::fmt::Debug;

use crate::chronicles::constraints::Constraint;
use aries_core::{Lit, VarRef};
use aries_model::lang::*;

/// A state variable (`Sv`) is a sequence of symbolic expressions e.g. `(location-of robot1)` where:
///  - the first symbol is the name for state variable (e.g. `location-of`)
///  - the remaining elements are its parameters (e.g. `robot1`).
pub type Sv = Vec<SAtom>;

/// Representation for time (action's start, deadlines, ...)
/// It is encoded as a fixed point numeric expression `(ivar + icst) / denum` where
///  - `ivar` is an integer variable (possibly the `ZERO` variable)
///  - `icst` is an integer constant
///  - `denum` is an integer constant that fixes the resolution of time
///     (and should be the same among all time expression)
pub type Time = FAtom;

pub trait Substitution {
    fn sub_var(&self, var: VarRef) -> VarRef;

    fn sub_ivar(&self, atom: IVar) -> IVar {
        IVar::new(self.sub_var(atom.into()))
    }
    fn sub_bvar(&self, atom: BVar) -> BVar {
        BVar::new(self.sub_var(atom.into()))
    }
    fn sub_svar(&self, atom: SVar) -> SVar {
        SVar::new(self.sub_var(atom.var), atom.tpe)
    }

    fn sub_lit(&self, b: Lit) -> Lit {
        let (var, rel, val) = b.unpack();
        Lit::new(self.sub_var(var), rel, val)
    }

    fn sub(&self, atom: Atom) -> Atom {
        match atom {
            Atom::Bool(b) => self.sub_lit(b).into(),
            Atom::Int(i) => self.isub(i).into(),
            Atom::Sym(s) => self.ssub(s).into(),
            Atom::Fixed(f) => self.fsub(f).into(),
        }
    }

    fn isub(&self, i: IAtom) -> IAtom {
        IAtom::new(self.sub_ivar(i.var), i.shift)
    }

    fn fsub(&self, r: FAtom) -> FAtom {
        FAtom::new(self.isub(r.num), r.denom)
    }

    fn ssub(&self, s: SAtom) -> SAtom {
        match s {
            SAtom::Var(v) => SAtom::Var(self.sub_svar(v)),
            SAtom::Cst(s) => SAtom::Cst(s),
        }
    }
}

/// A substitution of params by instances.
/// The constructor validates the input to make sure that the parameters and instances are of the same kind.
pub struct Sub {
    parameters: Vec<VarRef>,
    instances: Vec<VarRef>,
}
impl Sub {
    pub fn empty() -> Self {
        Sub {
            parameters: Vec::new(),
            instances: Vec::new(),
        }
    }

    pub fn contains(&self, v: impl Into<VarRef>) -> bool {
        let v = v.into();
        self.parameters.contains(&v)
    }

    fn add_untyped(&mut self, param: VarRef, instance: VarRef) -> Result<(), InvalidSubstitution> {
        if self.parameters.contains(&param) {
            Err(InvalidSubstitution::DuplicatedEntry(param))
        } else {
            self.parameters.push(param);
            self.instances.push(instance);
            Ok(())
        }
    }

    pub fn add(&mut self, param: Variable, instance: Variable) -> Result<(), InvalidSubstitution> {
        if param.kind() != instance.kind() {
            Err(InvalidSubstitution::IncompatibleTypes(param, instance))
        } else {
            self.add_untyped(param.into(), instance.into())
        }
    }

    /// When possible, adds a substitution that would make `param == instance` when applied to param.
    /// Note that this requires the same structure so that only swapping a variable for another is necessary.
    pub fn add_expr_unification(&mut self, param: Atom, instance: Atom) -> Result<(), InvalidSubstitution> {
        match (param, instance) {
            (Atom::Sym(a), Atom::Sym(b)) => self.add_sym_expr_unification(a, b),
            (Atom::Int(a), Atom::Int(b)) => self.add_int_expr_unification(a, b),
            (Atom::Bool(a), Atom::Bool(b)) => self.add_bool_expr_unification(a, b),
            _ => Err(InvalidSubstitution::IncompatibleStructures(param, instance)),
        }
    }

    pub fn add_sym_expr_unification(&mut self, param: SAtom, instance: SAtom) -> Result<(), InvalidSubstitution> {
        match (param, instance) {
            (SAtom::Var(x), SAtom::Var(y)) => self.add(x.into(), y.into()),
            (SAtom::Cst(a), SAtom::Cst(b)) if a == b => Ok(()),
            _ => Err(InvalidSubstitution::IncompatibleStructures(
                param.into(),
                instance.into(),
            )),
        }
    }
    pub fn add_fixed_expr_unification(&mut self, param: FAtom, instance: FAtom) -> Result<(), InvalidSubstitution> {
        if param.denom == instance.denom {
            self.add_int_expr_unification(param.num, instance.num)
        } else {
            Err(InvalidSubstitution::IncompatibleStructures(
                param.into(),
                instance.into(),
            ))
        }
    }
    pub fn add_int_expr_unification(&mut self, param: IAtom, instance: IAtom) -> Result<(), InvalidSubstitution> {
        match (param, instance) {
            (IAtom { var: x, shift: dx }, IAtom { var: y, shift: dy }) if dx == dy => {
                if x == y {
                    Ok(())
                } else {
                    self.add(x.into(), y.into())
                }
            }
            _ => Err(InvalidSubstitution::IncompatibleStructures(
                param.into(),
                instance.into(),
            )),
        }
    }
    pub fn add_bool_expr_unification(&mut self, param: Lit, instance: Lit) -> Result<(), InvalidSubstitution> {
        if param == instance {
            Ok(())
        } else if param.relation() == instance.relation() && param.value() == instance.value() {
            self.add_untyped(param.variable(), instance.variable())
        } else {
            Err(InvalidSubstitution::IncompatibleStructures(
                param.into(),
                instance.into(),
            ))
        }
    }

    pub fn add_boolean(&mut self, param: BVar, instance: BVar) -> Result<(), InvalidSubstitution> {
        self.add_untyped(param.into(), instance.into())
    }

    pub fn new(params: &[Variable], instances: &[Variable]) -> Result<Self, InvalidSubstitution> {
        if params.len() != instances.len() {
            return Err(InvalidSubstitution::DifferentLength);
        }
        let mut sub = Sub::empty();
        for i in 0..params.len() {
            sub.add(params[i], instances[i])?;
        }
        Ok(sub)
    }
}
#[derive(Debug)]
pub enum InvalidSubstitution {
    IncompatibleTypes(Variable, Variable),
    DifferentLength,
    DuplicatedEntry(VarRef),
    IncompatibleStructures(Atom, Atom),
}
impl std::error::Error for InvalidSubstitution {}
impl std::fmt::Display for InvalidSubstitution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvalidSubstitution::IncompatibleTypes(x, y) => {
                write!(f, "Substitution with incompatible types {:?} -> {:?}", x, y)
            }
            InvalidSubstitution::DifferentLength => write!(f, "Different number of arguments in substitution"),
            InvalidSubstitution::DuplicatedEntry(v) => write!(f, "Entry {:?} appears twice in the substitution", v),
            InvalidSubstitution::IncompatibleStructures(a, b) => write!(
                f,
                "Entries {:?} and {:?} have different structures and cannot be unified",
                a, b
            ),
        }
    }
}

impl Substitution for Sub {
    fn sub_var(&self, var: VarRef) -> VarRef {
        match self.parameters.iter().position(|&x| x == var) {
            Some(i) => self.instances[i], // safe to unwrap thanks to validation in constructor
            None => var,
        }
    }
}

pub trait Substitute
where
    Self: Sized,
{
    fn substitute(&self, substitution: &impl Substitution) -> Self;
}

impl Substitute for Vec<SAtom> {
    fn substitute(&self, substitution: &impl Substitution) -> Self {
        self.iter().map(|t| substitution.ssub(*t)).collect()
    }
}
impl Substitute for Vec<Atom> {
    fn substitute(&self, substitution: &impl Substitution) -> Self {
        self.iter().map(|t| substitution.sub(*t)).collect()
    }
}

#[derive(Clone, Debug)]
pub struct Effect {
    pub transition_start: Time,
    pub persistence_start: Time,
    pub state_var: Sv,
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
    fn substitute(&self, s: &impl Substitution) -> Self {
        Effect {
            transition_start: s.fsub(self.transition_start),
            persistence_start: s.fsub(self.persistence_start),
            state_var: self.state_var.substitute(s),
            value: s.sub(self.value),
        }
    }
}

/// A condition stating that the state variable `state_var` should have the value `value`
/// over the `[start,end]` temporal interval.
///
/// in ANML: `[start,end] state_var == value`
#[derive(Debug, Clone)]
pub struct Condition {
    pub start: Time,
    pub end: Time,
    pub state_var: Sv,
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

impl Substitute for Condition {
    fn substitute(&self, s: &impl Substitution) -> Self {
        Condition {
            start: s.fsub(self.start),
            end: s.fsub(self.end),
            state_var: self.state_var.substitute(s),
            value: s.sub(self.value),
        }
    }
}

/// Represents a task, first element is the task name and the others are the parameters
/// For instance `(transport package1 loc1)`
pub type Task = Vec<SAtom>;

/// Subtask of a chronicle.
#[derive(Debug, Clone)]
pub struct SubTask {
    /// An optional identifier for the task that allows referring to it unambiguously.
    pub id: Option<String>,
    /// Time reference at which the task must start
    pub start: Time,
    /// Time reference at which the task must end
    pub end: Time,
    /// Full name of the task, including its parameters.
    pub task_name: Task,
}
impl Substitute for SubTask {
    fn substitute(&self, s: &impl Substitution) -> Self {
        SubTask {
            id: self.id.clone(),
            start: s.fsub(self.start),
            end: s.fsub(self.end),
            task_name: self.task_name.substitute(s),
        }
    }
}

/// Kind of a chronicle, related to its source in the problem definition.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ChronicleKind {
    /// Encodes part or all of the problem definition (initial facts, goals, ...)
    Problem,
    /// Represents a method, a synthetic chronicle used for encoding task decomposition
    /// or other planning process
    Method,
    /// Represents an action, its name should be part of the plan.
    Action,
    /// Represents a durative action
    DurativeAction,
}

#[derive(Clone)]
pub struct Chronicle {
    pub kind: ChronicleKind,
    /// Boolean atom indicating whether the chronicle is present in the solution.
    pub presence: Lit,
    pub start: Time,
    pub end: Time,
    /// Name and parameters of the action, e.g., `(move ?from ?to)
    /// Where the first element (name of the action template) is typically constant while
    /// the remaining elements are typically variable representing the parameters of the action.
    pub name: Sv,
    /// Task achieved by the chronicle, if different from its name.
    pub task: Option<Task>,
    pub conditions: Vec<Condition>,
    pub effects: Vec<Effect>,
    pub constraints: Vec<Constraint>,
    /// Unordered set of subtasks of the chronicle.
    /// To force an order between the subtasks, one should add to the `constraints` field boolean
    /// expression on the start/end timepoint of these subtasks.
    pub subtasks: Vec<SubTask>,
}

impl Debug for Chronicle {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        println!("Kind :{:?}", &self.kind);
        println!("Presence :{:?}", &self.presence);
        println!("Start :{:?}", &self.start);
        println!("End :{:?}", &self.end);
        println!("Name :{:?}", &self.name);
        println!("Conditions :{:?}", &self.conditions.as_slice());
        println!("Effects :{:?}", &self.effects.as_slice());
        println!("Constraints :{:?}", &self.constraints);
        Ok(())
    }
}

impl Substitute for Chronicle {
    fn substitute(&self, s: &impl Substitution) -> Self {
        Chronicle {
            kind: self.kind,
            presence: s.sub_lit(self.presence),
            start: s.fsub(self.start),
            end: s.fsub(self.end),
            name: self.name.substitute(s),
            task: self.task.as_ref().map(|t| t.substitute(s)),
            conditions: self.conditions.iter().map(|c| c.substitute(s)).collect(),
            effects: self.effects.iter().map(|e| e.substitute(s)).collect(),
            constraints: self.constraints.iter().map(|c| c.substitute(s)).collect(),
            subtasks: self.subtasks.iter().map(|c| c.substitute(s)).collect(),
        }
    }
}
