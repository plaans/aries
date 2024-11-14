use crate::core::state::Domains;
use std::cell::RefCell;

use crate::core::literals::Disjunction;

thread_local! {
    /// Represent a valid solution to the problem attempted on the current thread
    static SOLUTION_WITNESS: RefCell<Option<Domains>> = const { RefCell::new(None) };
}

/// Records a solution witness for the current thread
pub fn set_solution_witness(sol: &Domains) {
    SOLUTION_WITNESS.with(|w| *w.borrow_mut() = Some(sol.clone()));
}

/// Remove the solution witness for the current thread
pub fn remove_solution_witness() {
    SOLUTION_WITNESS.with(|w| *w.borrow_mut() = None);
}

pub fn on_drop_witness_cleaner() -> WitnessCleaner {
    WitnessCleaner
}

pub struct WitnessCleaner;
impl Drop for WitnessCleaner {
    fn drop(&mut self) {
        remove_solution_witness()
    }
}

/// Returns true if the witness solution was to be pruned by the clause
pub fn pruned_by_clause(clause: &Disjunction) -> bool {
    SOLUTION_WITNESS.with(|witness| {
        if let Some(sol) = witness.borrow().as_ref() {
            for l in clause {
                if sol.entails(l) {
                    return false;
                }
            }
            true
        } else {
            false
        }
    })
}
