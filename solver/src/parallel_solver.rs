use crate::signals::{InputSignal, InputStream, OutputSignal, SolverOutput, ThreadID};
use crate::solver::{Exit, Solver};
use aries_model::extensions::SavedAssignment;
use aries_model::lang::{IAtom, IntCst};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;

pub struct ParSolver {
    solvers: Vec<Worker>,
}

/// A worker is a solver that is either running on another thread or available.
/// If it is running we only have its input stream to commonuicate with it.
enum Worker {
    Running(InputStream),
    Idle(Box<Solver>),
}
impl Worker {
    /// Marks the solver a running and return it if it wasn't previously running.
    pub fn extract(&mut self) -> Option<Box<Solver>> {
        let stream = match self {
            Worker::Running(_) => return None,
            Worker::Idle(solver) => solver.input_stream(),
        };
        let mut replace = Worker::Running(stream);
        std::mem::swap(&mut replace, self);
        if let Worker::Idle(solver) = replace {
            Some(solver)
        } else {
            None
        }
    }
}

/// Result of running a computation with a result of type `O` on a solver.
/// The solver it self is provided as a part of the result.
struct WorkerResult<O> {
    id: ThreadID,
    output: Result<O, Exit>,
    solver: Box<Solver>,
}

impl ParSolver {
    /// Creates a new parallel solver.
    ///
    /// All solvers will be based on a clone of `base_solver`, on which the provided `adapt` function
    /// will be called to allow its customisation.
    pub fn new(mut base_solver: Box<Solver>, num_workers: usize, adapt: impl Fn(usize, &mut Solver)) -> Self {
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
    fn plug_solvers_output(&mut self) -> Receiver<SolverOutput> {
        let (snd, rcv) = std::sync::mpsc::channel();
        for x in &mut self.solvers {
            if let Worker::Idle(solver) = x {
                solver.set_solver_output(snd.clone());
            } else {
                panic!("A worker is not available")
            }
        }
        rcv
    }

    /// Solve the problem that was given on initialization using all available solvers.
    pub fn solve(&mut self) -> Result<Option<Arc<SavedAssignment>>, Exit> {
        self.race_solvers(|s| s.solve())
    }

    /// Minimize the value of the given expression.
    pub fn minimize(&mut self, objective: impl Into<IAtom>) -> Result<Option<(IntCst, Arc<SavedAssignment>)>, Exit> {
        let objective = objective.into();
        self.race_solvers(move |s| s.minimize(objective))
    }

    /// Generic function to run a lambda in parallel on all available solvers and return the result of the
    /// first finishing one.
    ///
    /// This function also setups inter-solver communication to enable clause/solution sharing.
    /// Once a first result is found, it sends an interruption message to all other workers and wait for them to yield.
    fn race_solvers<O, F>(&mut self, run: F) -> Result<O, Exit>
    where
        O: Send + 'static,
        F: Fn(&mut Solver) -> Result<O, Exit> + Send + 'static + Copy,
    {
        let solvers_output = self.plug_solvers_output();
        let (result_snd, result_rcv) = channel();

        // lambda used to start a thread and run a solver on it.
        let spawn = |id: usize, mut solver: Box<Solver>, result_snd: Sender<WorkerResult<O>>| {
            thread::spawn(move || {
                let output = run(&mut solver);
                let answer = WorkerResult { id, output, solver };
                result_snd.send(answer).expect("Error while sending message");
            });
        };

        let mut solvers_inputs = Vec::with_capacity(self.solvers.len());

        // start all solvers
        for (i, worker) in self.solvers.iter_mut().enumerate() {
            let solver = worker.extract().expect("A solver is already busy");
            solvers_inputs.push(solver.input_stream());
            spawn(i, solver, result_snd.clone());
        }

        // start a new thread whose role is to send learnt clauses to other solvers
        thread::spawn(move || {
            while let Ok(x) = solvers_output.recv() {
                // resend message to all other solvers. Note that a solver might have exited already
                // and thus would not be able to receive the message
                match x.msg {
                    OutputSignal::LearntClause(cl) => {
                        for input in &solvers_inputs {
                            if input.id != x.emitter {
                                let _ = input.sender.send(InputSignal::LearnedClause(cl.clone()));
                            }
                        }
                    }
                    OutputSignal::SolutionFound(assignment) => {
                        for input in &solvers_inputs {
                            if input.id != x.emitter {
                                let _ = input.sender.send(InputSignal::SolutionFound(assignment.clone()));
                            }
                        }
                    }
                }
            }
        });

        let WorkerResult {
            id: first_id,
            output: first_result,
            solver,
        } = result_rcv.recv().unwrap();
        self.solvers[first_id] = Worker::Idle(solver);

        for s in &self.solvers {
            if let Worker::Running(input) = s {
                input.sender.send(InputSignal::Interrupt).unwrap();
            }
        }

        for _ in 0..(self.solvers.len() - 1) {
            let result = result_rcv.recv().unwrap();
            self.solvers[result.id] = Worker::Idle(result.solver);
        }

        first_result
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
