use core::fmt;
use itertools::Itertools;
use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use crate::chronicles::constraints::Constraint;
use crate::chronicles::Fluent;
use aries::core::{IntCst, Lit, VarRef};
use aries::model::lang::linear::{LinearSum, LinearTerm};
use aries::model::lang::*;

/// A state variable e.g. `(location-of robot1)` where:
///  - the fluent is the name of the state variable (e.g. `location-of`) and defines its type.
///  - the remaining elements are its parameters (e.g. `robot1`).
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct StateVar {
    pub fluent: Arc<Fluent>,
    pub args: Vec<SAtom>,
}
impl StateVar {
    pub fn new(fluent: Arc<Fluent>, args: Vec<SAtom>) -> Self {
        StateVar { fluent, args }
    }
}
impl Debug for StateVar {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.fluent)?;
        f.debug_list().entries(self.args.iter()).finish()
    }
}

/// The name of a chronicle
pub type ChronicleName = Vec<Atom>;

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

    fn sub_linear_term(&self, term: &LinearTerm) -> LinearTerm {
        LinearTerm::new(
            term.factor(),
            self.sub_ivar(term.var()),
            self.sub_lit(term.lit()),
            term.denom(),
        )
    }

    fn sub_linear_sum(&self, sum: &LinearSum) -> LinearSum {
        let mut result = LinearSum::constant_rational(sum.constant(), sum.denom());
        for term in sum.terms().iter() {
            result += self.sub_linear_term(term);
        }
        result
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

    pub fn add_untyped(&mut self, param: VarRef, instance: VarRef) -> Result<(), InvalidSubstitution> {
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
            (Atom::Fixed(a), Atom::Fixed(b)) => self.add_fixed_expr_unification(a, b),
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

    pub fn replaced_vars(&self) -> impl Iterator<Item = VarRef> + '_ {
        self.parameters.iter().copied()
    }

    pub fn replacement_vars(&self) -> impl Iterator<Item = VarRef> + '_ {
        self.instances.iter().copied().unique()
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
                write!(f, "Substitution with incompatible types {x:?} -> {y:?}")
            }
            InvalidSubstitution::DifferentLength => write!(f, "Different number of arguments in substitution"),
            InvalidSubstitution::DuplicatedEntry(v) => write!(f, "Entry {v:?} appears twice in the substitution"),
            InvalidSubstitution::IncompatibleStructures(a, b) => write!(
                f,
                "Entries {a:?} and {b:?} have different structures and cannot be unified"
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
impl Substitute for StateVar {
    fn substitute(&self, substitution: &impl Substitution) -> Self {
        StateVar {
            fluent: self.fluent.clone(),
            args: self.args.substitute(substitution),
        }
    }
}

/// The cost of a chronicle
#[derive(Clone, Debug)]
pub enum Cost {
    /// The cost is a fixed integer value
    Fixed(IntCst),
    /// The cost is a chronicle's variable
    Variable(IVar),
}

impl Substitute for Cost {
    fn substitute(&self, s: &impl Substitution) -> Self {
        match self {
            Cost::Fixed(x) => Cost::Fixed(*x),
            Cost::Variable(x) => Cost::Variable(s.sub_ivar(*x)),
        }
    }
}

/// Represents an effect on a state variable.
/// The effect has a first transition phase `]transition_start, transition_end[` during which the
/// value of the state variable is unknown.
/// Exactly at time `transition_end`, the state variable `state_var` is update with `value`
/// (assignment or increase based on `operation`).
/// For assignment effects, this value will persist until another assignment effect starts its own transition.
#[derive(Clone)]
pub struct Effect {
    /// Time at which the transition to the new value will start
    pub transition_start: Time,
    /// Time at which the transition will end
    pub transition_end: Time,
    /// If specified, the assign effect is required to persist at least until all of these timepoints.
    pub min_mutex_end: Vec<Time>,
    /// State variable affected by the effect
    pub state_var: StateVar,
    /// Operation carried out by the effect (value assignment, increase)
    pub operation: EffectOp,
}

#[derive(Clone, Eq, PartialEq)]
pub enum EffectOp {
    Assign(Atom),
    Increase(LinearSum),
}
impl EffectOp {
    pub const TRUE_ASSIGNMENT: EffectOp = EffectOp::Assign(Atom::TRUE);
    pub const FALSE_ASSIGNMENT: EffectOp = EffectOp::Assign(Atom::FALSE);
}
impl Debug for EffectOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectOp::Assign(val) => {
                write!(f, ":= {val:?}")
            }
            EffectOp::Increase(val) => {
                write!(f, "+= {:?}", val.simplify())
            }
        }
    }
}

impl Debug for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}, {:?}] {:?} {:?}",
            self.transition_start, self.transition_end, self.state_var, self.operation
        )
    }
}

impl Effect {
    pub fn effective_start(&self) -> Time {
        self.transition_end
    }
    pub fn transition_start(&self) -> Time {
        self.transition_start
    }
    pub fn variable(&self) -> &StateVar {
        &self.state_var
    }
}
impl Substitute for Effect {
    fn substitute(&self, s: &impl Substitution) -> Self {
        Effect {
            transition_start: s.fsub(self.transition_start),
            transition_end: s.fsub(self.transition_end),
            min_mutex_end: self.min_mutex_end.iter().map(|t| s.fsub(*t)).collect(),
            state_var: self.state_var.substitute(s),
            operation: self.operation.substitute(s),
        }
    }
}
impl Substitute for EffectOp {
    fn substitute(&self, substitution: &impl Substitution) -> Self {
        match self {
            EffectOp::Assign(val) => EffectOp::Assign(substitution.sub(*val)),
            EffectOp::Increase(val) => EffectOp::Increase(substitution.sub_linear_sum(val)),
        }
    }
}

/// A condition stating that the state variable `state_var` should have the value `value`
/// over the `[start,end]` temporal interval.
///
/// in ANML: `[start,end] state_var == value`
#[derive(Clone)]
pub struct Condition {
    pub start: Time,
    pub end: Time,
    pub state_var: StateVar,
    pub value: Atom,
}

impl Debug for Condition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}, {:?}] {:?} == {:?}",
            self.start, self.end, self.state_var, self.value
        )
    }
}
impl Condition {
    pub fn start(&self) -> Time {
        self.start
    }
    pub fn end(&self) -> Time {
        self.end
    }
    pub fn variable(&self) -> &StateVar {
        &self.state_var
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
pub type Task = Vec<Atom>;

/// Subtask of a chronicle.
#[derive(Clone)]
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

impl Debug for SubTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?},{:?}] {:?}", self.start, self.end, self.task_name)?;
        if let Some(ref id) = self.id {
            write!(f, " as {id}")?;
        }
        Ok(())
    }
}

/// Kind of a chronicle, related to its source in the problem definition.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
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

impl Debug for ChronicleKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChronicleKind::Problem => write!(f, "Problem"),
            ChronicleKind::Method => write!(f, "Method"),
            ChronicleKind::Action => write!(f, "Action"),
            ChronicleKind::DurativeAction => write!(f, "DurativeAction"),
        }
    }
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
    pub name: ChronicleName,
    /// Task achieved by the chronicle, if different from its name.
    pub task: Option<Task>,
    pub conditions: Vec<Condition>,
    pub effects: Vec<Effect>,
    pub constraints: Vec<Constraint>,
    /// Unordered set of subtasks of the chronicle.
    /// To force an order between the subtasks, one should add to the `constraints` field boolean
    /// expression on the start/end timepoint of these subtasks.
    pub subtasks: Vec<SubTask>,
    /// Cost of this chronicle. If left empty, it is interpreted as 0.
    pub cost: Option<Cost>,
}

struct VarSet(HashSet<VarRef>);
impl VarSet {
    fn new() -> Self {
        VarSet(HashSet::new())
    }

    fn add_lit(&mut self, l: Lit) {
        self.0.insert(l.variable());
    }

    fn add_atom(&mut self, atom: impl Into<Atom>) {
        let atom = atom.into();
        match atom {
            Atom::Bool(b) => self.0.insert(b.variable()),
            Atom::Int(i) => self.0.insert(VarRef::from(i.var)),
            Atom::Fixed(f) => self.0.insert(f.num.var.into()),
            Atom::Sym(s) => self.0.insert(s.int_view().var.into()),
        };
    }

    fn add_syms(&mut self, atoms: &[SAtom]) {
        for a in atoms {
            self.add_atom(*a);
        }
    }

    fn add_sv(&mut self, sv: &StateVar) {
        self.add_syms(&sv.args)
    }

    fn add_atoms(&mut self, atoms: &[Atom]) {
        for a in atoms {
            self.add_atom(*a)
        }
    }

    fn add_linear_term(&mut self, term: &LinearTerm) {
        self.add_atom(term.var());
        self.add_atom(term.factor());
        self.add_atom(term.denom());
        self.add_lit(term.lit());
    }

    fn add_linear_sum(&mut self, sum: &LinearSum) {
        self.add_atom(sum.constant());
        self.add_atom(sum.denom());
        for term in sum.terms() {
            self.add_linear_term(term);
        }
    }
}

impl Chronicle {
    /// Returns a set of all variables that appear in this chronicle.
    pub fn variables(&self) -> HashSet<VarRef> {
        let mut vars = VarSet::new();
        vars.add_lit(self.presence);
        vars.add_atom(self.start);
        vars.add_atom(self.end);
        vars.add_atoms(&self.name);
        if let Some(task) = &self.task {
            vars.add_atoms(task)
        }
        for cond in &self.conditions {
            vars.add_atom(cond.start);
            vars.add_atom(cond.end);
            vars.add_atom(cond.value);
            vars.add_sv(&cond.state_var)
        }
        for eff in &self.effects {
            vars.add_atom(eff.transition_start);
            vars.add_atom(eff.transition_end);
            match &eff.operation {
                EffectOp::Assign(x) => vars.add_atom(*x),
                EffectOp::Increase(x) => vars.add_linear_sum(x),
            }
            vars.add_sv(&eff.state_var)
        }
        for constraint in &self.constraints {
            for a in &constraint.variables {
                vars.add_atom(*a);
            }
        }
        for subtask in &self.subtasks {
            vars.add_atom(subtask.start);
            vars.add_atom(subtask.end);
            vars.add_atoms(&subtask.task_name)
        }

        vars.0
    }
}

impl Debug for Chronicle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn fmt_vec<T: Debug>(f: &mut std::fmt::Formatter<'_>, v: &[T]) -> std::fmt::Result {
            for e in v {
                writeln!(f, "\t{e:?}")?;
            }
            Ok(())
        }
        writeln!(f, "\nKIND : {:?}", &self.kind)?;
        writeln!(f, "PRESENCE :{:?}", &self.presence)?;
        writeln!(f, "START :{:?}", &self.start)?;
        writeln!(f, "END :{:?}", &self.end)?;
        writeln!(f, "NAME : {:?}", self.name)?;
        writeln!(f, "\nCONDITIONS :")?;
        fmt_vec(f, &self.conditions)?;
        writeln!(f, "\nEFFECTS :")?;
        fmt_vec(f, &self.effects)?;
        writeln!(f, "\nCONSTRAINTS :")?;
        fmt_vec(f, &self.constraints)?;
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
            cost: self.cost.as_ref().map(|c| c.substitute(s)),
        }
    }
}
