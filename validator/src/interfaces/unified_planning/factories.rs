use unified_planning::{atom::Content, effect_expression::EffectKind, *};

use super::constants::{UP_BOOL, UP_EQUALS, UP_INTEGER, UP_REAL};

mod content {
    use super::*;

    pub fn symbol(s: &str) -> Content {
        Content::Symbol(s.into())
    }
    pub fn int(i: i64) -> Content {
        Content::Int(i)
    }
    pub fn real(numerator: i64, denominator: i64) -> Content {
        Content::Real(Real { numerator, denominator })
    }
    pub fn boolean(b: bool) -> Content {
        Content::Boolean(b)
    }
}

pub mod action {
    use super::*;

    pub fn action(n: &str, parameters: Vec<Parameter>, conditions: Vec<Condition>, effects: Vec<Effect>) -> Action {
        Action {
            name: n.into(),
            parameters,
            duration: None,
            conditions,
            effects,
        }
    }
}

pub mod condition {
    use super::*;

    pub fn condition(e: Expression) -> Condition {
        Condition {
            cond: Some(e),
            span: None,
        }
    }
}

pub mod effect {
    use super::*;

    pub fn effect(k: EffectKind, sv: Expression, v: Expression, condition: Option<Expression>) -> Effect {
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
        effect(EffectKind::Assign, sv, v, condition)
    }
}

pub mod expression {
    use super::*;

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
        atom(c, t, ExpressionKind::Constant)
    }

    pub fn symbol(s: &str, t: &str) -> Expression {
        constant(super::content::symbol(s), t)
    }

    pub fn int(i: i64) -> Expression {
        constant(super::content::int(i), UP_INTEGER)
    }

    pub fn real(numerator: i64, denominator: i64) -> Expression {
        constant(super::content::real(numerator, denominator), UP_REAL)
    }

    pub fn boolean(b: bool) -> Expression {
        constant(super::content::boolean(b), UP_BOOL)
    }

    pub fn parameter(s: &str, t: &str) -> Expression {
        atom(super::content::symbol(s), t, ExpressionKind::Parameter)
    }

    pub fn variable(t: &str, n: &str) -> Expression {
        atom(super::content::symbol(n), t, ExpressionKind::Variable)
    }

    pub fn fluent_symbol(s: &str) -> Expression {
        atom(super::content::symbol(s), "", ExpressionKind::FluentSymbol)
    }

    pub fn function_symbol(s: &str) -> Expression {
        atom(super::content::symbol(s), "", ExpressionKind::FunctionSymbol)
    }

    pub fn state_variable(args: Vec<Expression>) -> Expression {
        list(args, ExpressionKind::StateVariable)
    }

    pub fn function_application(args: Vec<Expression>) -> Expression {
        list(args, ExpressionKind::FunctionApplication)
    }

    pub fn container_id() -> Expression {
        Expression {
            kind: ExpressionKind::ContainerId.into(),
            ..Default::default()
        }
    }
}

pub mod object {
    use super::*;

    pub fn object(n: &str, t: &str) -> ObjectDeclaration {
        ObjectDeclaration {
            name: n.into(),
            r#type: t.into(),
        }
    }
}

pub mod parameter {
    use super::*;

    pub fn parameter(n: &str, t: &str) -> Parameter {
        Parameter {
            name: n.into(),
            r#type: t.into(),
        }
    }
}

pub mod plan {
    use super::*;

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
                    expression::symbol(r1, robot_type).atom.unwrap(),
                    expression::symbol(loc1, loc_type).atom.unwrap(),
                    expression::symbol(loc2, loc_type).atom.unwrap(),
                ],
                start_time: None,
                end_time: None,
            }],
        }
    }
}

pub mod problem {
    use super::*;

    pub fn mock() -> Problem {
        let locatable_type = "locatable";
        let robot_type = "robot";
        let robot_param = "r";
        let r1 = "R1";
        let loc_type = "location";
        let loc_fluent = "loc";
        let loc1 = "L1";
        let loc2 = "L2";
        let move_action = "move";

        let loc_robot = expression::state_variable(vec![
            expression::fluent_symbol(loc_fluent),
            expression::parameter(robot_param, robot_type),
        ]);

        Problem {
            domain_name: "domain".into(),
            problem_name: "problem".into(),
            types: vec![
                TypeDeclaration {
                    type_name: locatable_type.into(),
                    parent_type: "".into(),
                },
                TypeDeclaration {
                    type_name: robot_type.into(),
                    parent_type: locatable_type.into(),
                },
                TypeDeclaration {
                    type_name: loc_type.into(),
                    parent_type: locatable_type.into(),
                },
            ],
            fluents: vec![Fluent {
                name: loc_fluent.into(),
                value_type: loc_type.into(),
                parameters: vec![parameter::parameter(robot_param, robot_type)],
                default_value: Some(expression::symbol(loc1, loc_type)),
            }],
            objects: vec![
                object::object(r1, robot_type),
                object::object(loc1, loc_type),
                object::object(loc2, loc_type),
            ],
            actions: vec![action::action(
                move_action,
                vec![
                    parameter::parameter(robot_param, robot_type),
                    parameter::parameter("from", loc_type),
                    parameter::parameter("to", loc_type),
                ],
                vec![condition::condition(expression::function_application(vec![
                    expression::function_symbol(UP_EQUALS),
                    loc_robot.clone(),
                    expression::parameter("from", loc_type),
                ]))],
                vec![effect::assign(loc_robot, expression::parameter("to", loc_type), None)],
            )],
            initial_state: vec![Assignment {
                fluent: Some(expression::state_variable(vec![
                    expression::fluent_symbol(loc_fluent),
                    expression::parameter(r1, robot_type),
                ])),
                value: Some(expression::symbol(loc1, loc_type)),
            }],
            timed_effects: vec![],
            goals: vec![Goal {
                goal: Some(expression::function_application(vec![
                    expression::function_symbol(UP_EQUALS),
                    expression::state_variable(vec![
                        expression::fluent_symbol(loc_fluent),
                        expression::parameter(r1, robot_type),
                    ]),
                    expression::parameter(loc2, loc_type),
                ])),
                timing: None,
            }],
            features: vec![],
            metrics: vec![],
            hierarchy: None,
        }
    }
}
