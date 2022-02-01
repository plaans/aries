#![allow(unused)]

pub mod upf {
    tonic::include_proto!("upf");
}
use std::fmt::Debug;
use upf::{Action, ActionInstance, Answer, Assignment, Expression, Fluent, Object, Payload, Problem, SequentialPlan};

use aries_planning::chronicles::{concrete, Chronicle, ChronicleKind};

#[derive(Default, Clone)]
pub struct Problem_ {
    pub name: String,
    pub fluents: Vec<Fluent_>,
    pub objects: Vec<Object_>,
    pub actions: Vec<Action_>,
    pub initial_state: Vec<Assignment_>,
    pub goals: Vec<Expression_>,
}

impl Problem_ {
    pub fn new() -> Self {
        Problem_ {
            name: String::default(),
            fluents: vec![Fluent_::new()],
            objects: vec![Object_::new()],
            actions: vec![Action_::new()],
            initial_state: vec![Assignment_::new()],
            goals: vec![Expression_::new()],
        }
    }

    pub fn parse_problem(msg: Problem) -> Problem_ {
        Problem_ {
            name: msg.name,
            fluents: Fluent_::parse_fluents(msg.fluents),
            objects: Object_::parse_objects(msg.objects),
            actions: Action_::parse_actions(msg.actions),
            initial_state: Assignment_::parse_assignments(msg.initial_state),
            goals: Expression_::parse_expressions(msg.goals),
        }
    }

    pub fn deserialize(msg: Problem) -> Problem_ {
        Self::parse_problem(msg)
    }

    pub fn kind(&self) -> ChronicleKind {
        ChronicleKind::Problem
    }
}

impl Debug for Problem_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn format_iter<T: Debug>(vec: Vec<T>) -> std::fmt::Result {
            for v in vec {
                println!("{:?}", v);
            }
            Result::Ok(())
        }
        write!(f, "PROBLEMS: {:?}", self.name)?;
        write!(f, "\n\nFLUENTS:\n")?;
        format_iter(self.fluents.clone())?;
        write!(f, "\n\nOBJECTS:\n")?;
        format_iter(self.objects.clone())?;
        write!(f, "\n\nACTIONS:\n")?;
        format_iter(self.actions.clone())?;
        write!(f, "\n\nINITIAL STATE:\n")?;
        format_iter(self.initial_state.clone())?;
        write!(f, "\n\nGOALS:\n")?;
        format_iter(self.goals.clone())?;

        Result::Ok(())
    }
}

#[derive(Default, Clone)]
pub struct Fluent_ {
    pub name: String,
    pub value: String,
    pub signature: Vec<String>,
}

impl Fluent_ {
    pub fn new() -> Self {
        Fluent_ {
            name: String::default(),
            value: String::default(),
            signature: vec![],
        }
    }

    pub fn parse_fluents(msg: Vec<Fluent>) -> Vec<Fluent_> {
        let mut fluents = vec![];
        for fluent in msg {
            fluents.push(Fluent_ {
                name: fluent.name,
                value: fluent.value_type,
                signature: fluent.signature,
            });
        }
        fluents
    }
}

impl Debug for Fluent_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Fluent: {},\tvalue: {},\t\tsignature: {:?}",
            self.name, self.value, self.signature
        )
    }
}

//OBJECTS
#[derive(Default, Clone)]
pub struct Object_ {
    pub name: String,
    pub type_: String,
}

impl Object_ {
    pub fn new() -> Self {
        Object_ {
            name: String::default(),
            type_: String::default(),
        }
    }

    pub fn parse_objects(msg: Vec<Object>) -> Vec<Object_> {
        let mut objects = vec![];
        for object in msg {
            objects.push(Object_ {
                name: object.name,
                type_: object.r#type,
            });
        }
        objects
    }
}

impl Debug for Object_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Object: {}\ttype: {}", self.name, self.type_)
    }
}

//ASSIGNMENTS
#[derive(Default, Clone)]
pub struct Assignment_ {
    pub x: Option<Expression_>,
    pub v: Option<Expression_>,
}

impl Assignment_ {
    pub fn new() -> Self {
        Assignment_ {
            x: Some(Expression_::new()),
            v: Some(Expression_::new()),
        }
    }

    pub fn parse_assignments(msg: Vec<Assignment>) -> Vec<Assignment_> {
        let mut assignments = vec![];
        for assignment in msg {
            assignments.push(Assignment_ {
                x: Expression_::parse_expression(assignment.x),
                v: Expression_::parse_expression(assignment.v),
            });
        }
        assignments
    }
}

impl Debug for Assignment_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "> {:?} := {:?}", self.x.as_ref().unwrap(), self.v.as_ref().unwrap())
    }
}

impl Into<Assignment> for Assignment_ {
    fn into(self) -> Assignment {
        Assignment {
            x: self.x.unwrap().into(),
            v: self.v.unwrap().into(),
        }
    }
}

//EXPRESSIONS
#[derive(Default, Clone)]
pub struct Expression_ {
    pub type_: i64,
    pub args: Vec<Expression_>,
    pub payload: Option<Payload_>,
}

impl Expression_ {
    pub fn new() -> Self {
        Expression_ {
            type_: 0,
            args: vec![Expression_::new()],
            payload: Option::None,
        }
    }

    pub fn parse_expression(msg: Option<Expression>) -> Option<Expression_> {
        match msg {
            Some(expression) => Some(Expression_ {
                type_: expression.r#type,
                args: Expression_::parse_expressions(expression.args),
                payload: Payload_::parse_payload(expression.payload),
            }),
            None => None,
        }
    }

    pub fn parse_expressions(msg: Vec<Expression>) -> Vec<Expression_> {
        let mut args = vec![];
        for arg in msg {
            args.push(Expression_ {
                type_: arg.r#type,
                args: Expression_::parse_expressions(arg.args),
                payload: Payload_::parse_payload(arg.payload),
            });
        }
        args
    }
}

impl Debug for Expression_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {:?} {:?}", self.type_, self.payload.as_ref().unwrap(), self.args,)
    }
}

impl Into<Expression> for Expression_ {
    fn into(self) -> Expression {
        Expression {
            r#type: self.type_,
            args: self.args.into_iter().map(|x| x.into()).collect(),
            payload: self.payload.unwrap().into(),
        }
    }
}

impl Into<Option<Expression>> for Expression_ {
    fn into(self) -> Option<Expression> {
        Some(self.into())
    }
}

//PAYLOADS
#[derive(Default, Clone)]
pub struct Payload_ {
    pub type_: String,
    pub value: String,
}

impl Payload_ {
    pub fn new() -> Self {
        Payload_ {
            type_: String::default(),
            value: String::default(),
        }
    }

    pub fn parse_payload(msg: Option<Payload>) -> Option<Payload_> {
        match msg {
            Some(payload) => Some(Payload_ {
                type_: payload.r#type,
                value: payload.value,
            }),
            None => None,
        }
    }
}

impl Debug for Payload_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({} {})", self.type_, self.value)?;
        Result::Ok(())
    }
}

impl Into<Payload> for Payload_ {
    fn into(self) -> Payload {
        Payload {
            r#type: self.type_,
            value: self.value,
        }
    }
}

impl Into<Option<Payload>> for Payload_ {
    fn into(self) -> Option<Payload> {
        Some(self.into())
    }
}

// ACTIONS
#[derive(Default, Clone)]
pub struct Action_ {
    pub name: String,
    pub parameters: Vec<String>,
    pub parameter_types: Vec<String>,
    pub preconditions: Vec<Expression_>,
    pub effects: Vec<Assignment_>,
}

impl Action_ {
    pub fn new() -> Self {
        Action_ {
            name: String::default(),
            parameters: vec![],
            parameter_types: vec![],
            preconditions: vec![],
            effects: vec![],
        }
    }

    pub fn parse_actions(msg: Vec<Action>) -> Vec<Action_> {
        let mut actions = vec![];
        for action in msg {
            actions.push(Action_ {
                name: action.name,
                parameters: action.parameters,
                parameter_types: action.parameter_types,
                preconditions: Expression_::parse_expressions(action.preconditions),
                effects: Assignment_::parse_assignments(action.effects),
            });
        }
        actions
    }

    pub fn parse_action(msg: Option<Action>) -> Option<Action_> {
        match msg {
            Some(action) => Some(Action_ {
                name: action.name,
                parameters: action.parameters,
                parameter_types: action.parameter_types,
                preconditions: Expression_::parse_expressions(action.preconditions),
                effects: Assignment_::parse_assignments(action.effects),
            }),
            None => None,
        }
    }

    pub fn kind(&self) -> ChronicleKind {
        ChronicleKind::Action
    }
}

impl Debug for Action_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn format_iter<T: Debug>(vec: Vec<T>) -> std::fmt::Result {
            for v in vec {
                println!("{:?}", v);
            }
            Result::Ok(())
        }
        write!(f, "Action: {}", self.name)?;
        write!(f, "\nParameters: {:?}", self.parameters)?;
        write!(f, "\nParameter types: {:?}", self.parameter_types)?;
        write!(f, "\nPreconditions:\n")?;
        format_iter(self.preconditions.clone())?;
        write!(f, "\nEffects:\n")?;
        format_iter(self.effects.clone())
    }
}

impl Into<Action> for Action_ {
    fn into(self) -> Action {
        Action {
            name: self.name,
            parameters: self.parameters,
            parameter_types: self.parameter_types,
            preconditions: self.preconditions.into_iter().map(|x| x.into()).collect(),
            effects: self.effects.into_iter().map(|x| x.into()).collect(),
        }
    }
}

impl Into<Option<Action>> for Action_ {
    fn into(self) -> Option<Action> {
        Some(self.into())
    }
}

// ACTION INSTANCE
#[derive(Default, Clone)]
pub struct ActionInstance_ {
    pub action: Option<Action_>,
    pub parameters: Vec<Expression_>,
}

impl ActionInstance_ {
    pub fn new() -> Self {
        ActionInstance_ {
            action: Option::None,
            parameters: vec![],
        }
    }

    pub fn parse_action_instances(msg: Vec<ActionInstance>) -> Vec<ActionInstance_> {
        let mut action_instances = vec![];
        for action_instance in msg {
            action_instances.push(ActionInstance_ {
                action: Action_::parse_action(action_instance.action),
                parameters: Expression_::parse_expressions(action_instance.parameters),
            });
        }
        action_instances
    }
}

impl Debug for ActionInstance_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ActionInstance: {:?}", self.action.as_ref().unwrap().name)?;
        write!(f, "\nParameters: {:?}", self.parameters)
    }
}

impl Into<ActionInstance> for ActionInstance_ {
    fn into(self) -> ActionInstance {
        ActionInstance {
            action: self.action.unwrap().into(),
            parameters: self.parameters.into_iter().map(|x| x.into()).collect(),
        }
    }
}

//SEQUENCIAL PLAN
#[derive(Default, Clone)]
pub struct SequentialPlan_ {
    pub actions: Vec<ActionInstance_>,
}

impl SequentialPlan_ {
    pub fn new() -> Self {
        SequentialPlan_ { actions: vec![] }
    }

    pub fn parse_sequential_plan(msg: Option<SequentialPlan>) -> Option<SequentialPlan_> {
        match msg {
            Some(sequential_plan) => Some(SequentialPlan_ {
                actions: ActionInstance_::parse_action_instances(sequential_plan.actions),
            }),
            None => None,
        }
    }
}

impl Into<SequentialPlan> for SequentialPlan_ {
    fn into(self) -> SequentialPlan {
        SequentialPlan {
            actions: self.actions.into_iter().map(|x| x.into()).collect(),
        }
    }
}

impl Into<Option<SequentialPlan>> for SequentialPlan_ {
    fn into(self) -> Option<SequentialPlan> {
        Some(self.into())
    }
}

impl Debug for SequentialPlan_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn format_iter<T: Debug>(vec: Vec<T>) -> std::fmt::Result {
            for v in vec {
                println!("{:?}", v);
            }
            Result::Ok(())
        }
        write!(f, "SequentialPlan:")?;
        format_iter(self.actions.clone())
    }
}

//ANSWER
#[derive(Default, Clone)]
pub struct Answer_ {
    pub status: i32,
    pub plan: Option<SequentialPlan_>,
}

impl Answer_ {
    pub fn new() -> Self {
        Answer_ {
            status: 0,
            plan: Option::None,
        }
    }

    pub fn parse_answer(msg: Option<Answer>) -> Option<Answer_> {
        match msg {
            Some(answer) => Some(Answer_ {
                status: answer.status,
                plan: SequentialPlan_::parse_sequential_plan(answer.plan),
            }),
            None => None,
        }
    }

    pub fn serialize(&self) -> Answer {
        Answer {
            status: self.status,
            plan: self.plan.clone().map(|x| x.into()),
        }
    }
}
