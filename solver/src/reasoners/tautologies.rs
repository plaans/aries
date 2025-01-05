use crate::backtrack::{Backtrack, DecLvl};
use crate::core::state::{Cause, Domains, DomainsSnapshot, Explanation, InferenceCause};
use crate::core::Lit;
use crate::reasoners::{Contradiction, ReasonerId, Theory};
use crate::utils::SnapshotStatistics;

/// A reasoner that holds a set of tautologies (single literals that are known to be true)
/// and propagates them at every decision level.
///
/// The purpose of this reasoner is to avoid backtracking at the root level to impose a universal fact.
/// This is in particular the case when optimizing, that leads to frequent additions of tautology
/// facts to force improvement on the optimized variable.
#[derive(Clone, Default)]
pub struct Tautologies {
    /// list of literals that are always true
    tautologies: Vec<Lit>,
    lvl: crate::backtrack::DecisionLevelTracker,
}

impl Tautologies {
    pub fn add_tautology(&mut self, lit: Lit) {
        self.tautologies.push(lit)
    }
}

impl Backtrack for Tautologies {
    fn save_state(&mut self) -> DecLvl {
        self.lvl.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.lvl.num_saved()
    }

    fn restore_last(&mut self) {
        self.lvl.restore_last()
    }
}

impl Theory for Tautologies {
    fn identity(&self) -> ReasonerId {
        ReasonerId::Tautologies
    }

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        // dummy cause, as
        let cause = Cause::Inference(InferenceCause {
            writer: self.identity(),
            payload: 0,
        });
        for l in self.tautologies.iter().rev() {
            // iterate backwards as latests are likely stronger
            model.set(*l, cause)?;
        }
        if self.current_decision_level() == DecLvl::ROOT {
            // these have been propagated and can never be undone
            self.tautologies.clear()
        }
        Ok(())
    }

    fn explain(
        &mut self,
        _: Lit,
        context: InferenceCause,
        _domains: &DomainsSnapshot,
        _out_explanation: &mut Explanation,
    ) {
        debug_assert_eq!(context.writer, self.identity());
        debug_assert_eq!(context.payload, 0);
        // Nothing to explain as the literal is in principle entailed at the ROOT.
        // This propagator only enforces it systematically to avoid restarts.
    }

    fn print_stats(&self) {}

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Clone)]
pub struct TautologiesStatSnapshot;
impl std::fmt::Display for TautologiesStatSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", std::any::type_name::<Self>())
    }
}


impl SnapshotStatistics for Tautologies {
    type Stats = TautologiesStatSnapshot;

    fn snapshot_statistics(&self) -> Self::Stats {
        TautologiesStatSnapshot
    }
}
