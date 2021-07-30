use aries_model::assignments::SavedAssignment;
use aries_model::bounds::Disjunction;
use aries_model::lang::{IAtom, IntCst};
use env_param::EnvParam;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

/// The maximum size of a clause that can be shared with other threads.
static MAX_CLAUSE_SHARING_SIZE: EnvParam<usize> = EnvParam::new("ARIES_MAX_CLAUSE_SHARING_SIZE", "6");

static THREAD_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);
pub type ThreadID = usize;
fn get_next_thread_id() -> ThreadID {
    THREAD_ID_COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Signals that can be received by a Solver.
pub enum InputSignal {
    /// This solver should stop ASAP.
    Interrupt,
    /// A clause was learned in another solver.
    LearnedClause(Arc<Disjunction>),
    /// A solution was found in another solver.
    SolutionFound {
        /// Objective expression
        objective: IAtom,
        /// Value of the objective for the assignment
        objective_value: IntCst,
        /// Variable assignment of the solution
        assignment: Arc<SavedAssignment>,
    },
}

pub struct InputStream {
    /// A unique ID for this solver.
    pub id: ThreadID,
    pub sender: Sender<InputSignal>,
}

pub struct SolverOutput {
    pub emitter: ThreadID,
    pub msg: OutputSignal,
}

pub enum OutputSignal {
    /// Represents a clause that has been inferred by the solver
    LearntClause(Arc<Disjunction>),
    /// An intermediate solution was found, typical a solution that is valid but was not proven optimal yet.
    SolutionFound {
        objective: IAtom,
        objective_value: IntCst,
        assignment: Arc<SavedAssignment>,
    },
}

/// A structure that holds the various components to communicate to a solver.
pub struct Synchro {
    pub id: ThreadID,
    /// The sender end of the socket. Should be given to anybody that wants to communicate with the solver.
    pub sender: Sender<InputSignal>,
    /// The receiver end of the sockets. Own by a solver (and not shared with anybody else.
    pub signals: Receiver<InputSignal>,

    /// A channel where a solver's output can be sent (typically for learnt clauses or intermediate solutions).
    pub output: Option<Sender<SolverOutput>>,
}

impl Synchro {
    pub fn new() -> Self {
        let (snd, rcv) = std::sync::mpsc::channel();
        Synchro {
            id: get_next_thread_id(),
            sender: snd,
            signals: rcv,
            output: None,
        }
    }

    pub fn set_output(&mut self, out: Sender<SolverOutput>) {
        self.output = Some(out)
    }

    pub fn input_stream(&self) -> InputStream {
        InputStream {
            id: self.id,
            sender: self.sender.clone(),
        }
    }

    pub fn notify_learnt(&self, clause: &Disjunction) {
        if let Some(output) = &self.output {
            let len = clause.len();
            if len > 0 && len <= MAX_CLAUSE_SHARING_SIZE.get() {
                let msg = OutputSignal::LearntClause(Arc::new(Disjunction::from(clause)));
                output.send(SolverOutput { emitter: self.id, msg }).unwrap()
            }
        }
    }

    pub fn notify_solution_found(&self, objective: IAtom, objective_value: IntCst, assignment: Arc<SavedAssignment>) {
        if let Some(output) = &self.output {
            let msg = OutputSignal::SolutionFound {
                objective,
                objective_value,
                assignment,
            };
            output.send(SolverOutput { emitter: self.id, msg }).unwrap()
        }
    }
}

impl Clone for Synchro {
    fn clone(&self) -> Self {
        let mut res = Self::new();
        if let Some(out) = &self.output {
            res.output = Some(out.clone())
        }
        res
    }
}

impl Default for Synchro {
    fn default() -> Self {
        Self::new()
    }
}
