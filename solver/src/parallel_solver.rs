use crate::signals::{InputSignal, InputStream, OutputSignal, SolverOutput};
use crate::solver::{Exit, Solver};
use aries_model::assignments::SavedAssignment;
use std::sync::mpsc::{channel, Receiver, Sender};
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

    pub fn get_solver(&self) -> Option<&Solver> {
        match self {
            Worker::Running(_) => None,
            Worker::Idle(solver) => Some(solver.as_ref()),
        }
    }
}

struct WorkerResult {
    id: usize,
    output: Result<bool, Exit>,
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
    pub fn solve(&mut self) -> Result<Option<&SavedAssignment>, Exit> {
        let solvers_output = self.plug_solvers_output();
        let (result_snd, result_rcv) = channel();

        let spawn = |id: usize, mut solver: Box<Solver>, result_snd: Sender<WorkerResult>| {
            thread::spawn(move || {
                let output = solver.solve();
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
                match x.msg {
                    OutputSignal::LearntClause(cl) => {
                        for input in &solvers_inputs {
                            if input.id != x.emitter {
                                input.sender.send(InputSignal::LearnedClause(cl.clone())).unwrap()
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

        match first_result {
            Ok(true) => Ok(Some(&self.solvers[first_id].get_solver().unwrap().model)),
            Ok(false) => Ok(None),
            Err(x) => Err(x),
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
