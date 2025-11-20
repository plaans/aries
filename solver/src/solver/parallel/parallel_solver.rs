use crate::core::IntCst;
use crate::core::Lit;
use crate::model::extensions::{AssignmentExt, SavedAssignment};
use crate::model::lang::IAtom;
use crate::model::Label;
use crate::solver::parallel::signals::{InputSignal, InputStream, OutputSignal, SolverOutput, ThreadID};
use crate::solver::{Exit, Solver, UnsatCore};
use crossbeam_channel::{select, Receiver, Sender};
use itertools::Itertools;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

pub struct ParSolver<Lbl> {
    solvers: Vec<Worker<Lbl>>,
}

pub type Solution = Arc<SavedAssignment>;

#[derive(Clone)]
pub enum SolverResult<Solution> {
    /// The solver terminated with a solution.
    Sol(Solution),
    /// The solver terminated, without a finding a solution
    Unsat(Option<UnsatCore>),
    /// Teh solver was interrupted due to a timeout.
    /// It may have found a suboptimal solution.
    Timeout(Option<Solution>),
}

impl<Sol> SolverResult<Sol> {
    pub fn map<Out>(self, f: impl FnOnce(Sol) -> Out) -> SolverResult<Out> {
        match self {
            SolverResult::Sol(s) => SolverResult::Sol(f(s)),
            SolverResult::Unsat(uc) => SolverResult::Unsat(uc),
            SolverResult::Timeout(opt_sol) => SolverResult::Timeout(opt_sol.map(f)),
        }
    }
}

/// A worker is a solver that is either running on another thread or available.
/// If it is running we only have its input stream to commonuicate with it.
enum Worker<Lbl> {
    /// The solver is currently running, and can be reached through the given input stream
    Running(InputStream),
    /// The solver has been interrupted but has not stopped yet.
    Halting,
    /// The solver is idle.
    Idle(Box<Solver<Lbl>>),
}
impl<Lbl: Label> Worker<Lbl> {
    /// Marks the solver a running and return it if it wasn't previously running.
    pub fn extract(&mut self) -> Option<Box<Solver<Lbl>>> {
        let stream = match self {
            Worker::Running(_) => return None,
            Worker::Idle(solver) => solver.input_stream(),
            Worker::Halting => unimplemented!(),
        };
        let mut replace = Worker::Running(stream);
        std::mem::swap(&mut replace, self);
        if let Worker::Idle(solver) = replace {
            Some(solver)
        } else {
            None
        }
    }

    pub fn interrupt(&mut self) {
        if let Worker::Running(input) = self {
            let _ = input.sender.send(InputSignal::Interrupt);
            *self = Worker::Halting;
        }
    }
}

/// Result of running a computation with a result of type `O` on a solver.
/// The solver it self is provided as a part of the result.
struct WorkerResult<O, Lbl> {
    id: ThreadID,
    output: Result<O, Exit>,
    solver: Box<Solver<Lbl>>,
}

impl<Lbl: Label> ParSolver<Lbl> {
    /// Creates a new parallel solver.
    ///
    /// All solvers will be based on a clone of `base_solver`, on which the provided `adapt` function
    /// will be called to allow its customisation.
    pub fn new(mut base_solver: Box<Solver<Lbl>>, num_workers: usize, adapt: impl Fn(usize, &mut Solver<Lbl>)) -> Self {
        let mut solver = ParSolver {
            solvers: Vec::with_capacity(num_workers),
        };
        for i in 0..(num_workers - 1) {
            let mut s = base_solver.clone();
            adapt(i, &mut s);
            solver.solvers.push(Worker::Idle(s));
        }
        adapt(num_workers - 1, &mut base_solver);
        solver.solvers.push(Worker::Idle(base_solver));

        solver
    }

    /// Sets the output of all solvers to a particular channel and return its receiving end.
    ///
    /// Assumes that no worker is currently running.
    fn plug_solvers_output(&mut self) -> Receiver<SolverOutput> {
        let (snd, rcv) = crossbeam_channel::unbounded();
        for x in &mut self.solvers {
            if let Worker::Idle(solver) = x {
                solver.set_solver_output(snd.clone());
            } else {
                panic!("A worker is not available")
            }
        }
        rcv
    }

    pub fn incremental_push_all(&mut self, assumptions: Vec<Lit>) -> Result<(), UnsatCore> {
        let mut res: Result<(), UnsatCore> = Ok(());
        for s in self.solvers.iter_mut() {
            match s {
                Worker::Running(_) => panic!(),
                Worker::Halting => panic!(),
                Worker::Idle(s) => {
                    if let Err((_, unsat_core)) = s.incremental_push_all(assumptions.clone()) {
                        if let Err(ref uc) = res {
                            if unsat_core.literals().len() < uc.literals().len() {
                                res = Err(unsat_core);
                            }
                        } else {
                            res = Err(unsat_core);
                        }
                    }
                }
            }
        }
        res
    }

    /// Solve the problem that was given on initialization, using all available solvers.
    ///
    /// In case of unsatisfiability, will return an unsat core of
    /// the assumptions that were initially pushed to `base_solver`.
    pub fn incremental_solve(&mut self, deadline: Option<Instant>) -> SolverResult<Solution> {
        debug_assert!(
            self.solvers
                .iter()
                .map(|s| match s {
                    Worker::Running(_) => panic!(),
                    Worker::Halting => panic!(),
                    Worker::Idle(s) => s.model.state.assumptions(),
                })
                .all_equal(),
            "Workers need to have the same assumptions pushed into them",
        );
        self.race_solvers(
            |s| s.incremental_solve().map(|res| res.map_err(|uc: UnsatCore| Some(uc))),
            |_| {},
            deadline,
        )
    }

    pub fn solve_with_assumptions(
        &mut self,
        assumptions: Vec<Lit>,
        deadline: Option<Instant>,
    ) -> SolverResult<Solution> {
        let run = move |s: &mut Solver<Lbl>| {
            s.solve_with_assumptions(assumptions.iter().as_slice())
                .map(|res| res.map_err(|uc: UnsatCore| Some(uc)))
        };
        self.race_solvers(run, |_| {}, deadline)
    }

    /// Solve the problem that was given on initialization using all available solvers.
    pub fn solve(&mut self, deadline: Option<Instant>) -> SolverResult<Solution> {
        self.race_solvers(|s| s.solve().map(|res| res.ok_or(None)), |_| {}, deadline)
    }

    /// Minimize the value of the given expression.
    pub fn minimize(&mut self, objective: impl Into<IAtom>, deadline: Option<Instant>) -> SolverResult<Solution> {
        let objective = objective.into();
        self.race_solvers(
            move |s| match s.minimize(objective) {
                Ok(Some((_cost, sol))) => Ok(Ok(sol)),
                Ok(None) => Ok(Err(None)),
                Err(x) => Err(x),
            },
            |_| {},
            deadline,
        )
    }

    /// Minimize the value of the given expression.
    /// Each time a new solution is found with an improved objective value, the corresponding
    /// assignment will be passed to the given callback.
    pub fn minimize_with(
        &mut self,
        objective: impl Into<IAtom>,
        on_improved_solution: impl Fn(Solution),
        initial_solution: Option<(IntCst, Solution)>,
        deadline: Option<Instant>,
    ) -> SolverResult<Solution> {
        let objective = objective.into();
        // cost of the best solution found so far
        let mut previous_best = None;

        // callback that checks if a new solution is a strict improvement over the previous one
        // and if that the case, invokes the user-provided callback
        let on_new_sol = |ass: Solution| {
            let obj_value = ass.var_domain(objective).lb;
            let is_improvement = match previous_best {
                Some(prev) => prev > obj_value,
                None => true,
            };
            if is_improvement {
                on_improved_solution(ass);
                previous_best = Some(obj_value)
            }
        };
        self.race_solvers(
            move |s| match s.minimize_with_optional_initial_solution(objective, initial_solution.clone()) {
                Ok(Some((_cost, sol))) => Ok(Ok(sol)),
                Ok(None) => Ok(Err(None)),
                Err(x) => Err(x),
            },
            on_new_sol,
            deadline,
        )
    }

    /// Generic function to run a lambda in parallel on all available solvers and return the result of the
    /// first finishing one.
    ///
    /// This function also setups inter-solver communication to enable clause/solution sharing.
    /// Once a first result is found, it sends an interruption message to all other workers and wait for them to yield.
    fn race_solvers<F, G>(&mut self, run: F, mut on_new_sol: G, deadline: Option<Instant>) -> SolverResult<Solution>
    where
        F: Fn(&mut Solver<Lbl>) -> Result<Result<Solution, Option<UnsatCore>>, Exit> + Send + 'static + Clone,
        G: FnMut(Solution),
    {
        // a receiver that will collect all intermediates results (incumbent solution and learned clauses)
        // from the solvers
        let solvers_output = self.plug_solvers_output();

        // channel that is used to get the final results of the solvers.
        let (result_snd, result_rcv) = crossbeam_channel::unbounded();

        // lambda used to start a thread and run a solver on it.
        let spawn = |id: usize,
                     mut solver: Box<Solver<Lbl>>,
                     result_snd: Sender<WorkerResult<Result<Solution, Option<UnsatCore>>, Lbl>>| {
            thread::spawn(move || {
                let output = run(&mut solver);
                let answer = WorkerResult { id, output, solver };
                // ignore message delivery failures (on another solver might have found the solution earlier)
                let _ = result_snd.send(answer);
            });
        };

        let mut solvers_inputs = Vec::with_capacity(self.solvers.len());

        // start all solvers
        for (i, worker) in self.solvers.iter_mut().enumerate() {
            let solver = worker.extract().expect("A solver is already busy");
            solvers_inputs.push(solver.input_stream());
            spawn.clone()(i, solver, result_snd.clone());
        }

        let mut status = SolverStatus::Pending;

        while self.is_worker_running() {
            let time_left = if let Some(deadline) = deadline {
                deadline - Instant::now()
            } else {
                Duration::MAX
            };
            select! {
                recv(result_rcv) -> res => { // solver termination
                    let WorkerResult {
                        id: worker_id,
                        output: result,
                        solver,
                    } = res.unwrap();
                    self.solvers[worker_id] = Worker::Idle(solver);

                    if !matches!(status, SolverStatus::Final(_)) {
                        // this is the first result we got, store it and stop other solvers
                        let result = match result {
                            Ok(Ok(sol)) => SolverResult::Sol(sol),
                            Ok(Err(uc)) => SolverResult::Unsat(uc),
                            Err(_) => {
                                eprintln!("Unexpected interruption of solver.");
                                continue
                            }
                        };
                        status = SolverStatus::Final(result);
                        for s in &mut self.solvers {
                            s.interrupt()
                        }
                    }
                }
                recv(solvers_output) -> msg => { // solver intermediate result
                    if let Ok(msg) = msg {
                        self.share_among_solvers(&msg);
                        if !matches!(status, SolverStatus::Final(_)) {
                            if let OutputSignal::SolutionFound(assignment) = msg.msg {
                                on_new_sol(assignment.clone());
                                status = SolverStatus::Intermediate(assignment);
                            }
                        }
                    }
                }
                default(time_left) => { // timeout
                    for s in &mut self.solvers {
                        // notify all threads that they should stop ASAP
                        s.interrupt()
                    }
                    let result = match status {
                        SolverStatus::Pending => SolverResult::Timeout(None),
                        SolverStatus::Intermediate(sol) => SolverResult::Timeout(Some(sol)),
                        SolverStatus::Final(result) => result,
                    };
                    status = SolverStatus::Final(result);

                }
            }
        }

        match status {
            SolverStatus::Final(res) => res,
            _ => unreachable!(),
        }
    }

    /// Returns true if there is at least one worker that is currently running.
    fn is_worker_running(&self) -> bool {
        self.solvers.iter().any(|solver| matches!(&solver, Worker::Running(_)))
    }

    /// Share an intermediate result with other running solvers that might be interested.
    fn share_among_solvers(&self, signal: &SolverOutput) {
        // resend message to all other solvers. Note that a solver might have exited already
        // and thus would not be able to receive the message
        for solver in &self.solvers {
            match solver {
                Worker::Running(input) if input.id != signal.emitter => match &signal.msg {
                    OutputSignal::LearntClause(cl) => {
                        let _ = input.sender.send(InputSignal::LearnedClause(cl.clone()));
                    }
                    OutputSignal::SolutionFound(assignment) => {
                        let _ = input.sender.send(InputSignal::SolutionFound(assignment.clone()));
                    }
                },
                _ => { /* Solver is not running or is the emitter, ignore */ }
            }
        }
    }

    /// Prints the statistics of all solvers.
    pub fn print_stats(&self) {
        for (id, solver) in self.solvers.iter().enumerate() {
            println!("\n==== Worker {}", id + 1);
            if let Worker::Idle(solver) = solver {
                solver.print_stats()
            } else {
                println!("Solver is running");
            }
        }
    }
}

enum SolverStatus<Sol> {
    /// Still waiting for a final result.
    Pending,
    Intermediate(Sol),
    /// A final result was provided by at least one solver.
    Final(SolverResult<Sol>),
}
