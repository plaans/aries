use crate::chronicles::constraints::Constraint;
use aries_model::lang::*;
use serde::export::Formatter;

pub type SV = Vec<SAtom>;
type Time = IAtom;

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

    fn sub(&self, atom: Atom) -> Atom {
        match atom {
            Atom::Bool(b) => self.bsub(b).into(),
            Atom::Int(i) => self.isub(i).into(),
            Atom::Sym(s) => self.ssub(s).into(),
        }
    }

    fn isub(&self, i: IAtom) -> IAtom {
        match i.var {
            Some(x) => IAtom::new(Some(self.sub_ivar(x)), i.shift),
            None => IAtom::new(None, i.shift),
        }
    }
    fn bsub(&self, b: BAtom) -> BAtom {
        match b {
            BAtom::Cst(b) => BAtom::Cst(b),
            BAtom::Var { var, negated } => BAtom::Var {
                var: self.sub_bvar(var),
                negated,
            },
            BAtom::Expr(_) => panic!("UNSUPPORTED substitution in an expression"),
        }
    }

    fn ssub(&self, s: SAtom) -> SAtom {
        match s {
            SAtom::Var(v) => SAtom::Var(self.sub_svar(v)),
            SAtom::Cst(s) => SAtom::Cst(s),
        }
    }
}

/// A substitution of params by instances.
/// The constructor validated the input to make sure that the parameters and instances are of the same kind.
pub struct Sub {
    parameters: Vec<VarRef>,
    instances: Vec<VarRef>,
}
impl Sub {
    pub fn new(params: &[Variable], instances: &[Variable]) -> Result<Self, InvalidSubstitution> {
        if params.len() != instances.len() {
            return Err(InvalidSubstitution::DifferentLength);
        }
        for i in 0..params.len() {
            if params[i].kind() != instances[i].kind() {
                return Err(InvalidSubstitution::IncompatibleTypes(params[i], instances[i]));
            }
        }
        Ok(Sub {
            parameters: params.iter().copied().map(VarRef::from).collect(),
            instances: instances.iter().copied().map(VarRef::from).collect(),
        })
    }
}
#[derive(Debug)]
pub enum InvalidSubstitution {
    IncompatibleTypes(Variable, Variable),
    DifferentLength,
}
impl std::error::Error for InvalidSubstitution {}
impl std::fmt::Display for InvalidSubstitution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            InvalidSubstitution::IncompatibleTypes(x, y) => {
                write!(f, "Substitution with incomaptible types {:?} -> {:?}", x, y)
            }
            InvalidSubstitution::DifferentLength => write!(f, "Different number of arguments in substitution"),
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
    fn substitute(&self, s: &impl Substitution) -> Self {
        Effect {
            transition_start: s.isub(self.transition_start),
            persistence_start: s.isub(self.persistence_start),
            state_var: self.state_var.substitute(s),
            value: s.sub(self.value),
        }
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

impl Substitute for Condition {
    fn substitute(&self, s: &impl Substitution) -> Self {
        Condition {
            start: s.isub(self.start),
            end: s.isub(self.end),
            state_var: self.state_var.substitute(s),
            value: s.sub(self.value),
        }
    }
}

/// Represents a task, first element is the task name and the others are the parameters
pub type Task = Vec<SAtom>;

/// Subtask of a chronicle.
#[derive(Clone)]
pub struct SubTask {
    pub id: Option<String>,
    pub start: Time,
    pub end: Time,
    pub task: Task,
}
impl Substitute for SubTask {
    fn substitute(&self, s: &impl Substitution) -> Self {
        SubTask {
            id: self.id.clone(),
            start: s.isub(self.start),
            end: s.isub(self.end),
            task: self.task.substitute(s),
        }
    }
}

#[derive(Clone)]
pub struct Chronicle {
    pub presence: BAtom,
    pub start: Time,
    pub end: Time,
    pub name: SV,
    pub task: Option<Task>,
    pub conditions: Vec<Condition>,
    pub effects: Vec<Effect>,
    pub constraints: Vec<Constraint>,
    pub subtasks: Vec<SubTask>,
}

impl Substitute for Chronicle {
    fn substitute(&self, s: &impl Substitution) -> Self {
        Chronicle {
            presence: s.bsub(self.presence),
            start: s.isub(self.start),
            end: s.isub(self.end),
            name: self.name.substitute(s),
            task: self.task.as_ref().map(|t| t.substitute(s)),
            conditions: self.conditions.iter().map(|c| c.substitute(s)).collect(),
            effects: self.effects.iter().map(|e| e.substitute(s)).collect(),
            constraints: self.constraints.iter().map(|c| c.substitute(s)).collect(),
            subtasks: self.subtasks.iter().map(|c| c.substitute(s)).collect(),
        }
    }
}
