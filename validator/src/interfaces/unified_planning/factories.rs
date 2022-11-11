#![allow(dead_code)]
#![allow(clippy::new_ret_no_self)]
use unified_planning::{atom::Content, effect_expression::EffectKind, *};

use crate::interfaces::unified_planning::constants::UP_EQUALS;

use super::constants::{UP_BOOL, UP_INTEGER, UP_REAL};

fn symbol(s: &str) -> Content {
    Content::Symbol(s.into())
}
fn int(i: i64) -> Content {
    Content::Int(i)
}
fn real(numerator: i64, denominator: i64) -> Content {
    Content::Real(Real { numerator, denominator })
}
fn boolean(b: bool) -> Content {
    Content::Boolean(b)
}

pub struct ActionFactory;
impl ActionFactory {
    pub fn new(n: &str, parameters: Vec<Parameter>, conditions: Vec<Condition>, effects: Vec<Effect>) -> Action {
        Action {
            name: n.into(),
            parameters,
            duration: None,
            conditions,
            effects,
        }
    }
}

pub struct ConditionFactory;
impl ConditionFactory {
    pub fn new(e: Expression) -> Condition {
        Condition {
            cond: Some(e),
            span: None,
        }
    }
}

pub struct EffectFactory;
impl EffectFactory {
    pub fn new(k: EffectKind, sv: Expression, v: Expression, condition: Option<Expression>) -> Effect {
        Effect {
            effect: Some(EffectExpression {
                kind: k.into(),
                fluent: Some(sv),
                value: Some(v),
                condition,
            }),
            occurrence_time: None,
        }
    }

    pub fn assign(sv: Expression, v: Expression, condition: Option<Expression>) -> Effect {
        Self::new(EffectKind::Assign, sv, v, condition)
    }

    pub fn increase(sv: Expression, v: Expression, condition: Option<Expression>) -> Effect {
        Self::new(EffectKind::Increase, sv, v, condition)
    }

    pub fn decrease(sv: Expression, v: Expression, condition: Option<Expression>) -> Effect {
        Self::new(EffectKind::Decrease, sv, v, condition)
    }
}

pub struct ExpressionFactory;
impl ExpressionFactory {
    pub fn unknown() -> Expression {
        Expression {
            kind: ExpressionKind::Unknown.into(),
            ..Default::default()
        }
    }

    pub fn atom(c: Content, t: &str, k: ExpressionKind) -> Expression {
        Expression {
            atom: Some(Atom { content: Some(c) }),
            r#type: t.into(),
            kind: k.into(),
            ..Default::default()
        }
    }

    pub fn list(list: Vec<Expression>, k: ExpressionKind) -> Expression {
        Expression {
            list,
            kind: k.into(),
            ..Default::default()
        }
    }

    pub fn constant(c: Content, t: &str) -> Expression {
        Self::atom(c, t, ExpressionKind::Constant)
    }

    pub fn symbol(s: &str, t: &str) -> Expression {
        Self::constant(symbol(s), t)
    }

    pub fn int(i: i64) -> Expression {
        Self::constant(int(i), UP_INTEGER)
    }

    pub fn real(numerator: i64, denominator: i64) -> Expression {
        Self::constant(real(numerator, denominator), UP_REAL)
    }

    pub fn boolean(b: bool) -> Expression {
        Self::constant(boolean(b), UP_BOOL)
    }

    pub fn parameter(s: &str, t: &str) -> Expression {
        Self::atom(symbol(s), t, ExpressionKind::Parameter)
    }

    pub fn variable(t: &str, n: &str) -> Expression {
        Self::atom(symbol(n), t, ExpressionKind::Variable)
    }

    pub fn fluent_symbol(s: &str) -> Expression {
        Self::atom(symbol(s), "", ExpressionKind::FluentSymbol)
    }

    pub fn function_symbol(s: &str) -> Expression {
        Self::atom(symbol(s), "", ExpressionKind::FunctionSymbol)
    }

    pub fn state_variable(args: Vec<Expression>) -> Expression {
        Self::list(args, ExpressionKind::StateVariable)
    }

    pub fn function_application(args: Vec<Expression>) -> Expression {
        Self::list(args, ExpressionKind::FunctionApplication)
    }

    pub fn container_id() -> Expression {
        Expression {
            kind: ExpressionKind::ContainerId.into(),
            ..Default::default()
        }
    }
}

pub struct ObjectFactory;
impl ObjectFactory {
    pub fn new(n: &str, t: &str) -> ObjectDeclaration {
        ObjectDeclaration {
            name: n.into(),
            r#type: t.into(),
        }
    }
}

pub struct ParameterFactory;
impl ParameterFactory {
    pub fn new(n: &str, t: &str) -> Parameter {
        Parameter {
            name: n.into(),
            r#type: t.into(),
        }
    }
}

pub struct PlanFactory;
impl PlanFactory {
    pub fn mock() -> Plan {
        let robot_type = "robot";
        let r1 = "R1";
        let loc_type = "location";
        let loc1 = "L1";
        let loc2 = "L2";
        let move_action = "move";

        Plan {
            actions: vec![ActionInstance {
                id: "a1".into(),
                action_name: move_action.into(),
                parameters: vec![
                    ExpressionFactory::symbol(r1, robot_type).atom.unwrap(),
                    ExpressionFactory::symbol(loc1, loc_type).atom.unwrap(),
                    ExpressionFactory::symbol(loc2, loc_type).atom.unwrap(),
                ],
                start_time: None,
                end_time: None,
            }],
        }
    }
}

pub struct ProblemFactory;
impl ProblemFactory {
    pub fn mock() -> Problem {
        let robot_type = "robot";
        let robot_param = "r";
        let r1 = "R1";
        let loc_type = "location";
        let loc_fluent = "loc";
        let loc1 = "L1";
        let loc2 = "L2";
        let move_action = "move";

        let loc_robot = ExpressionFactory::state_variable(vec![
            ExpressionFactory::fluent_symbol(loc_fluent),
            ExpressionFactory::parameter(robot_param, robot_type),
        ]);

        Problem {
            domain_name: "domain".into(),
            problem_name: "problem".into(),
            types: vec![
                TypeDeclaration {
                    type_name: robot_type.into(),
                    parent_type: "".into(),
                },
                TypeDeclaration {
                    type_name: loc_type.into(),
                    parent_type: "".into(),
                },
            ],
            fluents: vec![Fluent {
                name: loc_fluent.into(),
                value_type: loc_type.into(),
                parameters: vec![ParameterFactory::new(robot_param, robot_type)],
                default_value: Some(ExpressionFactory::symbol(loc1, loc_type)),
            }],
            objects: vec![
                ObjectFactory::new(r1, robot_type),
                ObjectFactory::new(loc1, loc_type),
                ObjectFactory::new(loc2, loc_type),
            ],
            actions: vec![ActionFactory::new(
                move_action,
                vec![
                    ParameterFactory::new(robot_param, robot_type),
                    ParameterFactory::new("from", loc_type),
                    ParameterFactory::new("to", loc_type),
                ],
                vec![ConditionFactory::new(ExpressionFactory::function_application(vec![
                    ExpressionFactory::function_symbol(UP_EQUALS),
                    loc_robot.clone(),
                    ExpressionFactory::parameter("from", loc_type),
                ]))],
                vec![EffectFactory::assign(
                    loc_robot,
                    ExpressionFactory::parameter("to", loc_type),
                    None,
                )],
            )],
            initial_state: vec![Assignment {
                fluent: Some(ExpressionFactory::state_variable(vec![
                    ExpressionFactory::fluent_symbol(loc_fluent),
                    ExpressionFactory::parameter(r1, robot_type),
                ])),
                value: Some(ExpressionFactory::symbol(loc1, loc_type)),
            }],
            timed_effects: vec![],
            goals: vec![Goal {
                goal: Some(ExpressionFactory::function_application(vec![
                    ExpressionFactory::function_symbol(UP_EQUALS),
                    ExpressionFactory::state_variable(vec![
                        ExpressionFactory::fluent_symbol(loc_fluent),
                        ExpressionFactory::parameter(r1, robot_type),
                    ]),
                    ExpressionFactory::parameter(loc2, loc_type),
                ])),
                timing: None,
            }],
            features: vec![],
            metrics: vec![],
            hierarchy: None,
        }
    }
}
