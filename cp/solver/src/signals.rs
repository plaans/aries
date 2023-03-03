use aries::core::literals::Disjunction;
use aries::model::extensions::SavedAssignment;
use crossbeam_channel::{Receiver, Sender};
use env_param::EnvParam;
use std::fmt::{Debug, Formatter};
use std::sync::atomic::{AtomicUsize, Ordering};
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
    SolutionFound(Arc<SavedAssignment>),
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
impl Debug for SolverOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} ", self.emitter)?;
        match &self.msg {
            OutputSignal::LearntClause(cl) => {
                write!(f, "clause {cl:?}")
            }
            OutputSignal::SolutionFound(_) => {
                write!(f, "solution")
            }
        }
    }
}

pub enum OutputSignal {
    /// Represents a clause that has been inferred by the solver
    LearntClause(Arc<Disjunction>),
    /// An intermediate solution was found, typically a solution that is valid but was not proven optimal yet.
    SolutionFound(Arc<SavedAssignment>),
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
        let (snd, rcv) = crossbeam_channel::unbounded();
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

    /// Notify listeners that a a new clause was learnt.
    ///
    /// Heuristics are applied to determine whether this clause is worth sharing,
    /// typically based on its size.
    pub fn notify_learnt(&self, clause: &Disjunction) {
        if let Some(output) = &self.output {
            let len = clause.len();
            if len > 0 && len <= MAX_CLAUSE_SHARING_SIZE.get() {
                let msg = OutputSignal::LearntClause(Arc::new(Disjunction::from(clause)));
                // ignore errors as the thread might just be running alone in the ether
                let _ = output.send(SolverOutput { emitter: self.id, msg });
            }
        }
    }

    /// Notify listeners that a new solution was found.
    pub fn notify_solution_found(&self, assignment: Arc<SavedAssignment>) {
        if let Some(output) = &self.output {
            let msg = OutputSignal::SolutionFound(assignment);
            // ignore errors as the thread might just be running alone in the ether
            let _ = output.send(SolverOutput { emitter: self.id, msg });
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
