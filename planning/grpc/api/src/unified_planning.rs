/// As in s-expression, an Expression is either an atom or list representing the application of some parameters to a function/fluent.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Expression {
    /// If non-empty, the expression is a single atom.
    /// For instance `3`, `+`, `kitchen`, `at-robot`, ...
    #[prost(message, optional, tag = "1")]
    pub atom: ::core::option::Option<Atom>,
    /// If the `atom` field is empty, then the expression is a list of sub-expressions,
    /// typically representing the application of some arguments to a function or fluent.
    /// For instance `(+ 1 3)`, (at-robot l1)`, `(>= (battery_level) 20)`
    #[prost(message, repeated, tag = "2")]
    pub list: ::prost::alloc::vec::Vec<Expression>,
    /// Type of the expression. For instance "int", "location", ...
    #[prost(string, tag = "3")]
    pub r#type: ::prost::alloc::string::String,
    /// Kind of the expression, specifying the content of the expression.
    /// This is intended to facilitate parsing of the expression.
    #[prost(enumeration = "ExpressionKind", tag = "4")]
    pub kind: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Atom {
    #[prost(oneof = "atom::Content", tags = "1, 2, 3, 4")]
    pub content: ::core::option::Option<atom::Content>,
}
/// Nested message and enum types in `Atom`.
pub mod atom {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Content {
        #[prost(string, tag = "1")]
        Symbol(::prost::alloc::string::String),
        #[prost(int64, tag = "2")]
        Int(i64),
        #[prost(message, tag = "3")]
        Real(super::Real),
        #[prost(bool, tag = "4")]
        Boolean(bool),
    }
}
/// Representation of a constant real number, as the fraction `(numerator / denominator)`.
/// A real should be in its canonical form (with smallest possible denominator).
/// Notably, if this number is an integer, then it is guaranteed that `denominator == 1`.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Real {
    #[prost(int64, tag = "1")]
    pub numerator: i64,
    #[prost(int64, tag = "2")]
    pub denominator: i64,
}
/// Declares the existence of a symbolic type.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TypeDeclaration {
    /// Name of the type that is declared.
    #[prost(string, tag = "1")]
    pub type_name: ::prost::alloc::string::String,
    /// Optional. If the string is non-empty, this is the parent type of `type_name`.
    /// If set, the parent type must have been previously declared (i.e. should appear earlier in the problem's type declarations.
    /// feature: HIERARCHICAL_TYPING
    #[prost(string, tag = "2")]
    pub parent_type: ::prost::alloc::string::String,
}
/// Parameter of a fluent or of an action
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Parameter {
    /// Name of the parameter.
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// Type of the parameter.
    #[prost(string, tag = "2")]
    pub r#type: ::prost::alloc::string::String,
}
/// A state-dependent variable.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Fluent {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// Return type of the fluent.
    #[prost(string, tag = "2")]
    pub value_type: ::prost::alloc::string::String,
    /// Typed and named parameters of the fluent.
    #[prost(message, repeated, tag = "3")]
    pub parameters: ::prost::alloc::vec::Vec<Parameter>,
    /// If non-empty, then any state variable using this fluent that is not explicitly given a value in the initial state
    /// will be assumed to have this default value.
    /// This allows mimicking the closed world assumption by setting a "false" default value to predicates.
    /// Note that in the initial state of the problem message, it is assumed that all default values are set.
    #[prost(message, optional, tag = "4")]
    pub default_value: ::core::option::Option<Expression>,
}
/// Declares an object with the given name and type.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ObjectDeclaration {
    /// Name of the object.
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// Type of the object.
    /// The type must have been previously declared in the problem definition.
    #[prost(string, tag = "2")]
    pub r#type: ::prost::alloc::string::String,
}
/// An effect expression is of the form `FLUENT OP VALUE`.
/// We explicitly restrict the different types of effects by setting the allowed operators.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EffectExpression {
    #[prost(enumeration = "effect_expression::EffectKind", tag = "1")]
    pub kind: i32,
    /// Expression that must be of the STATE_VARIABLE kind.
    #[prost(message, optional, tag = "2")]
    pub fluent: ::core::option::Option<Expression>,
    #[prost(message, optional, tag = "3")]
    pub value: ::core::option::Option<Expression>,
    /// Optional. If the effect is conditional, then the following field must be set.
    /// In this case, the `effect` will only be applied if the `condition`` holds.
    /// If the effect is unconditional, the effect is set to the constant 'true' value.
    /// features: CONDITIONAL_EFFECT
    #[prost(message, optional, tag = "4")]
    pub condition: ::core::option::Option<Expression>,
    /// The variables that quantify this effect
    #[prost(message, repeated, tag = "5")]
    pub forall: ::prost::alloc::vec::Vec<Expression>,
}
/// Nested message and enum types in `EffectExpression`.
pub mod effect_expression {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
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
    impl EffectKind {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                EffectKind::Assign => "ASSIGN",
                EffectKind::Increase => "INCREASE",
                EffectKind::Decrease => "DECREASE",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "ASSIGN" => Some(Self::Assign),
                "INCREASE" => Some(Self::Increase),
                "DECREASE" => Some(Self::Decrease),
                _ => None,
            }
        }
    }
}
/// Representation of an effect that allows qualifying the effect expression, e.g., to make it a conditional effect.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Effect {
    /// Required. The actual effect that should take place.
    #[prost(message, optional, tag = "1")]
    pub effect: ::core::option::Option<EffectExpression>,
    /// Optional. If the effect is within a durative action, the following must be set and will specify when the effect takes place.
    /// features: DURATIVE_ACTIONS
    #[prost(message, optional, tag = "2")]
    pub occurrence_time: ::core::option::Option<Timing>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Condition {
    #[prost(message, optional, tag = "1")]
    pub cond: ::core::option::Option<Expression>,
    /// Optional. Must be set for durative actions where it specifies the temporal interval
    /// over which when the condition should hold.
    /// features: DURATIVE_ACTIONS
    #[prost(message, optional, tag = "2")]
    pub span: ::core::option::Option<TimeInterval>,
}
/// Unified action representation that represents any kind of actions.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Action {
    /// Action name. E.g. "move"
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// Typed and named parameters of the action.
    #[prost(message, repeated, tag = "2")]
    pub parameters: ::prost::alloc::vec::Vec<Parameter>,
    /// If set, the action is durative. Otherwise it is instantaneous.
    /// features: DURATIVE_ACTIONS
    #[prost(message, optional, tag = "3")]
    pub duration: ::core::option::Option<Duration>,
    /// Conjunction of conditions that must hold for the action to be applicable.
    #[prost(message, repeated, tag = "4")]
    pub conditions: ::prost::alloc::vec::Vec<Condition>,
    /// Conjunction of effects as a result of applying this action.
    #[prost(message, repeated, tag = "5")]
    pub effects: ::prost::alloc::vec::Vec<Effect>,
}
/// Symbolic reference to an absolute time.
/// It might represent:
/// - the time of the initial/final state, or
/// - the start/end of the containing action, or
/// - the start/end of one of the subtask in the context of a method or of a task network.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Timepoint {
    #[prost(enumeration = "timepoint::TimepointKind", tag = "1")]
    pub kind: i32,
    /// If non-empty, identifies the container of which we are extracting the start/end timepoint.
    /// In the context of a task-network or of a method, this could be the `id` of one of the subtasks.
    /// feature: hierarchies
    #[prost(string, tag = "2")]
    pub container_id: ::prost::alloc::string::String,
}
/// Nested message and enum types in `Timepoint`.
pub mod timepoint {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
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
    impl TimepointKind {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                TimepointKind::GlobalStart => "GLOBAL_START",
                TimepointKind::GlobalEnd => "GLOBAL_END",
                TimepointKind::Start => "START",
                TimepointKind::End => "END",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "GLOBAL_START" => Some(Self::GlobalStart),
                "GLOBAL_END" => Some(Self::GlobalEnd),
                "START" => Some(Self::Start),
                "END" => Some(Self::End),
                _ => None,
            }
        }
    }
}
/// Represents a time (`timepoint` + `delay`), that is a time defined relatively to a particular `timepoint`.
/// Note that an absolute time can be defined by setting the `delay` relative to the `GLOBAL_START`` which is the reference time.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Timing {
    #[prost(message, optional, tag = "1")]
    pub timepoint: ::core::option::Option<Timepoint>,
    #[prost(message, optional, tag = "2")]
    pub delay: ::core::option::Option<Real>,
}
/// An interval `[lower, upper]` where `lower` and `upper` are arbitrary expressions.
/// The `is_left_open` and `is_right_open` fields indicate whether the interval is
/// opened on left and right side respectively.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Interval {
    #[prost(bool, tag = "1")]
    pub is_left_open: bool,
    #[prost(message, optional, tag = "2")]
    pub lower: ::core::option::Option<Expression>,
    #[prost(bool, tag = "3")]
    pub is_right_open: bool,
    #[prost(message, optional, tag = "4")]
    pub upper: ::core::option::Option<Expression>,
}
/// A contiguous slice of time represented as an interval `[lower, upper]` where `lower` and `upper` are time references.
/// The `is_left_open` and `is_right_open` fields indicate whether the interval is
/// opened on left and right side respectively.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TimeInterval {
    #[prost(bool, tag = "1")]
    pub is_left_open: bool,
    #[prost(message, optional, tag = "2")]
    pub lower: ::core::option::Option<Timing>,
    #[prost(bool, tag = "3")]
    pub is_right_open: bool,
    #[prost(message, optional, tag = "4")]
    pub upper: ::core::option::Option<Timing>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Duration {
    /// / The duration of the action can be freely chosen within the indicated bounds
    #[prost(message, optional, tag = "1")]
    pub controllable_in_bounds: ::core::option::Option<Interval>,
}
/// Declares an abstract task together with its expected parameters.
///
/// Example: goto(robot: Robot, destination: Location)
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AbstractTaskDeclaration {
    /// Example: "goto"
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// Example:
    ///   - robot: Robot
    ///   - destination: Location
    #[prost(message, repeated, tag = "2")]
    pub parameters: ::prost::alloc::vec::Vec<Parameter>,
}
/// Representation of an abstract or primitive task that should be achieved,
/// required either in the initial task network or as a subtask of a method.
///
/// Example:  task of sending a `robot` to the KITCHEN
///    - t1: goto(robot, KITCHEN)
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Task {
    /// Identifier of the task, required to be unique in the method/task-network where the task appears.
    /// The `id` is notably used to refer to the start/end of the task.
    ///
    /// Example: t1
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
    /// Name of the task that should be achieved. It might either
    ///   - an abstract task if the name is the one of a task declared in the problem
    ///   - a primitive task if the name is the one of an action declared in the problem
    ///
    /// Example:
    ///   - "goto" (abstract task)
    ///   - "move" (action / primitive task)
    #[prost(string, tag = "2")]
    pub task_name: ::prost::alloc::string::String,
    /// Example: (for a "goto" task)
    ///   - robot    (a parameter from an outer scope)
    ///   - KITCHEN  (a constant symbol in the problem)
    #[prost(message, repeated, tag = "3")]
    pub parameters: ::prost::alloc::vec::Vec<Expression>,
}
/// A method describes one possible way of achieving a task.
///
/// Example: A method that make a "move" action and recursively calls itself until reaching the destination.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Method {
    /// A name that uniquely identify the method.
    /// This is mostly used for user facing output or plan validation.
    ///
    /// Example: "m-recursive-goto"
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// Example: [robot: Robot, source: Location, intermediate: Location, destination: Location]
    #[prost(message, repeated, tag = "2")]
    pub parameters: ::prost::alloc::vec::Vec<Parameter>,
    /// The task that is achieved by the method.
    /// A subset of the parameters of the method will typically be used to
    /// define the task that is achieved.
    ///
    /// Example: goto(robot, destination)
    #[prost(message, optional, tag = "3")]
    pub achieved_task: ::core::option::Option<Task>,
    /// A set of subtasks that should be achieved to carry out the method.
    /// Note that the order of subtasks is irrelevant and that any ordering constraint should be
    /// specified in the `constraints` field.
    ///
    /// Example:
    ///   - t1: (move robot source intermediate)
    ///   - t2: goto(robot destination)
    #[prost(message, repeated, tag = "4")]
    pub subtasks: ::prost::alloc::vec::Vec<Task>,
    /// Constraints enable the definition of ordering constraints as well as constraints
    /// on the allowed instantiation of the method's parameters.
    ///
    /// Example:
    ///   - end(t1) < start(t2)
    ///   - source != intermediate
    #[prost(message, repeated, tag = "5")]
    pub constraints: ::prost::alloc::vec::Vec<Expression>,
    /// Conjunction of conditions that must hold for the method to be applicable.
    /// As for the conditions of actions, these can be temporally qualified to refer to intermediate timepoints.
    /// In addition to the start/end of the method, the temporal qualification might refer to the start/end of
    /// one of the subtasks using its identifier.
    ///
    /// Example:
    ///   - \[start\] loc(robot) == source
    ///   - \[end(t1)\] loc(robot) == intermediate
    ///   - \[end\] loc(robot) == destination
    #[prost(message, repeated, tag = "6")]
    pub conditions: ::prost::alloc::vec::Vec<Condition>,
}
/// A task network defines a set of subtasks and associated constraints.
/// It is intended to be used to define the initial task network of the hierarchical problem.
///
/// Example: an arbitrary robot should go to the KITCHEN before time 100
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TaskNetwork {
    /// robot: Location
    #[prost(message, repeated, tag = "1")]
    pub variables: ::prost::alloc::vec::Vec<Parameter>,
    /// t1: goto(robot, KITCHEN)
    #[prost(message, repeated, tag = "2")]
    pub subtasks: ::prost::alloc::vec::Vec<Task>,
    /// end(t1) <= 100
    #[prost(message, repeated, tag = "3")]
    pub constraints: ::prost::alloc::vec::Vec<Expression>,
}
/// Represents the hierarchical part of a problem.
/// features: hierarchical
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Hierarchy {
    #[prost(message, repeated, tag = "1")]
    pub abstract_tasks: ::prost::alloc::vec::Vec<AbstractTaskDeclaration>,
    #[prost(message, repeated, tag = "2")]
    pub methods: ::prost::alloc::vec::Vec<Method>,
    #[prost(message, optional, tag = "3")]
    pub initial_task_network: ::core::option::Option<TaskNetwork>,
}
/// Activity in a scheduling problem.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Activity {
    /// Name of the activity that must uniquely identify it.
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// Typed and named parameters of the activity.
    #[prost(message, repeated, tag = "2")]
    pub parameters: ::prost::alloc::vec::Vec<Parameter>,
    /// Duration of the activity
    #[prost(message, optional, tag = "3")]
    pub duration: ::core::option::Option<Duration>,
    /// Conjunction of conditions that must hold if the activity is present.
    #[prost(message, repeated, tag = "4")]
    pub conditions: ::prost::alloc::vec::Vec<Condition>,
    /// Conjunction of effects that this activity produces.
    #[prost(message, repeated, tag = "5")]
    pub effects: ::prost::alloc::vec::Vec<Effect>,
    /// Conjunction of static constraints that must hold if the activity is present.
    #[prost(message, repeated, tag = "6")]
    pub constraints: ::prost::alloc::vec::Vec<Expression>,
}
/// Extension of `Problem` for scheduling
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SchedulingExtension {
    /// All potential activities of the scheduling problem.
    #[prost(message, repeated, tag = "1")]
    pub activities: ::prost::alloc::vec::Vec<Activity>,
    /// All variables in the base problem
    #[prost(message, repeated, tag = "2")]
    pub variables: ::prost::alloc::vec::Vec<Parameter>,
    /// All constraints in the base problem.
    #[prost(message, repeated, tag = "5")]
    pub constraints: ::prost::alloc::vec::Vec<Expression>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Schedule {
    /// Name of the activities that appear in the solution
    #[prost(string, repeated, tag = "1")]
    pub activities: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    /// Assignment of all variables and activity parameters and timepoints
    /// that appear in the solution.
    #[prost(map = "string, message", tag = "2")]
    pub variable_assignments: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        Atom,
    >,
}
/// A Goal is currently an expression that must hold either:
/// - in the final state,
/// - over a specific temporal interval (under the `timed_goals` features)
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Goal {
    /// Goal expression that must hold in the final state.
    #[prost(message, optional, tag = "1")]
    pub goal: ::core::option::Option<Expression>,
    /// Optional. If specified the goal should hold over the specified temporal interval (instead of on the final state).
    /// features: TIMED_GOALS
    #[prost(message, optional, tag = "2")]
    pub timing: ::core::option::Option<TimeInterval>,
}
/// Represents an effect that will occur sometime beyond the initial state. (similar to timed initial literals)
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TimedEffect {
    /// Required. An effect expression that will take place sometime in the future (i.e. not at the intial state) as specified by the temporal qualifiation.
    #[prost(message, optional, tag = "1")]
    pub effect: ::core::option::Option<EffectExpression>,
    /// Required. Temporal qualification denoting when the timed fact will occur.
    #[prost(message, optional, tag = "2")]
    pub occurrence_time: ::core::option::Option<Timing>,
}
/// An assignment of a value to a fluent, as it appears in the initial state definition.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Assignment {
    /// State variable that is assigned the `value`.
    /// It should be an expression of the STATE_VARIABLE kind for which all parameters are of the CONSTANT kind.
    #[prost(message, optional, tag = "1")]
    pub fluent: ::core::option::Option<Expression>,
    /// An expression of the CONSTANT kind, denoting the value take by the state variable.
    #[prost(message, optional, tag = "2")]
    pub value: ::core::option::Option<Expression>,
}
/// Represents a goal associated with a weight, used to define oversubscription planning.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GoalWithWeight {
    /// Goal expression
    #[prost(message, optional, tag = "1")]
    pub goal: ::core::option::Option<Expression>,
    /// The weight
    #[prost(message, optional, tag = "2")]
    pub weight: ::core::option::Option<Real>,
}
/// Represents a timed goal associated with a weight, used to define temporal oversubscription planning.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TimedGoalWithWeight {
    /// Goal expression
    #[prost(message, optional, tag = "1")]
    pub goal: ::core::option::Option<Expression>,
    /// The time interval
    #[prost(message, optional, tag = "2")]
    pub timing: ::core::option::Option<TimeInterval>,
    /// The weight
    #[prost(message, optional, tag = "3")]
    pub weight: ::core::option::Option<Real>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Metric {
    #[prost(enumeration = "metric::MetricKind", tag = "1")]
    pub kind: i32,
    /// Expression to minimize/maximize in the final state.
    /// Empty, if the `kind` is not {MIN/MAX}IMIZE_EXPRESSION_ON_FINAL_STATE
    #[prost(message, optional, tag = "2")]
    pub expression: ::core::option::Option<Expression>,
    /// If `kind == MINIMIZE_ACTION_COSTS``, then each action is associated to a cost expression.
    ///
    /// TODO: Document what is allowed in the expression. See issue #134
    /// In particular, for this metric to be useful in many practical problems, the cost expression
    /// should allow referring to the action parameters (and possibly the current state at the action start/end).
    /// This is very awkward to do in this setting where the expression is detached from its scope.
    #[prost(map = "string, message", tag = "3")]
    pub action_costs: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        Expression,
    >,
    #[prost(message, optional, tag = "4")]
    pub default_action_cost: ::core::option::Option<Expression>,
    /// List of goals used to define the oversubscription planning problem.
    /// Empty, if the `kind` is not OVERSUBSCRIPTION
    #[prost(message, repeated, tag = "5")]
    pub goals: ::prost::alloc::vec::Vec<GoalWithWeight>,
    /// List of timed goals used to define the temporal oversubscription planning problem.
    /// Empty, if the `kind` is not TEMPORAL_OVERSUBSCRIPTION
    #[prost(message, repeated, tag = "6")]
    pub timed_goals: ::prost::alloc::vec::Vec<TimedGoalWithWeight>,
}
/// Nested message and enum types in `Metric`.
pub mod metric {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum MetricKind {
        /// Minimize the action costs expressed in the `action_costs` field
        MinimizeActionCosts = 0,
        /// Minimize the length of the resulting sequential plan
        MinimizeSequentialPlanLength = 1,
        /// Minimize the makespan in case of temporal planning
        /// features: durative_actions
        MinimizeMakespan = 2,
        /// Minimize the value of the expression defined in the `expression` field
        MinimizeExpressionOnFinalState = 3,
        /// Maximize the value of the expression defined in the `expression` field
        MaximizeExpressionOnFinalState = 4,
        /// Maximize the weighted number of goals reached
        Oversubscription = 5,
        /// Maximize the weighted number of timed goals reached
        TemporalOversubscription = 6,
    }
    impl MetricKind {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                MetricKind::MinimizeActionCosts => "MINIMIZE_ACTION_COSTS",
                MetricKind::MinimizeSequentialPlanLength => {
                    "MINIMIZE_SEQUENTIAL_PLAN_LENGTH"
                }
                MetricKind::MinimizeMakespan => "MINIMIZE_MAKESPAN",
                MetricKind::MinimizeExpressionOnFinalState => {
                    "MINIMIZE_EXPRESSION_ON_FINAL_STATE"
                }
                MetricKind::MaximizeExpressionOnFinalState => {
                    "MAXIMIZE_EXPRESSION_ON_FINAL_STATE"
                }
                MetricKind::Oversubscription => "OVERSUBSCRIPTION",
                MetricKind::TemporalOversubscription => "TEMPORAL_OVERSUBSCRIPTION",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "MINIMIZE_ACTION_COSTS" => Some(Self::MinimizeActionCosts),
                "MINIMIZE_SEQUENTIAL_PLAN_LENGTH" => {
                    Some(Self::MinimizeSequentialPlanLength)
                }
                "MINIMIZE_MAKESPAN" => Some(Self::MinimizeMakespan),
                "MINIMIZE_EXPRESSION_ON_FINAL_STATE" => {
                    Some(Self::MinimizeExpressionOnFinalState)
                }
                "MAXIMIZE_EXPRESSION_ON_FINAL_STATE" => {
                    Some(Self::MaximizeExpressionOnFinalState)
                }
                "OVERSUBSCRIPTION" => Some(Self::Oversubscription),
                "TEMPORAL_OVERSUBSCRIPTION" => Some(Self::TemporalOversubscription),
                _ => None,
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Problem {
    #[prost(string, tag = "1")]
    pub domain_name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub problem_name: ::prost::alloc::string::String,
    #[prost(message, repeated, tag = "3")]
    pub types: ::prost::alloc::vec::Vec<TypeDeclaration>,
    #[prost(message, repeated, tag = "4")]
    pub fluents: ::prost::alloc::vec::Vec<Fluent>,
    #[prost(message, repeated, tag = "5")]
    pub objects: ::prost::alloc::vec::Vec<ObjectDeclaration>,
    /// List of actions in the domain.
    /// features: ACTION_BASED
    #[prost(message, repeated, tag = "6")]
    pub actions: ::prost::alloc::vec::Vec<Action>,
    /// Initial state, including default values of state variables.
    #[prost(message, repeated, tag = "7")]
    pub initial_state: ::prost::alloc::vec::Vec<Assignment>,
    /// Facts and effects that are expected to occur strictly later than the initial state.
    /// features: TIMED_EFFECTS
    #[prost(message, repeated, tag = "8")]
    pub timed_effects: ::prost::alloc::vec::Vec<TimedEffect>,
    /// Goals of the planning problem.
    #[prost(message, repeated, tag = "9")]
    pub goals: ::prost::alloc::vec::Vec<Goal>,
    /// all features of the problem
    #[prost(enumeration = "Feature", repeated, tag = "10")]
    pub features: ::prost::alloc::vec::Vec<i32>,
    /// The plan quality metrics
    #[prost(message, repeated, tag = "11")]
    pub metrics: ::prost::alloc::vec::Vec<Metric>,
    /// If the problem is hierarchical, defines the tasks and methods as well as the initial task network.
    /// features: HIERARCHICAL
    #[prost(message, optional, tag = "12")]
    pub hierarchy: ::core::option::Option<Hierarchy>,
    /// Scheduling-specific extension of the problem.
    /// features: SCHEDULING
    #[prost(message, optional, tag = "17")]
    pub scheduling_extension: ::core::option::Option<SchedulingExtension>,
    /// Trajectory constraints of the planning problem.
    #[prost(message, repeated, tag = "13")]
    pub trajectory_constraints: ::prost::alloc::vec::Vec<Expression>,
    /// Flag defining if the time is discrete
    #[prost(bool, tag = "14")]
    pub discrete_time: bool,
    /// Flag defining if the self_overlapping is allowed
    #[prost(bool, tag = "15")]
    pub self_overlapping: bool,
    /// Optional. epsilon required by the problem
    #[prost(message, optional, tag = "16")]
    pub epsilon: ::core::option::Option<Real>,
}
/// Representation of an action instance that appears in a plan.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ActionInstance {
    /// Optional. A unique identifier of the action that might be used to refer to it (e.g. in HTN plans).
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
    /// name of the action
    #[prost(string, tag = "2")]
    pub action_name: ::prost::alloc::string::String,
    /// Parameters of the action instance, required to be constants.
    #[prost(message, repeated, tag = "3")]
    pub parameters: ::prost::alloc::vec::Vec<Atom>,
    /// Start time of the action. The default 0 value is OK in the case of non-temporal planning
    /// feature: \[DURATIVE_ACTIONS\]
    #[prost(message, optional, tag = "4")]
    pub start_time: ::core::option::Option<Real>,
    /// End time of the action. The default 0 value is OK in the case of non-temporal planning
    /// feature: \[DURATIVE_ACTIONS\]
    #[prost(message, optional, tag = "5")]
    pub end_time: ::core::option::Option<Real>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MethodInstance {
    ///   A unique identifier of the method that is used to refer to it in the hierarchy.
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
    /// name of the method
    #[prost(string, tag = "2")]
    pub method_name: ::prost::alloc::string::String,
    /// Parameters of the method instance, required to be constants.
    #[prost(message, repeated, tag = "3")]
    pub parameters: ::prost::alloc::vec::Vec<Atom>,
    /// A mapping of the IDs of the method's subtasks into the IDs of the action/methods that refine them.
    #[prost(map = "string, string", tag = "6")]
    pub subtasks: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PlanHierarchy {
    /// A mapping of the root task IDs into the IDs of the actions and methods that refine them.
    #[prost(map = "string, string", tag = "1")]
    pub root_tasks: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
    /// Instances of all methods used in the plan.
    #[prost(message, repeated, tag = "2")]
    pub methods: ::prost::alloc::vec::Vec<MethodInstance>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Plan {
    /// An ordered sequence of actions that appear in the plan.
    /// The order of the actions in the list must be compatible with the partial order of the start times.
    /// In case of non-temporal planning, this allows having all start time at 0 and only rely on the order in this sequence.
    /// features: ACTION_BASED
    #[prost(message, repeated, tag = "1")]
    pub actions: ::prost::alloc::vec::Vec<ActionInstance>,
    /// When the plan is hierarchical, this object provides the decomposition of hte root tasks into the actions of the plan
    /// feature: HIERARCHY
    #[prost(message, optional, tag = "2")]
    pub hierarchy: ::core::option::Option<PlanHierarchy>,
    /// Solution representation of a scheduling problem.
    /// feature: SCHEDULING
    #[prost(message, optional, tag = "3")]
    pub schedule: ::core::option::Option<Schedule>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PlanRequest {
    /// Problem that should be solved.
    #[prost(message, optional, tag = "1")]
    pub problem: ::core::option::Option<Problem>,
    #[prost(enumeration = "plan_request::Mode", tag = "2")]
    pub resolution_mode: i32,
    /// Max allowed runtime time in seconds.
    #[prost(double, tag = "3")]
    pub timeout: f64,
    /// Engine specific options to be passed to the engine
    #[prost(map = "string, string", tag = "4")]
    pub engine_options: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
}
/// Nested message and enum types in `PlanRequest`.
pub mod plan_request {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Mode {
        Satisfiable = 0,
        SolvedOptimally = 1,
    }
    impl Mode {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Mode::Satisfiable => "SATISFIABLE",
                Mode::SolvedOptimally => "SOLVED_OPTIMALLY",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "SATISFIABLE" => Some(Self::Satisfiable),
                "SOLVED_OPTIMALLY" => Some(Self::SolvedOptimally),
                _ => None,
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ValidationRequest {
    /// Problem to be validated.
    #[prost(message, optional, tag = "1")]
    pub problem: ::core::option::Option<Problem>,
    /// Plan to validate.
    #[prost(message, optional, tag = "2")]
    pub plan: ::core::option::Option<Plan>,
}
/// A freely formatted logging message.
/// Each message is annotated with its criticality level from the minimal (DEBUG) to the maximal (ERROR).
/// Criticality level is expected to be used by an end user to decide the level of verbosity.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LogMessage {
    #[prost(enumeration = "log_message::LogLevel", tag = "1")]
    pub level: i32,
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
}
/// Nested message and enum types in `LogMessage`.
pub mod log_message {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum LogLevel {
        Debug = 0,
        Info = 1,
        Warning = 2,
        Error = 3,
    }
    impl LogLevel {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                LogLevel::Debug => "DEBUG",
                LogLevel::Info => "INFO",
                LogLevel::Warning => "WARNING",
                LogLevel::Error => "ERROR",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "DEBUG" => Some(Self::Debug),
                "INFO" => Some(Self::Info),
                "WARNING" => Some(Self::Warning),
                "ERROR" => Some(Self::Error),
                _ => None,
            }
        }
    }
}
/// Message sent by engine.
/// Contains the engine exit status as well as the best plan found if any.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PlanGenerationResult {
    #[prost(enumeration = "plan_generation_result::Status", tag = "1")]
    pub status: i32,
    /// Optional. Best plan found if any.
    #[prost(message, optional, tag = "2")]
    pub plan: ::core::option::Option<Plan>,
    /// A set of engine specific values that can be reported, for instance
    /// - "grounding-time": "10ms"
    /// - "expanded-states": "1290"
    #[prost(map = "string, string", tag = "3")]
    pub metrics: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
    /// Optional log messages about the engine's activity.
    /// Note that it should not be expected that logging messages are visible to the end user.
    /// If used in conjunction with INTERNAL_ERROR or UNSUPPORTED_PROBLEM, it would be expected to have at least one log message at the ERROR level.
    #[prost(message, repeated, tag = "4")]
    pub log_messages: ::prost::alloc::vec::Vec<LogMessage>,
    /// Synthetic description of the engine that generated this message.
    #[prost(message, optional, tag = "5")]
    pub engine: ::core::option::Option<Engine>,
}
/// Nested message and enum types in `PlanGenerationResult`.
pub mod plan_generation_result {
    /// ==== Engine stopped normally ======
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
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
    impl Status {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Status::SolvedSatisficing => "SOLVED_SATISFICING",
                Status::SolvedOptimally => "SOLVED_OPTIMALLY",
                Status::UnsolvableProven => "UNSOLVABLE_PROVEN",
                Status::UnsolvableIncompletely => "UNSOLVABLE_INCOMPLETELY",
                Status::Timeout => "TIMEOUT",
                Status::Memout => "MEMOUT",
                Status::InternalError => "INTERNAL_ERROR",
                Status::UnsupportedProblem => "UNSUPPORTED_PROBLEM",
                Status::Intermediate => "INTERMEDIATE",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "SOLVED_SATISFICING" => Some(Self::SolvedSatisficing),
                "SOLVED_OPTIMALLY" => Some(Self::SolvedOptimally),
                "UNSOLVABLE_PROVEN" => Some(Self::UnsolvableProven),
                "UNSOLVABLE_INCOMPLETELY" => Some(Self::UnsolvableIncompletely),
                "TIMEOUT" => Some(Self::Timeout),
                "MEMOUT" => Some(Self::Memout),
                "INTERNAL_ERROR" => Some(Self::InternalError),
                "UNSUPPORTED_PROBLEM" => Some(Self::UnsupportedProblem),
                "INTERMEDIATE" => Some(Self::Intermediate),
                _ => None,
            }
        }
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Engine {
    /// Short name of the engine (planner, validator, ...)
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
}
/// Message sent by the validator.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ValidationResult {
    #[prost(enumeration = "validation_result::ValidationResultStatus", tag = "1")]
    pub status: i32,
    /// A set of engine specific values that can be reported
    #[prost(map = "string, string", tag = "4")]
    pub metrics: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
    /// Optional. Information given by the engine to the user.
    #[prost(message, repeated, tag = "2")]
    pub log_messages: ::prost::alloc::vec::Vec<LogMessage>,
    /// Synthetic description of the engine that generated this message.
    #[prost(message, optional, tag = "3")]
    pub engine: ::core::option::Option<Engine>,
}
/// Nested message and enum types in `ValidationResult`.
pub mod validation_result {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum ValidationResultStatus {
        /// The Plan is valid for the Problem.
        Valid = 0,
        /// The Plan is not valid for the Problem.
        Invalid = 1,
        /// The engine can't determine if the plan is VALID or INVALID for the Problem.
        Unknown = 2,
    }
    impl ValidationResultStatus {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                ValidationResultStatus::Valid => "VALID",
                ValidationResultStatus::Invalid => "INVALID",
                ValidationResultStatus::Unknown => "UNKNOWN",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "VALID" => Some(Self::Valid),
                "INVALID" => Some(Self::Invalid),
                "UNKNOWN" => Some(Self::Unknown),
                _ => None,
            }
        }
    }
}
/// Message sent by the grounder.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CompilerResult {
    /// The problem generated by the Compiler
    #[prost(message, optional, tag = "1")]
    pub problem: ::core::option::Option<Problem>,
    /// The map_back_plan field is a map from the ActionInstance of the
    /// compiled problem to the original ActionInstance.
    #[prost(map = "string, message", tag = "2")]
    pub map_back_plan: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ActionInstance,
    >,
    /// A set of engine specific values that can be reported
    #[prost(map = "string, string", tag = "5")]
    pub metrics: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
    /// Optional. Information given by the engine to the user.
    #[prost(message, repeated, tag = "3")]
    pub log_messages: ::prost::alloc::vec::Vec<LogMessage>,
    /// Synthetic description of the engine that generated this message.
    #[prost(message, optional, tag = "4")]
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
impl ExpressionKind {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ExpressionKind::Unknown => "UNKNOWN",
            ExpressionKind::Constant => "CONSTANT",
            ExpressionKind::Parameter => "PARAMETER",
            ExpressionKind::Variable => "VARIABLE",
            ExpressionKind::FluentSymbol => "FLUENT_SYMBOL",
            ExpressionKind::FunctionSymbol => "FUNCTION_SYMBOL",
            ExpressionKind::StateVariable => "STATE_VARIABLE",
            ExpressionKind::FunctionApplication => "FUNCTION_APPLICATION",
            ExpressionKind::ContainerId => "CONTAINER_ID",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "UNKNOWN" => Some(Self::Unknown),
            "CONSTANT" => Some(Self::Constant),
            "PARAMETER" => Some(Self::Parameter),
            "VARIABLE" => Some(Self::Variable),
            "FLUENT_SYMBOL" => Some(Self::FluentSymbol),
            "FUNCTION_SYMBOL" => Some(Self::FunctionSymbol),
            "STATE_VARIABLE" => Some(Self::StateVariable),
            "FUNCTION_APPLICATION" => Some(Self::FunctionApplication),
            "CONTAINER_ID" => Some(Self::ContainerId),
            _ => None,
        }
    }
}
/// Features of the problem.
/// Features are essential in that not supporting a feature `X` should allow disregarding any field tagged with `features: \[X\]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Feature {
    /// PROBLEM_CLASS
    ActionBased = 0,
    Hierarchical = 26,
    Scheduling = 56,
    /// PROBLEM_TYPE
    SimpleNumericPlanning = 30,
    GeneralNumericPlanning = 31,
    /// TIME
    ContinuousTime = 1,
    DiscreteTime = 2,
    IntermediateConditionsAndEffects = 3,
    ExternalConditionsAndEffects = 39,
    TimedEffects = 4,
    TimedGoals = 5,
    DurationInequalities = 6,
    SelfOverlapping = 47,
    /// EXPRESSION_DURATION
    StaticFluentsInDurations = 27,
    FluentsInDurations = 28,
    RealTypeDurations = 62,
    IntTypeDurations = 63,
    /// NUMBERS
    ContinuousNumbers = 7,
    DiscreteNumbers = 8,
    BoundedTypes = 38,
    /// CONDITIONS_KIND
    NegativeConditions = 9,
    DisjunctiveConditions = 10,
    Equalities = 11,
    ExistentialConditions = 12,
    UniversalConditions = 13,
    /// EFFECTS_KIND
    ConditionalEffects = 14,
    IncreaseEffects = 15,
    DecreaseEffects = 16,
    StaticFluentsInBooleanAssignments = 41,
    StaticFluentsInNumericAssignments = 42,
    StaticFluentsInObjectAssignments = 57,
    FluentsInBooleanAssignments = 43,
    FluentsInNumericAssignments = 44,
    FluentsInObjectAssignments = 58,
    ForallEffects = 59,
    /// TYPING
    FlatTyping = 17,
    HierarchicalTyping = 18,
    /// FLUENTS_TYPE
    NumericFluents = 19,
    ObjectFluents = 20,
    IntFluents = 60,
    RealFluents = 61,
    /// PARAMETERS
    BoolFluentParameters = 50,
    BoundedIntFluentParameters = 51,
    BoolActionParameters = 52,
    BoundedIntActionParameters = 53,
    UnboundedIntActionParameters = 54,
    RealActionParameters = 55,
    /// QUALITY_METRICS
    ActionsCost = 21,
    FinalValue = 22,
    Makespan = 23,
    PlanLength = 24,
    Oversubscription = 29,
    TemporalOversubscription = 40,
    /// ACTION_COST_KIND
    StaticFluentsInActionsCost = 45,
    FluentsInActionsCost = 46,
    RealNumbersInActionsCost = 64,
    IntNumbersInActionsCost = 65,
    /// OVERSUBSCRIPTION_KIND
    RealNumbersInOversubscription = 66,
    IntNumbersInOversubscription = 67,
    /// SIMULATED_ENTITIES
    SimulatedEffects = 25,
    /// CONSTRAINTS_KIND
    TrajectoryConstraints = 48,
    StateInvariants = 49,
    /// HIERARCHICAL
    MethodPreconditions = 32,
    TaskNetworkConstraints = 33,
    InitialTaskNetworkVariables = 34,
    TaskOrderTotal = 35,
    TaskOrderPartial = 36,
    TaskOrderTemporal = 37,
}
impl Feature {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Feature::ActionBased => "ACTION_BASED",
            Feature::Hierarchical => "HIERARCHICAL",
            Feature::Scheduling => "SCHEDULING",
            Feature::SimpleNumericPlanning => "SIMPLE_NUMERIC_PLANNING",
            Feature::GeneralNumericPlanning => "GENERAL_NUMERIC_PLANNING",
            Feature::ContinuousTime => "CONTINUOUS_TIME",
            Feature::DiscreteTime => "DISCRETE_TIME",
            Feature::IntermediateConditionsAndEffects => {
                "INTERMEDIATE_CONDITIONS_AND_EFFECTS"
            }
            Feature::ExternalConditionsAndEffects => "EXTERNAL_CONDITIONS_AND_EFFECTS",
            Feature::TimedEffects => "TIMED_EFFECTS",
            Feature::TimedGoals => "TIMED_GOALS",
            Feature::DurationInequalities => "DURATION_INEQUALITIES",
            Feature::SelfOverlapping => "SELF_OVERLAPPING",
            Feature::StaticFluentsInDurations => "STATIC_FLUENTS_IN_DURATIONS",
            Feature::FluentsInDurations => "FLUENTS_IN_DURATIONS",
            Feature::RealTypeDurations => "REAL_TYPE_DURATIONS",
            Feature::IntTypeDurations => "INT_TYPE_DURATIONS",
            Feature::ContinuousNumbers => "CONTINUOUS_NUMBERS",
            Feature::DiscreteNumbers => "DISCRETE_NUMBERS",
            Feature::BoundedTypes => "BOUNDED_TYPES",
            Feature::NegativeConditions => "NEGATIVE_CONDITIONS",
            Feature::DisjunctiveConditions => "DISJUNCTIVE_CONDITIONS",
            Feature::Equalities => "EQUALITIES",
            Feature::ExistentialConditions => "EXISTENTIAL_CONDITIONS",
            Feature::UniversalConditions => "UNIVERSAL_CONDITIONS",
            Feature::ConditionalEffects => "CONDITIONAL_EFFECTS",
            Feature::IncreaseEffects => "INCREASE_EFFECTS",
            Feature::DecreaseEffects => "DECREASE_EFFECTS",
            Feature::StaticFluentsInBooleanAssignments => {
                "STATIC_FLUENTS_IN_BOOLEAN_ASSIGNMENTS"
            }
            Feature::StaticFluentsInNumericAssignments => {
                "STATIC_FLUENTS_IN_NUMERIC_ASSIGNMENTS"
            }
            Feature::StaticFluentsInObjectAssignments => {
                "STATIC_FLUENTS_IN_OBJECT_ASSIGNMENTS"
            }
            Feature::FluentsInBooleanAssignments => "FLUENTS_IN_BOOLEAN_ASSIGNMENTS",
            Feature::FluentsInNumericAssignments => "FLUENTS_IN_NUMERIC_ASSIGNMENTS",
            Feature::FluentsInObjectAssignments => "FLUENTS_IN_OBJECT_ASSIGNMENTS",
            Feature::ForallEffects => "FORALL_EFFECTS",
            Feature::FlatTyping => "FLAT_TYPING",
            Feature::HierarchicalTyping => "HIERARCHICAL_TYPING",
            Feature::NumericFluents => "NUMERIC_FLUENTS",
            Feature::ObjectFluents => "OBJECT_FLUENTS",
            Feature::IntFluents => "INT_FLUENTS",
            Feature::RealFluents => "REAL_FLUENTS",
            Feature::BoolFluentParameters => "BOOL_FLUENT_PARAMETERS",
            Feature::BoundedIntFluentParameters => "BOUNDED_INT_FLUENT_PARAMETERS",
            Feature::BoolActionParameters => "BOOL_ACTION_PARAMETERS",
            Feature::BoundedIntActionParameters => "BOUNDED_INT_ACTION_PARAMETERS",
            Feature::UnboundedIntActionParameters => "UNBOUNDED_INT_ACTION_PARAMETERS",
            Feature::RealActionParameters => "REAL_ACTION_PARAMETERS",
            Feature::ActionsCost => "ACTIONS_COST",
            Feature::FinalValue => "FINAL_VALUE",
            Feature::Makespan => "MAKESPAN",
            Feature::PlanLength => "PLAN_LENGTH",
            Feature::Oversubscription => "OVERSUBSCRIPTION",
            Feature::TemporalOversubscription => "TEMPORAL_OVERSUBSCRIPTION",
            Feature::StaticFluentsInActionsCost => "STATIC_FLUENTS_IN_ACTIONS_COST",
            Feature::FluentsInActionsCost => "FLUENTS_IN_ACTIONS_COST",
            Feature::RealNumbersInActionsCost => "REAL_NUMBERS_IN_ACTIONS_COST",
            Feature::IntNumbersInActionsCost => "INT_NUMBERS_IN_ACTIONS_COST",
            Feature::RealNumbersInOversubscription => "REAL_NUMBERS_IN_OVERSUBSCRIPTION",
            Feature::IntNumbersInOversubscription => "INT_NUMBERS_IN_OVERSUBSCRIPTION",
            Feature::SimulatedEffects => "SIMULATED_EFFECTS",
            Feature::TrajectoryConstraints => "TRAJECTORY_CONSTRAINTS",
            Feature::StateInvariants => "STATE_INVARIANTS",
            Feature::MethodPreconditions => "METHOD_PRECONDITIONS",
            Feature::TaskNetworkConstraints => "TASK_NETWORK_CONSTRAINTS",
            Feature::InitialTaskNetworkVariables => "INITIAL_TASK_NETWORK_VARIABLES",
            Feature::TaskOrderTotal => "TASK_ORDER_TOTAL",
            Feature::TaskOrderPartial => "TASK_ORDER_PARTIAL",
            Feature::TaskOrderTemporal => "TASK_ORDER_TEMPORAL",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ACTION_BASED" => Some(Self::ActionBased),
            "HIERARCHICAL" => Some(Self::Hierarchical),
            "SCHEDULING" => Some(Self::Scheduling),
            "SIMPLE_NUMERIC_PLANNING" => Some(Self::SimpleNumericPlanning),
            "GENERAL_NUMERIC_PLANNING" => Some(Self::GeneralNumericPlanning),
            "CONTINUOUS_TIME" => Some(Self::ContinuousTime),
            "DISCRETE_TIME" => Some(Self::DiscreteTime),
            "INTERMEDIATE_CONDITIONS_AND_EFFECTS" => {
                Some(Self::IntermediateConditionsAndEffects)
            }
            "EXTERNAL_CONDITIONS_AND_EFFECTS" => Some(Self::ExternalConditionsAndEffects),
            "TIMED_EFFECTS" => Some(Self::TimedEffects),
            "TIMED_GOALS" => Some(Self::TimedGoals),
            "DURATION_INEQUALITIES" => Some(Self::DurationInequalities),
            "SELF_OVERLAPPING" => Some(Self::SelfOverlapping),
            "STATIC_FLUENTS_IN_DURATIONS" => Some(Self::StaticFluentsInDurations),
            "FLUENTS_IN_DURATIONS" => Some(Self::FluentsInDurations),
            "REAL_TYPE_DURATIONS" => Some(Self::RealTypeDurations),
            "INT_TYPE_DURATIONS" => Some(Self::IntTypeDurations),
            "CONTINUOUS_NUMBERS" => Some(Self::ContinuousNumbers),
            "DISCRETE_NUMBERS" => Some(Self::DiscreteNumbers),
            "BOUNDED_TYPES" => Some(Self::BoundedTypes),
            "NEGATIVE_CONDITIONS" => Some(Self::NegativeConditions),
            "DISJUNCTIVE_CONDITIONS" => Some(Self::DisjunctiveConditions),
            "EQUALITIES" => Some(Self::Equalities),
            "EXISTENTIAL_CONDITIONS" => Some(Self::ExistentialConditions),
            "UNIVERSAL_CONDITIONS" => Some(Self::UniversalConditions),
            "CONDITIONAL_EFFECTS" => Some(Self::ConditionalEffects),
            "INCREASE_EFFECTS" => Some(Self::IncreaseEffects),
            "DECREASE_EFFECTS" => Some(Self::DecreaseEffects),
            "STATIC_FLUENTS_IN_BOOLEAN_ASSIGNMENTS" => {
                Some(Self::StaticFluentsInBooleanAssignments)
            }
            "STATIC_FLUENTS_IN_NUMERIC_ASSIGNMENTS" => {
                Some(Self::StaticFluentsInNumericAssignments)
            }
            "STATIC_FLUENTS_IN_OBJECT_ASSIGNMENTS" => {
                Some(Self::StaticFluentsInObjectAssignments)
            }
            "FLUENTS_IN_BOOLEAN_ASSIGNMENTS" => Some(Self::FluentsInBooleanAssignments),
            "FLUENTS_IN_NUMERIC_ASSIGNMENTS" => Some(Self::FluentsInNumericAssignments),
            "FLUENTS_IN_OBJECT_ASSIGNMENTS" => Some(Self::FluentsInObjectAssignments),
            "FORALL_EFFECTS" => Some(Self::ForallEffects),
            "FLAT_TYPING" => Some(Self::FlatTyping),
            "HIERARCHICAL_TYPING" => Some(Self::HierarchicalTyping),
            "NUMERIC_FLUENTS" => Some(Self::NumericFluents),
            "OBJECT_FLUENTS" => Some(Self::ObjectFluents),
            "INT_FLUENTS" => Some(Self::IntFluents),
            "REAL_FLUENTS" => Some(Self::RealFluents),
            "BOOL_FLUENT_PARAMETERS" => Some(Self::BoolFluentParameters),
            "BOUNDED_INT_FLUENT_PARAMETERS" => Some(Self::BoundedIntFluentParameters),
            "BOOL_ACTION_PARAMETERS" => Some(Self::BoolActionParameters),
            "BOUNDED_INT_ACTION_PARAMETERS" => Some(Self::BoundedIntActionParameters),
            "UNBOUNDED_INT_ACTION_PARAMETERS" => Some(Self::UnboundedIntActionParameters),
            "REAL_ACTION_PARAMETERS" => Some(Self::RealActionParameters),
            "ACTIONS_COST" => Some(Self::ActionsCost),
            "FINAL_VALUE" => Some(Self::FinalValue),
            "MAKESPAN" => Some(Self::Makespan),
            "PLAN_LENGTH" => Some(Self::PlanLength),
            "OVERSUBSCRIPTION" => Some(Self::Oversubscription),
            "TEMPORAL_OVERSUBSCRIPTION" => Some(Self::TemporalOversubscription),
            "STATIC_FLUENTS_IN_ACTIONS_COST" => Some(Self::StaticFluentsInActionsCost),
            "FLUENTS_IN_ACTIONS_COST" => Some(Self::FluentsInActionsCost),
            "REAL_NUMBERS_IN_ACTIONS_COST" => Some(Self::RealNumbersInActionsCost),
            "INT_NUMBERS_IN_ACTIONS_COST" => Some(Self::IntNumbersInActionsCost),
            "REAL_NUMBERS_IN_OVERSUBSCRIPTION" => {
                Some(Self::RealNumbersInOversubscription)
            }
            "INT_NUMBERS_IN_OVERSUBSCRIPTION" => Some(Self::IntNumbersInOversubscription),
            "SIMULATED_EFFECTS" => Some(Self::SimulatedEffects),
            "TRAJECTORY_CONSTRAINTS" => Some(Self::TrajectoryConstraints),
            "STATE_INVARIANTS" => Some(Self::StateInvariants),
            "METHOD_PRECONDITIONS" => Some(Self::MethodPreconditions),
            "TASK_NETWORK_CONSTRAINTS" => Some(Self::TaskNetworkConstraints),
            "INITIAL_TASK_NETWORK_VARIABLES" => Some(Self::InitialTaskNetworkVariables),
            "TASK_ORDER_TOTAL" => Some(Self::TaskOrderTotal),
            "TASK_ORDER_PARTIAL" => Some(Self::TaskOrderPartial),
            "TASK_ORDER_TEMPORAL" => Some(Self::TaskOrderTemporal),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod unified_planning_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
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
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
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
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// An anytime plan request to the engine.
        /// The engine replies with a stream of N `Answer` messages where:
        ///  - the first (N-1) message are of type `IntermediateReport`
        ///  - the last message is of type `FinalReport`
        pub async fn plan_anytime(
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
                "/UnifiedPlanning/planAnytime",
            );
            self.inner.server_streaming(request.into_request(), path, codec).await
        }
        /// A oneshot plan request to the engine.
        /// The engine replies with athe PlanGenerationResult
        pub async fn plan_one_shot(
            &mut self,
            request: impl tonic::IntoRequest<super::PlanRequest>,
        ) -> Result<tonic::Response<super::PlanGenerationResult>, tonic::Status> {
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
            self.inner.unary(request.into_request(), path, codec).await
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
    /// Generated trait containing gRPC methods that should be implemented for use with UnifiedPlanningServer.
    #[async_trait]
    pub trait UnifiedPlanning: Send + Sync + 'static {
        /// Server streaming response type for the planAnytime method.
        type planAnytimeStream: futures_core::Stream<
                Item = Result<super::PlanGenerationResult, tonic::Status>,
            >
            + Send
            + 'static;
        /// An anytime plan request to the engine.
        /// The engine replies with a stream of N `Answer` messages where:
        ///  - the first (N-1) message are of type `IntermediateReport`
        ///  - the last message is of type `FinalReport`
        async fn plan_anytime(
            &self,
            request: tonic::Request<super::PlanRequest>,
        ) -> Result<tonic::Response<Self::planAnytimeStream>, tonic::Status>;
        /// A oneshot plan request to the engine.
        /// The engine replies with athe PlanGenerationResult
        async fn plan_one_shot(
            &self,
            request: tonic::Request<super::PlanRequest>,
        ) -> Result<tonic::Response<super::PlanGenerationResult>, tonic::Status>;
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
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
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
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
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
                "/UnifiedPlanning/planAnytime" => {
                    #[allow(non_camel_case_types)]
                    struct planAnytimeSvc<T: UnifiedPlanning>(pub Arc<T>);
                    impl<
                        T: UnifiedPlanning,
                    > tonic::server::ServerStreamingService<super::PlanRequest>
                    for planAnytimeSvc<T> {
                        type Response = super::PlanGenerationResult;
                        type ResponseStream = T::planAnytimeStream;
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
                                (*inner).plan_anytime(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = planAnytimeSvc(inner);
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
                "/UnifiedPlanning/planOneShot" => {
                    #[allow(non_camel_case_types)]
                    struct planOneShotSvc<T: UnifiedPlanning>(pub Arc<T>);
                    impl<
                        T: UnifiedPlanning,
                    > tonic::server::UnaryService<super::PlanRequest>
                    for planOneShotSvc<T> {
                        type Response = super::PlanGenerationResult;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
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
                        let res = grpc.unary(method, req).await;
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
    impl<T: UnifiedPlanning> tonic::server::NamedService for UnifiedPlanningServer<T> {
        const NAME: &'static str = "UnifiedPlanning";
    }
}
