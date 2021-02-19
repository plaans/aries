use crossbeam_channel::Receiver;

/// Signals that can be received by a Solver.
pub enum Signal {
    Interrupt,
}

pub struct Synchro {
    pub signals: Receiver<Signal>,
}

impl Default for Synchro {
    fn default() -> Self {
        Synchro {
            signals: crossbeam_channel::never(),
        }
    }
}
