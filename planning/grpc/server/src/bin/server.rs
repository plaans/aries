use anyhow::{bail, ensure, Context, Error};
use aries::model::extensions::SavedAssignment;
use aries_grpc_server::chronicles::problem_to_chronicles;
use aries_grpc_server::serialize::{engine, serialize_plan};
use aries_grpc_server::warm_up::plan_from_option;
use aries_plan_validator::validate_upf;
use aries_planners::solver;
use aries_planners::solver::{Metric, SolverResult, Strat};
use aries_planning::chronicles::analysis::hierarchy::hierarchical_is_non_recursive;
use aries_planning::chronicles::FiniteProblem;
use async_trait::async_trait;
use clap::{Args, Parser, Subcommand};
use env_param::EnvParam;
use itertools::Itertools;
use prost::Message;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};
use unified_planning as up;
use unified_planning::metric::MetricKind;
use unified_planning::unified_planning_server::{UnifiedPlanning, UnifiedPlanningServer};
use unified_planning::validation_result::ValidationResultStatus;
use unified_planning::{log_message, plan_generation_result, LogMessage, PlanGenerationResult, PlanRequest};
use unified_planning::{Problem, ValidationRequest, ValidationResult};

/// gRPC Server for the unified-planning integration of Aries.
#[derive(Parser, Debug)]
#[clap(about = "Aries, unified-planning server")]
struct App {
    /// Logging level to use: one of "error", "warn", "info", "debug", "trace"
    #[clap(short, long, default_value = "info")]
    log_level: tracing::Level,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Starts a gRPC server that can be used by up-aries integration.
    Serve(ServeArgs),
    /// Directly solve the problem in the indicated file (serialized to protobuf)
    Solve(SolveArgs),
}

#[derive(Debug, Args)]
struct ServeArgs {
    /// Address to listen on
    #[clap(short, long, default_value = "0.0.0.0:2222")]
    address: String,
}

#[derive(Debug, Args)]
struct SolveArgs {
    /// File from which to deserialize the unified planning problem
    problem_file: String,

    #[clap(flatten)]
    conf: SolverConfiguration,
}

#[derive(Debug, Args, Clone)]
pub struct SolverConfiguration {
    /// If true, the solver should look for optimal solutions
    #[clap(long)]
    pub optimal: bool,

    /// Timeout (s) after which search will stop
    #[clap(long)]
    pub timeout: Option<f64>,

    /// Minimal depth for the search.
    #[clap(long, default_value = "0")]
    pub min_depth: u32,

    /// Maximal depth for the search.
    #[clap(long, default_value = "4294967295")]
    pub max_depth: u32,

    /// If provided, the solver will only run the specified strategy instead of default set of strategies.
    /// When repeated, several strategies will be run in parallel.
    /// Allowed values: forward | activity | activity-bool | activity-bool-light | causal
    #[clap(long = "strategy", short = 's')]
    strategies: Vec<Strat>,

    #[clap(long)]
    pub warm_up_plan: Option<String>,
}

impl Default for SolverConfiguration {
    fn default() -> Self {
        SolverConfiguration {
            optimal: false,
            timeout: None,
            min_depth: 0,
            max_depth: u32::MAX,
            strategies: Vec::new(),
            warm_up_plan: None,
        }
    }
}

impl SolverConfiguration {
    fn update_from_map(&mut self, opts: &HashMap<String, String>) -> anyhow::Result<()> {
        for (key, value) in opts {
            match key.as_str() {
                "optimal" => match value.to_lowercase().as_str() {
                    "true" | "t" | "1" | "yes" | "y" => self.optimal = true,
                    "false" | "f" | "0" | "no" | "n" => self.optimal = false,
                    _ => bail!("Unknown option or `optimal`: `{value}`. Options are `true` and `false`."),
                },
                "min_depth" | "min-depth" => {
                    self.min_depth = value.parse().context("Unreadable value for `min-depth`.)?")?
                }
                "max_depth" | "max-depth" => {
                    self.max_depth = value.parse().context("Unreadable value for `max-depth`.)?")?
                }
                "warm_up_plan" | "warm-up-plan" => self.warm_up_plan = Some(value.clone()),
                _ => bail!("Unknown config key: {key}"),
            }
        }
        Ok(())
    }
}

async fn solve(
    problem: Arc<up::Problem>,
    on_new_sol: impl Fn(up::Plan) + Clone + Send + 'static,
    conf: Arc<SolverConfiguration>,
) -> Result<up::PlanGenerationResult, Error> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    // run CPU-bound computation on a separate OS Thread
    std::thread::spawn(move || {
        tx.send(solve_blocking(problem, on_new_sol, conf)).unwrap();
    });
    rx.await.unwrap()
}

/// Solves the given problem, giving any intermediate solution to the callback.
/// NOTE: This function is CPU-Bound and should not be used in an async context
fn solve_blocking(
    problem: Arc<up::Problem>,
    on_new_sol: impl Fn(up::Plan) + Clone,
    conf: Arc<SolverConfiguration>,
) -> Result<up::PlanGenerationResult, Error> {
    let reception_time = Instant::now();
    let deadline = conf
        .timeout
        .map(|timeout| reception_time + std::time::Duration::from_secs_f64(timeout));

    let htn_mode = problem.hierarchy.is_some();

    let base_problem = problem_to_chronicles(&problem)
        .with_context(|| format!("In problem {}/{}", &problem.domain_name, &problem.problem_name))?;
    let bounded = htn_mode && hierarchical_is_non_recursive(&base_problem) || base_problem.templates.is_empty();

    ensure!(problem.metrics.len() <= 1, "Unsupported: multiple metrics provided.");
    let metric = if !conf.optimal {
        None
    } else if let Some(metric) = problem.metrics.first() {
        match up::metric::MetricKind::try_from(metric.kind) {
            Ok(MetricKind::MinimizeActionCosts) => Some(Metric::ActionCosts),
            Ok(MetricKind::MinimizeSequentialPlanLength) => Some(Metric::PlanLength),
            Ok(MetricKind::MinimizeMakespan) => Some(Metric::Makespan),
            Ok(MetricKind::MinimizeExpressionOnFinalState) => Some(Metric::MinimizeVar(
                base_problem
                    .context
                    .metric_final_value()
                    .context("Trying to minimize an empty expression metric.")?,
            )),
            Ok(MetricKind::MaximizeExpressionOnFinalState) => Some(Metric::MaximizeVar(
                base_problem
                    .context
                    .metric_final_value()
                    .context("Trying to maximize an empty expression metric.")?,
            )),
            _ => bail!("Unsupported metric kind with ID: {}", metric.kind),
        }
    } else {
        None
    };

    let max_depth = conf.max_depth;
    let min_depth = if bounded {
        max_depth // non recursive htn: bounded size, go directly to max
    } else {
        conf.min_depth
    };

    let warm_up_plan = plan_from_option(conf.warm_up_plan.clone(), &base_problem)?;

    // callback that will be invoked each time an intermediate solution is found
    let on_new_solution = |pb: &FiniteProblem, ass: Arc<SavedAssignment>| {
        let plan = serialize_plan(&problem, pb, &ass);
        match plan {
            Ok(plan) => on_new_sol(plan),
            Err(err) => eprintln!("Error when serializing intermediate plan: {err}"),
        }
    };
    // run solver
    let result = solver::solve(
        base_problem,
        min_depth,
        max_depth,
        &conf.strategies,
        metric,
        htn_mode,
        warm_up_plan.clone(),
        on_new_solution,
        deadline,
    )?;
    match result {
        SolverResult::Sol((finite_problem, plan)) => {
            println!(
                "************* SOLUTION FOUND **************\n\n{}",
                solver::format_plan(&finite_problem, &plan, htn_mode)?
            );
            let status = if metric.is_some() && bounded {
                up::plan_generation_result::Status::SolvedOptimally
            } else {
                up::plan_generation_result::Status::SolvedSatisficing
            };
            let plan = serialize_plan(&problem, &finite_problem, &plan)?;
            Ok(up::PlanGenerationResult {
                status: status as i32,
                plan: Some(plan),
                metrics: Default::default(),
                log_messages: vec![],
                engine: Some(aries_grpc_server::serialize::engine()),
            })
        }
        SolverResult::Unsat => {
            println!("************* NO PLAN **************");
            Ok(up::PlanGenerationResult {
                status: up::plan_generation_result::Status::UnsolvableIncompletely as i32,
                plan: None,
                metrics: Default::default(),
                log_messages: vec![],
                engine: Some(engine()),
            })
        }
        SolverResult::Timeout(opt_plan) => {
            println!("************* TIMEOUT **************");
            let opt_plan = if let Some((finite_problem, plan)) = opt_plan {
                println!("\n{}", solver::format_plan(&finite_problem, &plan, htn_mode)?);
                Some(serialize_plan(&problem, &finite_problem, &plan)?)
            } else {
                None
            };

            let status = if opt_plan.is_none() || conf.optimal {
                up::plan_generation_result::Status::Timeout
            } else {
                up::plan_generation_result::Status::SolvedSatisficing
            };

            Ok(up::PlanGenerationResult {
                status: status as i32,
                plan: opt_plan,
                metrics: Default::default(),
                log_messages: vec![],
                engine: Some(engine()),
            })
        }
    }
}
#[derive(Default)]
pub struct UnifiedPlanningService {}

#[async_trait]
impl UnifiedPlanning for UnifiedPlanningService {
    type planAnytimeStream = ReceiverStream<Result<PlanGenerationResult, Status>>;

    async fn plan_anytime(&self, request: Request<PlanRequest>) -> Result<Response<Self::planAnytimeStream>, Status> {
        let reception_time = Instant::now();
        // Channel to send the stream of results
        // Channel is given a large capacity, as we do not want the solver to block when submitting
        // intermediate solutions
        let (tx, rx) = mpsc::channel(1024);
        let plan_request = request.into_inner();

        let problem = plan_request
            .problem
            .ok_or_else(|| Status::aborted("The `problem` field is empty"))?;

        let mut conf = SolverConfiguration::default();
        conf.update_from_map(&plan_request.engine_options)
            .expect("Error in configuration");
        if plan_request.timeout != 0f64 {
            conf.timeout = Some(plan_request.timeout)
        }
        conf.optimal = true;

        let tx2 = tx.clone();

        // Callback that will be called by the solver on each plan found.
        // Note that this is called outside of tokio and should no rely on async
        let on_new_sol = move |plan: up::Plan| {
            let mut answer = up::PlanGenerationResult {
                status: up::plan_generation_result::Status::Intermediate as i32,
                plan: Some(plan),
                metrics: Default::default(),
                log_messages: vec![],
                engine: Some(aries_grpc_server::serialize::engine()),
            };
            add_engine_time(&mut answer.metrics, &reception_time);

            // send results synchronously (queue is sized to avoid blocking in practice)
            if tx2.blocking_send(Ok(answer)).is_err() {
                eprintln!("Could not send intermediate solution through the gRPC channel.");
            }
        };

        let conf = Arc::new(conf);
        let problem = Arc::new(problem);

        tokio::spawn(async move {
            let result = solve(problem.clone(), on_new_sol, conf.clone()).await;
            match result {
                Ok(mut answer) => {
                    add_engine_time(&mut answer.metrics, &reception_time);
                    tx.send(Ok(answer)).await.unwrap();
                }
                Err(e) => {
                    let message = format!("{}", e.chain().rev().format("\n    Context: "));
                    let log_message = LogMessage {
                        level: log_message::LogLevel::Error as i32,
                        message,
                    };
                    let result = PlanGenerationResult {
                        status: plan_generation_result::Status::InternalError as i32,
                        plan: None,
                        metrics: Default::default(),
                        log_messages: vec![log_message],
                        engine: Some(engine()),
                    };
                    tx.send(Ok(result)).await.unwrap();
                }
            }
        });
        // return the output channel
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn plan_one_shot(&self, request: Request<PlanRequest>) -> Result<Response<PlanGenerationResult>, Status> {
        let reception_time = Instant::now();
        let plan_request = request.into_inner();

        let problem = plan_request
            .problem
            .ok_or_else(|| Status::aborted("The `problem` field is empty"))?;

        let mut conf = SolverConfiguration::default();
        conf.update_from_map(&plan_request.engine_options)
            .expect("Error in configuration");
        if plan_request.timeout != 0f64 {
            conf.timeout = Some(plan_request.timeout)
        }

        let conf = Arc::new(conf);
        let problem = Arc::new(problem);

        let result = solve(problem, |_| {}, conf).await;
        let mut answer = result.unwrap_or_else(|e| {
            let message = format!("{}", e.chain().rev().format("\n    Context: "));
            eprintln!("ERROR: {}", &message);
            let log_message = LogMessage {
                level: log_message::LogLevel::Error as i32,
                message,
            };
            PlanGenerationResult {
                status: plan_generation_result::Status::InternalError as i32,
                plan: None,
                metrics: Default::default(),
                log_messages: vec![log_message],
                engine: Some(engine()),
            }
        });
        add_engine_time(&mut answer.metrics, &reception_time);
        Ok(Response::new(answer))
    }

    async fn validate_plan(&self, request: Request<ValidationRequest>) -> Result<Response<ValidationResult>, Status> {
        let reception_time = Instant::now();
        let validation_request = request.into_inner();

        let problem = validation_request
            .problem
            .ok_or_else(|| Status::aborted("The `problem` field is empty"))?;
        let plan = validation_request
            .plan
            .ok_or_else(|| Status::aborted("The `plan` field is empty"))?;

        let verbose: EnvParam<bool> = EnvParam::new("ARIES_VAL_VERBOSE", "false");
        let result = validate_upf(&problem, &plan, verbose.get());
        let mut answer = match result {
            Ok(_) => {
                println!("************* VALID *************");
                ValidationResult {
                    status: ValidationResultStatus::Valid.into(),
                    metrics: Default::default(),
                    log_messages: vec![],
                    engine: Some(engine()),
                }
            }
            Err(e) => {
                let message = format!("{}", e.chain().rev().format("\n    Context: "));
                println!("!!!!!!!!!!!!!! INVALID !!!!!!!!!!!!!!!");
                println!("{message}");
                let log_message = LogMessage {
                    level: log_message::LogLevel::Error as i32,
                    message,
                };
                ValidationResult {
                    status: ValidationResultStatus::Invalid.into(),
                    metrics: Default::default(),
                    log_messages: vec![log_message],
                    engine: Some(engine()),
                }
            }
        };
        add_engine_time(&mut answer.metrics, &reception_time);
        Ok(Response::new(answer))
    }

    async fn compile(
        &self,
        _request: tonic::Request<up::Problem>,
    ) -> Result<tonic::Response<up::CompilerResult>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Compilation is not supported by the Aries engine.",
        ))
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = App::parse();

    // construct a subscriber that prints formatted traces to stdout
    let subscriber = tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::Uptime::from(Instant::now()))
        .with_thread_ids(true)
        .with_max_level(args.log_level)
        .finish();
    // use that subscriber to process traces emitted after this point
    tracing::subscriber::set_global_default(subscriber)?;

    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        std::process::exit(1);
    }));

    match &args.command {
        Command::Serve(serve_args) => {
            let addr = serve_args.address.as_str().parse()?;
            let upf_service = UnifiedPlanningService::default();

            println!("Serving: {addr}");
            Server::builder()
                .add_service(UnifiedPlanningServer::new(upf_service))
                .serve(addr)
                .await?;
        }
        Command::Solve(solve_args) => {
            let problem = std::fs::read(&solve_args.problem_file)?;
            let problem = Problem::decode(problem.as_slice())?;
            let problem = Arc::new(problem);
            let conf = Arc::new(solve_args.conf.clone());

            let answer = solve(problem, |_| {}, conf).await;

            match answer {
                Ok(res) => {
                    let plan = if res.plan.is_some() { "PLAN FOUND" } else { "NO PLAN..." };
                    let status = match plan_generation_result::Status::try_from(res.status) {
                        Ok(s) => s.as_str_name(),
                        Err(_) => "???",
                    };
                    println!("{plan}   ({status})")
                }
                Err(e) => bail!(e),
            }
        }
    }

    Ok(())
}

/// Adds a measure of the time spent in the engine in a the metrics
fn add_engine_time(metrics: &mut HashMap<String, String>, start: &Instant) {
    metrics.insert(
        "engine_internal_time".to_string(),
        format!("{:.6}", start.elapsed().as_secs_f64()),
    );
}
