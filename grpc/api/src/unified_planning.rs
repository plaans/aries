// =============== Types ================

// Type of expressions are represented as strings in protobuf format.
// A type might be, e.g., "int", "bool" or "location", where the latter is a problem-specific type.

// Built-in types:
//  - "bool"
//  - "int"
//  - "real"
//
// Any other string (e.g. "location") refers to a symbolic type and must have been declared in the problem definition.

// We can also consider restrictions to int/reals with specific syntax (e.g. "int\[0,100\]")
// but we need to agree on the semantics and syntax.

// ================== Expressions ====================

/// As in s-expression, an Expression is either an atom or list representing the application of some parameters to a function/fluent.
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
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Atom {
    #[prost(oneof = "atom::Content", tags = "1, 2, 3, 4")]
    pub content: ::core::option::Option<atom::Content>,
}
/// Nested message and enum types in `Atom`.
pub mod atom {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Content {
        #[prost(string, tag = "1")]
        Symbol(::prost::alloc::string::String),
        #[prost(int64, tag = "2")]
        Int(i64),
        #[prost(double, tag = "3")]
        Float(f64),
        #[prost(bool, tag = "4")]
        Boolean(bool),
    }
}
// ============= Domains ====================

/// Declares the existence of a symbolic type.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TypeDeclaration {
    /// Name of the type that is declared.
    #[prost(string, tag = "1")]
    pub type_name: ::prost::alloc::string::String,
    /// If the string is non-empty, this is the parent type of `type_name`.
    /// If set, the parent type must have been previously declared (i.e. should appear earlier in the problem's type declarations.
    #[prost(string, tag = "2")]
    pub parent_type: ::prost::alloc::string::String,
}
/// Parameter of a fluent or of an action
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
}
/// Declares an object with the given name and type.
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
// ========= Actions ========

/// An effect expression is of the form `FLUENT OP VALUE`.
/// We explicitly restrict the different types of effects by setting the allowed operators.
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
    /// features: conditional_effects
    #[prost(message, optional, tag = "4")]
    pub condition: ::core::option::Option<Expression>,
}
/// Nested message and enum types in `EffectExpression`.
pub mod effect_expression {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum EffectKind {
        /// This is the value that will be taken if the operator is not explicitly set.
        /// It is currently a logic error to have this value but is needed to allow later extensions.
        Undefined = 0,
        /// The `fluent` is set to the corresponding `value`
        Assign = 1,
        /// The `fluent` is increased by the amount `value`
        /// features: numeric?
        Increase = 2,
        /// The `fluent` is decreased by the amount `value`
        /// features: numeric?
        Decrease = 3,
    }
}
/// Representation of an effect that allows qualifying the effect expression, e.g., to make it a conditional effect.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Effect {
    /// Required. The actual effect that should take place.
    #[prost(message, optional, tag = "1")]
    pub effect: ::core::option::Option<EffectExpression>,
    /// Optional. If the effect is within a durative action, the following must be set and will specify when the effect takes place.
    /// features: durative_actions
    #[prost(message, optional, tag = "2")]
    pub occurence_time: ::core::option::Option<Timing>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Condition {
    #[prost(message, optional, tag = "1")]
    pub cond: ::core::option::Option<Expression>,
    /// Optional. Must be set for durative actions where it specifies the temporal interval
    /// over which when the condition should hold.
    /// features: durative_actions
    #[prost(message, optional, tag = "2")]
    pub span: ::core::option::Option<TimeInterval>,
}
/// Unified action representation that represents any kind of actions.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Action {
    /// Action name. E.g. "move"
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// Typed and named parameters of the action.
    #[prost(message, repeated, tag = "2")]
    pub parameters: ::prost::alloc::vec::Vec<Parameter>,
    /// If set, the action is durative. Otherwise it is instantaneous.
    /// features: durative_actions
    #[prost(message, optional, tag = "3")]
    pub duration: ::core::option::Option<Duration>,
    /// Conjunction of conditions that must hold for the action to be applicable.
    #[prost(message, repeated, tag = "4")]
    pub conditions: ::prost::alloc::vec::Vec<Condition>,
    /// Conjunction of effects as a result of applying this action.
    #[prost(message, repeated, tag = "5")]
    pub effects: ::prost::alloc::vec::Vec<Effect>,
    /// Cost of the action.
    /// features: action_costs
    #[prost(message, optional, tag = "6")]
    pub cost: ::core::option::Option<Expression>,
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
    #[prost(enumeration = "timepoint::TimepointKind", tag = "1")]
    pub kind: i32,
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
        /// Start of the container (typically the action) in which this symbol occurs
        Start = 2,
        /// End of the container (typically the action) in which this symbol occurs
        End = 3,
    }
}
/// Represents a time (`timepoint` + `delay`), that is a time defined relatively to a particular `timepoint`.
/// Note that an absolute time can be defined by setting the `delay` relative to the `GLOBAL_START`` which is the reference time.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Timing {
    #[prost(message, optional, tag = "1")]
    pub timepoint: ::core::option::Option<Timepoint>,
    #[prost(double, tag = "2")]
    pub delay: f64,
}
/// An interval `[lower, upper]` where `lower` and `upper` are arbitrary expressions.
/// The `is_left_open` and `is_right_open` fields indicate whether the interval is
/// opened on left and right side respectively.
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
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Duration {
    //// The duration of the action can be freely chosen within the indicated bounds
    #[prost(message, optional, tag = "1")]
    pub controllable_in_bounds: ::core::option::Option<Interval>,
}
// ============== Problem =========================

/// A Goal is currently an expression that must hold either:
/// - in the final state,
/// - over a specific temporal interval (under the `timed_goals` features)
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Goal {
    /// Goal expression that must hold in the final state.
    #[prost(message, optional, tag = "1")]
    pub goal: ::core::option::Option<Expression>,
    /// Optional. If specified the goal should hold over the specified temporal interval (instead of on the final state).
    /// features: timed_goals
    #[prost(message, optional, tag = "2")]
    pub timing: ::core::option::Option<TimeInterval>,
}
/// Represents an effect that will occur sometime beyond the initial state. (similar to timed initial literals)
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TimedEffect {
    /// Required. An effect expression taht will take place sometime in the future (i.e. not at the intial state) as specified by the temporal qualifiation.
    #[prost(message, optional, tag = "1")]
    pub effect: ::core::option::Option<EffectExpression>,
    /// Required. Temporal qualification denoting when the timed fact will occur.
    #[prost(message, optional, tag = "2")]
    pub occurence_time: ::core::option::Option<Timing>,
}
/// An assigment of a value to a fluent, as it appears in the initial state definition.
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
    #[prost(message, repeated, tag = "6")]
    pub actions: ::prost::alloc::vec::Vec<Action>,
    /// Initial state. It is asssumed that the initial state is fully defined by assignments.
    #[prost(message, repeated, tag = "7")]
    pub initial_state: ::prost::alloc::vec::Vec<Assignment>,
    /// Facts and effects that are expected to occur strictly later than the initial state.
    /// features: timed_effects
    #[prost(message, repeated, tag = "8")]
    pub timed_effects: ::prost::alloc::vec::Vec<TimedEffect>,
    /// Goals of the planning problem.
    #[prost(message, repeated, tag = "9")]
    pub goals: ::prost::alloc::vec::Vec<Goal>,
    /// all features of the problem
    #[prost(enumeration = "Feature", repeated, tag = "15")]
    pub features: ::prost::alloc::vec::Vec<i32>,
}
// =================== Plan ================

/// Representation of an action instance that appears in a plan.
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
    /// feature: \[durative_actions\]
    #[prost(double, tag = "4")]
    pub start_time: f64,
    /// End time of the action. The default 0 value is OK in the case of non-temporal planning
    /// feature: \[durative_actions\]
    #[prost(double, tag = "5")]
    pub end_time: f64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Plan {
    /// An ordered sequence of actions that appear in the plan.
    /// The order of the actions in the list must be compatible with the partial order of the start times.
    /// In case of non-temporal planning, this allows having all start time at 0 and only rely on the order in this sequence.
    #[prost(message, repeated, tag = "1")]
    pub actions: ::prost::alloc::vec::Vec<ActionInstance>,
}
// =============== RPC API =======================

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PlanRequest {
    /// Problem that should be solved.
    #[prost(message, optional, tag = "1")]
    pub problem: ::core::option::Option<Problem>,
    #[prost(enumeration = "plan_request::Mode", tag = "2")]
    pub resolution_mode: i32,
    /// Max allowed runtime time in seconds.
    #[prost(double, tag = "3")]
    pub timeout_seconds: f64,
    /// Planner specific options to be passed to the planner
    #[prost(map = "string, string", tag = "4")]
    pub planner_options: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
}
/// Nested message and enum types in `PlanRequest`.
pub mod plan_request {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Mode {
        Satisfiable = 0,
        Optimal = 1,
    }
}
/// A freely formatted logging message.
/// Each message is annotated with its criticality level from the minimal (DEBUG) to the maximal (ERROR).
/// Criticality level is expected to be used by an end user to decide the level of verbosity.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LogMessage {
    #[prost(enumeration = "log_message::LogLevel", tag = "1")]
    pub level: i32,
    #[prost(string, tag = "2")]
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
/// Intermediate report sent by the planner while running.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct IntermediateReport {
    /// Optional. If set, it is the latest found plan not already reported.
    #[prost(message, optional, tag = "1")]
    pub plan: ::core::option::Option<Plan>,
    #[prost(message, repeated, tag = "2")]
    pub logs: ::prost::alloc::vec::Vec<LogMessage>,
    /// Planner specific messages
    #[prost(map = "string, string", tag = "3")]
    pub metrics: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
}
/// Last message sent by planner before exiting.
/// Contains the planner exit status as well as the best plan found if any.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FinalReport {
    #[prost(enumeration = "final_report::Status", tag = "1")]
    pub status: i32,
    /// Optional. Best plan found if any.
    #[prost(message, optional, tag = "2")]
    pub best_plan: ::core::option::Option<Plan>,
    /// A set of planner specific values that can be reported, for instance
    /// - "grounding-time": "10ms"
    /// - "expanded-states": "1290"
    #[prost(map = "string, string", tag = "3")]
    pub metrics: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    /// Optional logs about the planner's activity.
    /// Note that it should not be expected that logging messages are visible to the end user.
    /// If used in conjunction with INTERNAL_ERROR or UNSUPPORTED_PROBLEM, it would be expected to have at least one log message at the ERROR level.
    #[prost(message, repeated, tag = "4")]
    pub logs: ::prost::alloc::vec::Vec<LogMessage>,
}
/// Nested message and enum types in `FinalReport`.
pub mod final_report {
    /// ==== Planner stopped normally ======
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Status {
        /// Valid plan found and search stopped immediately
        /// The `best_plan` field must be set.
        Sat = 0,
        /// Plan found with optimality guarantee
        /// The `best_plan` field must be set and contain an optimal solution.
        Opt = 1,
        /// No plan exists
        Unsat = 2,
        /// The planner was not able to find a solution but does not give any guarantee that none exist
        /// (i.e. the planner might not be complete)
        SearchSpaceExhausted = 3,
        // ====== Planner exited before making any conclusion ====
        // Search stopped before concluding OPT or UNSAT
        // If a plan was found, it might be reported in the `best_plan` field
        /// The planner ran out of time
        Timeout = 13,
        /// The planner ran out of memory
        Memout = 14,
        /// The planner faced an internal error.
        InternalError = 15,
        /// The problem submitted is not supported by the planner.
        UnsupportedProblem = 16,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Answer {
    #[prost(oneof = "answer::Content", tags = "1, 2")]
    pub content: ::core::option::Option<answer::Content>,
}
/// Nested message and enum types in `Answer`.
pub mod answer {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Content {
        #[prost(message, tag = "1")]
        Intermediate(super::IntermediateReport),
        #[prost(message, tag = "2")]
        Final(super::FinalReport),
    }
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
    /// Atom symbol reprenting a fluent of the problem. For instance `at-robot`.
    FluentSymbol = 3,
    /// Atom representing a function. For instance `+`, `=`, `and`, ...
    FunctionSymbol = 4,
    /// List. Application of some parameters to a fluent symbol. For instance `(at-robot l1)` or `(battery-charged)`
    /// The first element of the list must be a FLUENT_SYMBOL
    StateVariable = 5,
    /// List. The expression is the application of some parameters to a function. For instance `(+ 1 3)`.
    /// The first element of the list must be a FUNCTION_SYMBOL
    FunctionApplication = 6,
}
/// Features of the problem.
/// Features are essential in that not supporting a feature `X` should allow disregarding any field tagged with `features: \[X\]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Feature {
    DurativeActions = 0,
    ConditionalEffects = 1,
    ActionCosts = 2,
    TimedEffects = 3,
    TimedGoals = 4,
}
#[doc = r" Generated client implementations."]
pub mod unified_planning_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    #[derive(Debug, Clone)]
    pub struct UnifiedPlanningClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl UnifiedPlanningClient<tonic::transport::Channel> {
        #[doc = r" Attempt to create a new client by connecting to a given endpoint."]
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
        T::ResponseBody: Body + Send + 'static,
        T::Error: Into<StdError>,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_interceptor<F>(inner: T, interceptor: F) -> UnifiedPlanningClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<<T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody>,
            >,
            <T as tonic::codegen::Service<http::Request<tonic::body::BoxBody>>>::Error: Into<StdError> + Send + Sync,
        {
            UnifiedPlanningClient::new(InterceptedService::new(inner, interceptor))
        }
        #[doc = r" Compress requests with `gzip`."]
        #[doc = r""]
        #[doc = r" This requires the server to support it otherwise it might respond with an"]
        #[doc = r" error."]
        pub fn send_gzip(mut self) -> Self {
            self.inner = self.inner.send_gzip();
            self
        }
        #[doc = r" Enable decompressing responses with `gzip`."]
        pub fn accept_gzip(mut self) -> Self {
            self.inner = self.inner.accept_gzip();
            self
        }
        #[doc = " A plan request to the planner."]
        #[doc = " The planner replies with a stream of N `Answer` messages where:"]
        #[doc = "  - the first (N-1) message are of type `IntermediateReport`"]
        #[doc = "  - the last message is of type `FinalReport`"]
        pub async fn plan_one_shot(
            &mut self,
            request: impl tonic::IntoRequest<super::PlanRequest>,
        ) -> Result<tonic::Response<tonic::codec::Streaming<super::Answer>>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(tonic::Code::Unknown, format!("Service was not ready: {}", e.into()))
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/UnifiedPlanning/planOneShot");
            self.inner.server_streaming(request.into_request(), path, codec).await
        }
    }
}
#[doc = r" Generated server implementations."]
pub mod unified_planning_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    #[doc = "Generated trait containing gRPC methods that should be implemented for use with UnifiedPlanningServer."]
    #[async_trait]
    pub trait UnifiedPlanning: Send + Sync + 'static {
        #[doc = "Server streaming response type for the planOneShot method."]
        type planOneShotStream: futures_core::Stream<Item = Result<super::Answer, tonic::Status>> + Send + 'static;
        #[doc = " A plan request to the planner."]
        #[doc = " The planner replies with a stream of N `Answer` messages where:"]
        #[doc = "  - the first (N-1) message are of type `IntermediateReport`"]
        #[doc = "  - the last message is of type `FinalReport`"]
        async fn plan_one_shot(
            &self,
            request: tonic::Request<super::PlanRequest>,
        ) -> Result<tonic::Response<Self::planOneShotStream>, tonic::Status>;
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
            let inner = Arc::new(inner);
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
            }
        }
        pub fn with_interceptor<F>(inner: T, interceptor: F) -> InterceptedService<Self, F>
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
        type Error = Never;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/UnifiedPlanning/planOneShot" => {
                    #[allow(non_camel_case_types)]
                    struct planOneShotSvc<T: UnifiedPlanning>(pub Arc<T>);
                    impl<T: UnifiedPlanning> tonic::server::ServerStreamingService<super::PlanRequest> for planOneShotSvc<T> {
                        type Response = super::Answer;
                        type ResponseStream = T::planOneShotStream;
                        type Future = BoxFuture<tonic::Response<Self::ResponseStream>, tonic::Status>;
                        fn call(&mut self, request: tonic::Request<super::PlanRequest>) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).plan_one_shot(request).await };
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
                            .apply_compression_config(accept_compression_encodings, send_compression_encodings);
                        let res = grpc.server_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .header("content-type", "application/grpc")
                        .body(empty_body())
                        .unwrap())
                }),
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
    impl<T: UnifiedPlanning> tonic::transport::NamedService for UnifiedPlanningServer<T> {
        const NAME: &'static str = "UnifiedPlanning";
    }
}
