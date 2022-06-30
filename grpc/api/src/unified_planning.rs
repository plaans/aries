// =============== Types ================

// Type of expressions are represented as strings in protobuf format.
// A type might be, e.g., "int", "bool" or "location", where the latter is a problem-specific type.

// Built-in types are namespaced with the `up:` prefix:
//  - "up:bool"
//  - "up:integer"
//  - "up:real"
//  - "up:time"
//
// Any other string (e.g. "location") refers to a symbolic type and must have been declared in the problem definition.

// We can also consider restrictions to int/reals with specific syntax (e.g. "up:integer\[0,100\]").

// ================== Expressions ====================

/// As in s-expression, an Expression is either an atom or list representing the application of some parameters to a function/fluent.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Expression {
    /// If non-empty, the expression is a single atom.
    /// For instance `3`, `+`, `kitchen`, `at-robot`, ...
    #[prost(message, optional, tag="1")]
    pub atom: ::core::option::Option<Atom>,
    /// If the `atom` field is empty, then the expression is a list of sub-expressions,
    /// typically representing the application of some arguments to a function or fluent.
    /// For instance `(+ 1 3)`, (at-robot l1)`, `(>= (battery_level) 20)`
    #[prost(message, repeated, tag="2")]
    pub list: ::prost::alloc::vec::Vec<Expression>,
    /// Type of the expression. For instance "int", "location", ...
    #[prost(string, tag="3")]
    pub r#type: ::prost::alloc::string::String,
    /// Kind of the expression, specifying the content of the expression.
    /// This is intended to facilitate parsing of the expression.
    #[prost(enumeration="ExpressionKind", tag="4")]
    pub kind: i32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Atom {
    #[prost(oneof="atom::Content", tags="1, 2, 3, 4")]
    pub content: ::core::option::Option<atom::Content>,
}
/// Nested message and enum types in `Atom`.
pub mod atom {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Content {
        #[prost(string, tag="1")]
        Symbol(::prost::alloc::string::String),
        #[prost(int64, tag="2")]
        Int(i64),
        #[prost(message, tag="3")]
        Real(super::Real),
        #[prost(bool, tag="4")]
        Boolean(bool),
    }
}
/// Representation of a constant real number, as the fraction `(numerator / denominator)`.
/// A real should be in its canonical form (with smallest possible denominator).
/// Notably, if this number is an integer, then it is guaranteed that `denominator == 1`.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Real {
    #[prost(int64, tag="1")]
    pub numerator: i64,
    #[prost(int64, tag="2")]
    pub denominator: i64,
}
// ============= Domains ====================

/// Declares the existence of a symbolic type.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TypeDeclaration {
    /// Name of the type that is declared.
    #[prost(string, tag="1")]
    pub type_name: ::prost::alloc::string::String,
    /// Optional. If the string is non-empty, this is the parent type of `type_name`.
    /// If set, the parent type must have been previously declared (i.e. should appear earlier in the problem's type declarations.
    /// feature: HIERARCHICAL_TYPING
    #[prost(string, tag="2")]
    pub parent_type: ::prost::alloc::string::String,
}
/// Parameter of a fluent or of an action
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Parameter {
    /// Name of the parameter.
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Type of the parameter.
    #[prost(string, tag="2")]
    pub r#type: ::prost::alloc::string::String,
}
/// A state-dependent variable.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Fluent {
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Return type of the fluent.
    #[prost(string, tag="2")]
    pub value_type: ::prost::alloc::string::String,
    /// Typed and named parameters of the fluent.
    #[prost(message, repeated, tag="3")]
    pub parameters: ::prost::alloc::vec::Vec<Parameter>,
    /// If non-empty, then any state variable using this fluent that is not explicitly given a value in the initial state
    /// will be assumed to have this default value.
    /// This allows mimicking the closed world assumption by setting a "false" default value to predicates.
    /// Note that in the initial state of the problem message, it is assumed that all default values are set.
    #[prost(message, optional, tag="4")]
    pub default_value: ::core::option::Option<Expression>,
}
/// Declares an object with the given name and type.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ObjectDeclaration {
    /// Name of the object.
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Type of the object.
    /// The type must have been previously declared in the problem definition.
    #[prost(string, tag="2")]
    pub r#type: ::prost::alloc::string::String,
}
// ========= Actions ========

/// An effect expression is of the form `FLUENT OP VALUE`.
/// We explicitly restrict the different types of effects by setting the allowed operators.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EffectExpression {
    #[prost(enumeration="effect_expression::EffectKind", tag="1")]
    pub kind: i32,
    /// Expression that must be of the STATE_VARIABLE kind.
    #[prost(message, optional, tag="2")]
    pub fluent: ::core::option::Option<Expression>,
    #[prost(message, optional, tag="3")]
    pub value: ::core::option::Option<Expression>,
    /// Optional. If the effect is conditional, then the following field must be set.
    /// In this case, the `effect` will only be applied if the `condition`` holds.
    /// If the effect is unconditional, the effect is set to the constant 'true' value.
    /// features: CONDITIONAL_EFFECT
    #[prost(message, optional, tag="4")]
    pub condition: ::core::option::Option<Expression>,
}
/// Nested message and enum types in `EffectExpression`.
pub mod effect_expression {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum EffectKind {
        /// The `fluent` is set to the corresponding `value`
        Assign = 0,
        /// The `fluent` is increased by the amount `value`
        /// features: INCREASE_EFFECTS
        Increase = 1,
        /// The `fluent` is decreased by the amount `value`
        /// features: DECREASE_EFFECTS
        Decrease = 2,
    }
}
/// Representation of an effect that allows qualifying the effect expression, e.g., to make it a conditional effect.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Effect {
    /// Required. The actual effect that should take place.
    #[prost(message, optional, tag="1")]
    pub effect: ::core::option::Option<EffectExpression>,
    /// Optional. If the effect is within a durative action, the following must be set and will specify when the effect takes place.
    /// features: DURATIVE_ACTIONS
    #[prost(message, optional, tag="2")]
    pub occurrence_time: ::core::option::Option<Timing>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Condition {
    #[prost(message, optional, tag="1")]
    pub cond: ::core::option::Option<Expression>,
    /// Optional. Must be set for durative actions where it specifies the temporal interval
    /// over which when the condition should hold.
    /// features: DURATIVE_ACTIONS
    #[prost(message, optional, tag="2")]
    pub span: ::core::option::Option<TimeInterval>,
}
/// Unified action representation that represents any kind of actions.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Action {
    /// Action name. E.g. "move"
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Typed and named parameters of the action.
    #[prost(message, repeated, tag="2")]
    pub parameters: ::prost::alloc::vec::Vec<Parameter>,
    /// If set, the action is durative. Otherwise it is instantaneous.
    /// features: DURATIVE_ACTIONS
    #[prost(message, optional, tag="3")]
    pub duration: ::core::option::Option<Duration>,
    /// Conjunction of conditions that must hold for the action to be applicable.
    #[prost(message, repeated, tag="4")]
    pub conditions: ::prost::alloc::vec::Vec<Condition>,
    /// Conjunction of effects as a result of applying this action.
    #[prost(message, repeated, tag="5")]
    pub effects: ::prost::alloc::vec::Vec<Effect>,
}
/// Symbolic reference to an absolute time.
/// It might represent:
/// - the time of the initial/final state, or
/// - the start/end of the containing action.
///
/// It is currently composed of a single field whose interpretation might be context dependent
/// (e.g. "START" refers to the start of the containing action).
///
/// In the future, it could be extended to refer, e.g., to the start of a particular action/subtask
/// by adding an additional field with the identifier of an action/subtask.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Timepoint {
    #[prost(enumeration="timepoint::TimepointKind", tag="1")]
    pub kind: i32,
    /// If non-empty, identifies the container of which we are extracting the start/end timepoint.
    /// In the context of a task-network or of a method, this could be the `id` of one of the subtasks.
    /// feature: hierarchies
    #[prost(string, tag="2")]
    pub container_id: ::prost::alloc::string::String,
}
/// Nested message and enum types in `Timepoint`.
pub mod timepoint {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum TimepointKind {
        /// Global start of the planning problem. This is context independent and represents the time at which the initial state holds.
        GlobalStart = 0,
        /// Global end of the planning problem. This is context independent and represents the time at which the final state holds.
        GlobalEnd = 1,
        /// Start of the container (typically the action or method) in which this symbol occurs
        Start = 2,
        /// End of the container (typically the action or method) in which this symbol occurs
        End = 3,
    }
}
/// Represents a time (`timepoint` + `delay`), that is a time defined relatively to a particular `timepoint`.
/// Note that an absolute time can be defined by setting the `delay` relative to the `GLOBAL_START`` which is the reference time.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Timing {
    #[prost(message, optional, tag="1")]
    pub timepoint: ::core::option::Option<Timepoint>,
    #[prost(message, optional, tag="2")]
    pub delay: ::core::option::Option<Real>,
}
/// An interval `[lower, upper]` where `lower` and `upper` are arbitrary expressions.
/// The `is_left_open` and `is_right_open` fields indicate whether the interval is
/// opened on left and right side respectively.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Interval {
    #[prost(bool, tag="1")]
    pub is_left_open: bool,
    #[prost(message, optional, tag="2")]
    pub lower: ::core::option::Option<Expression>,
    #[prost(bool, tag="3")]
    pub is_right_open: bool,
    #[prost(message, optional, tag="4")]
    pub upper: ::core::option::Option<Expression>,
}
/// A contiguous slice of time represented as an interval `[lower, upper]` where `lower` and `upper` are time references.
/// The `is_left_open` and `is_right_open` fields indicate whether the interval is
/// opened on left and right side respectively.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TimeInterval {
    #[prost(bool, tag="1")]
    pub is_left_open: bool,
    #[prost(message, optional, tag="2")]
    pub lower: ::core::option::Option<Timing>,
    #[prost(bool, tag="3")]
    pub is_right_open: bool,
    #[prost(message, optional, tag="4")]
    pub upper: ::core::option::Option<Timing>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Duration {
    //// The duration of the action can be freely chosen within the indicated bounds
    #[prost(message, optional, tag="1")]
    pub controllable_in_bounds: ::core::option::Option<Interval>,
}
// ============== Hierarchies ====================

/// Declares an abstract task together with its expected parameters.
///
/// Example: goto(robot: Robot, destination: Location)
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AbstractTaskDeclaration {
    /// Example: "goto"
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Example:
    ///  - robot: Robot
    ///  - destination: Location
    #[prost(message, repeated, tag="2")]
    pub parameters: ::prost::alloc::vec::Vec<Parameter>,
}
/// Representation of an abstract or primitive task that should be achieved,
/// required either in the initial task network or as a subtask of a method.
///
/// Example:  task of sending a `robot` to the KITCHEN
///   - t1: goto(robot, KITCHEN)
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Task {
    /// Identifier of the task, required to be unique in the method/task-network where the task appears.
    /// The `id` is notably used to refer to the start/end of the task.
    ///
    /// Example: t1
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    /// Name of the task that should be achieved. It might either
    ///  - an abstract task if the name is the one of a task declared in the problem
    ///  - a primitive task if the name is the one of an action declared in the problem
    ///
    /// Example:
    ///  - "goto" (abstract task)
    ///  - "move" (action / primitive task)
    #[prost(string, tag="2")]
    pub task_name: ::prost::alloc::string::String,
    /// Example: (for a "goto" task)
    ///  - robot    (a parameter from an outer scope)
    ///  - KITCHEN  (a constant symbol in the problem)
    #[prost(message, repeated, tag="3")]
    pub parameters: ::prost::alloc::vec::Vec<Expression>,
}
/// A method describes one possible way of achieving a task.
///
/// Example: A method that make a "move" action and recursively calls itself until reaching the destination.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Method {
    /// A name that uniquely identify the method.
    /// This is mostly used for user facing output or plan validation.
    ///
    /// Example: "m-recursive-goto"
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
    /// Example: [robot: Robot, source: Location, intermediate: Location, destination: Location]
    #[prost(message, repeated, tag="2")]
    pub parameters: ::prost::alloc::vec::Vec<Parameter>,
    /// The task that is achieved by the method.
    /// A subset of the parameters of the method will typically be used to
    /// define the task that is achieved.
    ///
    /// Example: goto(robot, destination)
    #[prost(message, optional, tag="3")]
    pub achieved_task: ::core::option::Option<Task>,
    /// A set of subtasks that should be achieved to carry out the method.
    /// Note that the order of subtasks is irrelevant and that any ordering constraint should be
    /// specified in the `constraints` field.
    ///
    /// Example:
    ///  - t1: (move robot source intermediate)
    ///  - t2: goto(robot destination)
    #[prost(message, repeated, tag="4")]
    pub subtasks: ::prost::alloc::vec::Vec<Task>,
    /// Constraints enable the definition of ordering constraints as well as constraints
    /// on the allowed instantiation of the method's parameters.
    ///
    /// Example:
    ///  - end(t1) < start(t2)
    ///  - source != intermediate
    #[prost(message, repeated, tag="5")]
    pub constraints: ::prost::alloc::vec::Vec<Expression>,
    /// Conjunction of conditions that must hold for the method to be applicable.
    /// As for the conditions of actions, these can be temporally qualified to refer to intermediate timepoints.
    /// In addition to the start/end of the method, the temporal qualification might refer to the start/end of
    /// one of the subtasks using its identifier.
    ///
    /// Example:
    ///  - \[start\] loc(robot) == source
    ///  - \[end(t1)\] loc(robot) == intermediate
    ///  - \[end\] loc(robot) == destination
    #[prost(message, repeated, tag="6")]
    pub conditions: ::prost::alloc::vec::Vec<Condition>,
}
/// A task network defines a set of subtasks and associated constraints.
/// It is intended to be used to define the initial task network of the hierarchical problem.
///
/// Example: an arbitrary robot should go to the KITCHEN before time 100
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TaskNetwork {
    /// robot: Location
    #[prost(message, repeated, tag="1")]
    pub variables: ::prost::alloc::vec::Vec<Parameter>,
    /// t1: goto(robot, KITCHEN)
    #[prost(message, repeated, tag="2")]
    pub subtasks: ::prost::alloc::vec::Vec<Task>,
    /// end(t1) <= 100
    #[prost(message, repeated, tag="3")]
    pub constraints: ::prost::alloc::vec::Vec<Expression>,
}
/// Represents the hierarchical part of a problem.
/// features: hierarchical
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Hierarchy {
    #[prost(message, repeated, tag="1")]
    pub abstract_tasks: ::prost::alloc::vec::Vec<AbstractTaskDeclaration>,
    #[prost(message, repeated, tag="2")]
    pub methods: ::prost::alloc::vec::Vec<Method>,
    #[prost(message, optional, tag="3")]
    pub initial_task_network: ::core::option::Option<TaskNetwork>,
}
// ============== Problem =========================

/// A Goal is currently an expression that must hold either:
/// - in the final state,
/// - over a specific temporal interval (under the `timed_goals` features)
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Goal {
    /// Goal expression that must hold in the final state.
    #[prost(message, optional, tag="1")]
    pub goal: ::core::option::Option<Expression>,
    /// Optional. If specified the goal should hold over the specified temporal interval (instead of on the final state).
    /// features: TIMED_GOALS
    #[prost(message, optional, tag="2")]
    pub timing: ::core::option::Option<TimeInterval>,
}
/// Represents an effect that will occur sometime beyond the initial state. (similar to timed initial literals)
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TimedEffect {
    /// Required. An effect expression that will take place sometime in the future (i.e. not at the intial state) as specified by the temporal qualifiation.
    #[prost(message, optional, tag="1")]
    pub effect: ::core::option::Option<EffectExpression>,
    /// Required. Temporal qualification denoting when the timed fact will occur.
    #[prost(message, optional, tag="2")]
    pub occurrence_time: ::core::option::Option<Timing>,
}
/// An assignment of a value to a fluent, as it appears in the initial state definition.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Assignment {
    /// State variable that is assigned the `value`.
    /// It should be an expression of the STATE_VARIABLE kind for which all parameters are of the CONSTANT kind.
    #[prost(message, optional, tag="1")]
    pub fluent: ::core::option::Option<Expression>,
    /// An expression of the CONSTANT kind, denoting the value take by the state variable.
    #[prost(message, optional, tag="2")]
    pub value: ::core::option::Option<Expression>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Metric {
    #[prost(enumeration="metric::MetricKind", tag="1")]
    pub kind: i32,
    /// Expression to minimize/maximize in the final state.
    /// Empty, if the `kind` is not {MIN/MAX}IMIZE_EXPRESSION_ON_FINAL_STATE
    #[prost(message, optional, tag="2")]
    pub expression: ::core::option::Option<Expression>,
    /// If `kind == MINIMIZE_ACTION_COSTS``, then each action is associated to a cost expression.
    ///
    /// TODO: Document what is allowed in the expression. See issue #134
    /// In particular, for this metric to be useful in many practical problems, the cost expression
    /// should allow referring to the action parameters (and possibly the current state at the action start/end).
    /// This is very awkward to do in this setting where the expression is detached from its scope.
    #[prost(map="string, message", tag="3")]
    pub action_costs: ::std::collections::HashMap<::prost::alloc::string::String, Expression>,
    #[prost(message, optional, tag="4")]
    pub default_action_cost: ::core::option::Option<Expression>,
}
/// Nested message and enum types in `Metric`.
pub mod metric {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum MetricKind {
        /// Minimize the action costs expressed in the `action_costs` field
        MinimizeActionCosts = 0,
        /// Minimize the length of the resulting sequential plan
        MinimizeSequentialPlanLength = 1,
        /// Minimize the makespan in case of temporal planning
        /// features: durative_actions
        MinimizeMakespan = 2,
        /// Minimize the value of the expression defined in the `expression`` field
        MinimizeExpressionOnFinalState = 3,
        /// Maximize the value of the expression defined in the `expression`` field
        MaximizeExpressionOnFinalState = 4,
    }
}
/// features: ACTION_BASED
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Problem {
    #[prost(string, tag="1")]
    pub domain_name: ::prost::alloc::string::String,
    #[prost(string, tag="2")]
    pub problem_name: ::prost::alloc::string::String,
    #[prost(message, repeated, tag="3")]
    pub types: ::prost::alloc::vec::Vec<TypeDeclaration>,
    #[prost(message, repeated, tag="4")]
    pub fluents: ::prost::alloc::vec::Vec<Fluent>,
    #[prost(message, repeated, tag="5")]
    pub objects: ::prost::alloc::vec::Vec<ObjectDeclaration>,
    /// List of actions in the domain.
    #[prost(message, repeated, tag="6")]
    pub actions: ::prost::alloc::vec::Vec<Action>,
    /// Initial state, including default values of state variables.
    #[prost(message, repeated, tag="7")]
    pub initial_state: ::prost::alloc::vec::Vec<Assignment>,
    /// Facts and effects that are expected to occur strictly later than the initial state.
    /// features: TIMED_EFFECT
    #[prost(message, repeated, tag="8")]
    pub timed_effects: ::prost::alloc::vec::Vec<TimedEffect>,
    /// Goals of the planning problem.
    #[prost(message, repeated, tag="9")]
    pub goals: ::prost::alloc::vec::Vec<Goal>,
    /// all features of the problem
    #[prost(enumeration="Feature", repeated, tag="10")]
    pub features: ::prost::alloc::vec::Vec<i32>,
    /// The plan quality metrics
    #[prost(message, repeated, tag="11")]
    pub metrics: ::prost::alloc::vec::Vec<Metric>,
    /// If the problem is hierarchical, defines the tasks and methods as well as the initial task network.
    /// features: hierarchical
    #[prost(message, optional, tag="12")]
    pub hierarchy: ::core::option::Option<Hierarchy>,
}
// =================== Plan ================

/// Representation of an action instance that appears in a plan.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ActionInstance {
    /// Optional. A unique identifier of the action that might be used to refer to it (e.g. in HTN plans).
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    /// name of the action
    #[prost(string, tag="2")]
    pub action_name: ::prost::alloc::string::String,
    /// Parameters of the action instance, required to be constants.
    #[prost(message, repeated, tag="3")]
    pub parameters: ::prost::alloc::vec::Vec<Atom>,
    /// Start time of the action. The default 0 value is OK in the case of non-temporal planning
    /// feature: \[DURATIVE_ACTIONS\]
    #[prost(message, optional, tag="4")]
    pub start_time: ::core::option::Option<Real>,
    /// End time of the action. The default 0 value is OK in the case of non-temporal planning
    /// feature: \[DURATIVE_ACTIONS\]
    #[prost(message, optional, tag="5")]
    pub end_time: ::core::option::Option<Real>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Plan {
    /// An ordered sequence of actions that appear in the plan.
    /// The order of the actions in the list must be compatible with the partial order of the start times.
    /// In case of non-temporal planning, this allows having all start time at 0 and only rely on the order in this sequence.
    #[prost(message, repeated, tag="1")]
    pub actions: ::prost::alloc::vec::Vec<ActionInstance>,
}
// =============== RPC API =======================

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PlanRequest {
    /// Problem that should be solved.
    #[prost(message, optional, tag="1")]
    pub problem: ::core::option::Option<Problem>,
    #[prost(enumeration="plan_request::Mode", tag="2")]
    pub resolution_mode: i32,
    /// Max allowed runtime time in seconds.
    #[prost(double, tag="3")]
    pub timeout: f64,
    /// Engine specific options to be passed to the engine
    #[prost(map="string, string", tag="4")]
    pub engine_options: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
}
/// Nested message and enum types in `PlanRequest`.
pub mod plan_request {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Mode {
        Satisfiable = 0,
        SolvedOptimally = 1,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ValidationRequest {
    /// Problem to be validated.
    #[prost(message, optional, tag="1")]
    pub problem: ::core::option::Option<Problem>,
    /// Plan to validate.
    #[prost(message, optional, tag="2")]
    pub plan: ::core::option::Option<Plan>,
}
/// A freely formatted logging message.
/// Each message is annotated with its criticality level from the minimal (DEBUG) to the maximal (ERROR).
/// Criticality level is expected to be used by an end user to decide the level of verbosity.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LogMessage {
    #[prost(enumeration="log_message::LogLevel", tag="1")]
    pub level: i32,
    #[prost(string, tag="2")]
    pub message: ::prost::alloc::string::String,
}
/// Nested message and enum types in `LogMessage`.
pub mod log_message {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum LogLevel {
        Debug = 0,
        Info = 1,
        Warning = 2,
        Error = 3,
    }
}
/// Message sent by engine.
/// Contains the engine exit status as well as the best plan found if any.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PlanGenerationResult {
    #[prost(enumeration="plan_generation_result::Status", tag="1")]
    pub status: i32,
    /// Optional. Best plan found if any.
    #[prost(message, optional, tag="2")]
    pub plan: ::core::option::Option<Plan>,
    /// A set of engine specific values that can be reported, for instance
    /// - "grounding-time": "10ms"
    /// - "expanded-states": "1290"
    #[prost(map="string, string", tag="3")]
    pub metrics: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    /// Optional log messages about the engine's activity.
    /// Note that it should not be expected that logging messages are visible to the end user.
    /// If used in conjunction with INTERNAL_ERROR or UNSUPPORTED_PROBLEM, it would be expected to have at least one log message at the ERROR level.
    #[prost(message, repeated, tag="4")]
    pub log_messages: ::prost::alloc::vec::Vec<LogMessage>,
    /// Synthetic description of the engine that generated this message.
    #[prost(message, optional, tag="5")]
    pub engine: ::core::option::Option<Engine>,
}
/// Nested message and enum types in `PlanGenerationResult`.
pub mod plan_generation_result {
    /// ==== Engine stopped normally ======
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Status {
        /// Valid plan found
        /// The `plan` field must be set.
        SolvedSatisficing = 0,
        /// Plan found with optimality guarantee
        /// The `plan` field must be set and contains an optimal solution.
        SolvedOptimally = 1,
        /// No plan exists
        UnsolvableProven = 2,
        /// The engine was not able to find a solution but does not give any guarantee that none exist
        /// (i.e. the engine might not be complete)
        UnsolvableIncompletely = 3,
        // ====== Engine exited before making any conclusion ====
        // Search stopped before concluding SOLVED_OPTIMALLY or UNSOLVABLE_PROVEN
        // If a plan was found, it might be reported in the `plan` field

        /// The engine ran out of time
        Timeout = 13,
        /// The engine ran out of memory
        Memout = 14,
        /// The engine faced an internal error.
        InternalError = 15,
        /// The problem submitted is not supported by the engine.
        UnsupportedProblem = 16,
        /// ====== Intermediate answer ======
        /// This Answer is an Intermediate Answer and not a Final one
        Intermediate = 17,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Engine {
    /// Short name of the engine (planner, validator, ...)
    #[prost(string, tag="1")]
    pub name: ::prost::alloc::string::String,
}
/// Message sent by the validator.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ValidationResult {
    #[prost(enumeration="validation_result::ValidationResultStatus", tag="1")]
    pub status: i32,
    /// Optional. Information given by the engine to the user.
    #[prost(message, repeated, tag="2")]
    pub log_messages: ::prost::alloc::vec::Vec<LogMessage>,
    /// Synthetic description of the engine that generated this message.
    #[prost(message, optional, tag="3")]
    pub engine: ::core::option::Option<Engine>,
}
/// Nested message and enum types in `ValidationResult`.
pub mod validation_result {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum ValidationResultStatus {
        /// The Plan is valid for the Problem.
        Valid = 0,
        /// The Plan is not valid for the Problem.
        Invalid = 1,
    }
}
/// Message sent by the grounder.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompilerResult {
    /// The problem generated by the Compiler
    #[prost(message, optional, tag="1")]
    pub problem: ::core::option::Option<Problem>,
    /// The map_back_plan field is a map from the ActionInstance of the
    /// compiled problem to the original ActionInstance.
    #[prost(map="string, message", tag="2")]
    pub map_back_plan: ::std::collections::HashMap<::prost::alloc::string::String, ActionInstance>,
    /// Optional. Information given by the engine to the user.
    #[prost(message, repeated, tag="3")]
    pub log_messages: ::prost::alloc::vec::Vec<LogMessage>,
    /// Synthetic description of the engine that generated this message.
    #[prost(message, optional, tag="4")]
    pub engine: ::core::option::Option<Engine>,
}
/// The kind of an expression, which gives information related to its structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ExpressionKind {
    /// Default value, should not be used. Drop it if we are sure to never need it.
    Unknown = 0,
    /// Constant atom. For instance `3` or `kitchen` (where `kitchen` is an object defined in the problem)
    Constant = 1,
    /// Atom symbol representing a parameter from an outer scope. For instance `from` that would appear inside a `(move from to - location)` action.
    Parameter = 2,
    /// Atom symbol representing a variable from an outer scope.
    /// This is typically used to represent the variables that are existentially or universally qualified in expressions.
    Variable = 7,
    /// Atom symbol representing a fluent of the problem. For instance `at-robot`.
    FluentSymbol = 3,
    /// Atom representing a function. For instance `+`, `=`, `and`, ...
    FunctionSymbol = 4,
    /// List. Application of some parameters to a fluent symbol. For instance `(at-robot l1)` or `(battery-charged)`
    /// The first element of the list must be a FLUENT_SYMBOL
    StateVariable = 5,
    /// List. The expression is the application of some parameters to a function. For instance `(+ 1 3)`.
    /// The first element of the list must be a FUNCTION_SYMBOL
    FunctionApplication = 6,
    /// Atom symbol. Unique identifier of a task or action in the current scope.
    ContainerId = 8,
}
/// Features of the problem.
/// Features are essential in that not supporting a feature `X` should allow disregarding any field tagged with `features: \[X\]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Feature {
    /// PROBLEM_CLASS
    ActionBased = 0,
    Hierarchical = 26,
    /// TIME
    ContinuousTime = 1,
    DiscreteTime = 2,
    IntermediateConditionsAndEffects = 3,
    TimedEffect = 4,
    TimedGoals = 5,
    DurationInequalities = 6,
    /// EXPRESSION_DURATION
    StaticFluentsInDuration = 27,
    FluentsInDuration = 28,
    /// NUMBERS
    ContinuousNumbers = 7,
    DiscreteNumbers = 8,
    /// CONDITIONS_KIND
    NegativeConditions = 9,
    DisjunctiveConditions = 10,
    Equality = 11,
    ExistentialConditions = 12,
    UniversalConditions = 13,
    /// EFFECTS_KIND
    ConditionalEffects = 14,
    IncreaseEffects = 15,
    DecreaseEffects = 16,
    /// TYPING
    FlatTyping = 17,
    HierarchicalTyping = 18,
    /// FLUENTS_TYPE
    NumericFluents = 19,
    ObjectFluents = 20,
    /// QUALITY_METRICS
    ActionsCost = 21,
    FinalValue = 22,
    Makespan = 23,
    PlanLength = 24,
    /// SIMULATED_ENTITIES
    SimulatedEffects = 25,
}
/// Generated client implementations.
pub mod unified_planning_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    #[derive(Debug, Clone)]
    pub struct UnifiedPlanningClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl UnifiedPlanningClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> UnifiedPlanningClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> UnifiedPlanningClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            UnifiedPlanningClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with `gzip`.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_gzip(mut self) -> Self {
            self.inner = self.inner.send_gzip();
            self
        }
        /// Enable decompressing responses with `gzip`.
        #[must_use]
        pub fn accept_gzip(mut self) -> Self {
            self.inner = self.inner.accept_gzip();
            self
        }
        /// A plan request to the engine.
        /// The engine replies with a stream of N `Answer` messages where:
        ///  - the first (N-1) message are of type `IntermediateReport`
        ///  - the last message is of type `FinalReport`
        pub async fn plan_one_shot(
            &mut self,
            request: impl tonic::IntoRequest<super::PlanRequest>,
        ) -> Result<
                tonic::Response<tonic::codec::Streaming<super::PlanGenerationResult>>,
                tonic::Status,
            > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/UnifiedPlanning/planOneShot",
            );
            self.inner.server_streaming(request.into_request(), path, codec).await
        }
        /// A validation request to the engine.
        /// The engine replies with the ValidationResult
        pub async fn validate_plan(
            &mut self,
            request: impl tonic::IntoRequest<super::ValidationRequest>,
        ) -> Result<tonic::Response<super::ValidationResult>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/UnifiedPlanning/validatePlan",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// A compiler request to the engine.
        /// The engine replies with the CompilerResult
        pub async fn compile(
            &mut self,
            request: impl tonic::IntoRequest<super::Problem>,
        ) -> Result<tonic::Response<super::CompilerResult>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/UnifiedPlanning/compile");
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod unified_planning_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    ///Generated trait containing gRPC methods that should be implemented for use with UnifiedPlanningServer.
    #[async_trait]
    pub trait UnifiedPlanning: Send + Sync + 'static {
        ///Server streaming response type for the planOneShot method.
        type planOneShotStream: futures_core::Stream<
                Item = Result<super::PlanGenerationResult, tonic::Status>,
            >
            + Send
            + 'static;
        /// A plan request to the engine.
        /// The engine replies with a stream of N `Answer` messages where:
        ///  - the first (N-1) message are of type `IntermediateReport`
        ///  - the last message is of type `FinalReport`
        async fn plan_one_shot(
            &self,
            request: tonic::Request<super::PlanRequest>,
        ) -> Result<tonic::Response<Self::planOneShotStream>, tonic::Status>;
        /// A validation request to the engine.
        /// The engine replies with the ValidationResult
        async fn validate_plan(
            &self,
            request: tonic::Request<super::ValidationRequest>,
        ) -> Result<tonic::Response<super::ValidationResult>, tonic::Status>;
        /// A compiler request to the engine.
        /// The engine replies with the CompilerResult
        async fn compile(
            &self,
            request: tonic::Request<super::Problem>,
        ) -> Result<tonic::Response<super::CompilerResult>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct UnifiedPlanningServer<T: UnifiedPlanning> {
        inner: _Inner<T>,
        accept_compression_encodings: (),
        send_compression_encodings: (),
    }
    struct _Inner<T>(Arc<T>);
    impl<T: UnifiedPlanning> UnifiedPlanningServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for UnifiedPlanningServer<T>
    where
        T: UnifiedPlanning,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/UnifiedPlanning/planOneShot" => {
                    #[allow(non_camel_case_types)]
                    struct planOneShotSvc<T: UnifiedPlanning>(pub Arc<T>);
                    impl<
                        T: UnifiedPlanning,
                    > tonic::server::ServerStreamingService<super::PlanRequest>
                    for planOneShotSvc<T> {
                        type Response = super::PlanGenerationResult;
                        type ResponseStream = T::planOneShotStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::PlanRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).plan_one_shot(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = planOneShotSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.server_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/UnifiedPlanning/validatePlan" => {
                    #[allow(non_camel_case_types)]
                    struct validatePlanSvc<T: UnifiedPlanning>(pub Arc<T>);
                    impl<
                        T: UnifiedPlanning,
                    > tonic::server::UnaryService<super::ValidationRequest>
                    for validatePlanSvc<T> {
                        type Response = super::ValidationResult;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ValidationRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move {
                                (*inner).validate_plan(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = validatePlanSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/UnifiedPlanning/compile" => {
                    #[allow(non_camel_case_types)]
                    struct compileSvc<T: UnifiedPlanning>(pub Arc<T>);
                    impl<T: UnifiedPlanning> tonic::server::UnaryService<super::Problem>
                    for compileSvc<T> {
                        type Response = super::CompilerResult;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::Problem>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).compile(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = compileSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: UnifiedPlanning> Clone for UnifiedPlanningServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
            }
        }
    }
    impl<T: UnifiedPlanning> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: UnifiedPlanning> tonic::transport::NamedService
    for UnifiedPlanningServer<T> {
        const NAME: &'static str = "UnifiedPlanning";
    }
}
