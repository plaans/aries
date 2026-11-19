//! Borrows: a state variable required to have a value, and made unavailable to others while it is required.
//!
//! A borrow is the encoding of a condition immediately followed by an assignment of the same value
//! (e.g. a resource required to be available and held by an action): instead of a new assignment effect, it is encoded as
//!  - a condition (an [`HasValueAt`](crate::constraints::HasValueAt) whose `borrow` field identifies the borrow),
//!    requiring the value to be supported over the whole borrow `[cond_start, end]`, and
//!  - an exclusion: no other condition on the state variable may overlap the interval `[start, end]`
//!    (`[start + ε, end]` for the conditions of the same source, that may still read the value at `start`).
//!
//! The exclusion is enforced by each condition (see [`HasValueAt`](crate::constraints::HasValueAt)),
//! as a disjunction for each borrow it may overlap.

use aries_solver::{core::views::Dom, prelude::*};

use crate::{
    StateVar, TaskId, Time,
    boxes::{BBox, BoxRef, BoxUniverse, Segment},
};

/// Interval `[start, end]` during which a state variable is borrowed: no other condition may read it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Borrow {
    pub state_var: StateVar,
    /// First instant at which other conditions on the state variable are excluded.
    pub start: Time,
    /// Last instant at which other conditions on the state variable are excluded.
    pub end: Time,
    /// Presence of the borrow.
    pub prez: Lit,
    /// Task from which the borrow originates: its own conditions are only excluded from `[start + ε, end]`.
    pub source: Option<TaskId>,
}

impl Borrow {
    /// Bounding box of the exclusion: `[lb(start), ub(end)]` and `[lb(a), ub(a)]` for each argument `a`.
    fn exclusion_box(&self, dom: &impl Dom) -> BBox {
        let mut buff = crate::boxes::Segments::new();
        buff.push(Segment::new(dom.lb(self.start), dom.ub(self.end)));
        buff.extend(self.state_var.args.iter().map(|arg| Segment::from(dom.bounds(arg))));
        BBox::new(buff)
    }
}

pub type BorrowId = usize;

#[derive(Clone, Default)]
pub struct Borrows {
    borrows: Vec<Borrow>,
    /// Exclusion box of each borrow, partitioned by fluent.
    boxes: BoxUniverse<String, BorrowId>,
}

impl Borrows {
    pub fn add(&mut self, borrow: Borrow, dom: &impl Dom) -> BorrowId {
        let id = self.borrows.len();
        self.boxes
            .add_box(&borrow.state_var.fluent, borrow.exclusion_box(dom).segments(), id);
        self.borrows.push(borrow);
        id
    }

    pub fn get(&self, id: BorrowId) -> &Borrow {
        &self.borrows[id]
    }

    pub fn iter(&self) -> impl Iterator<Item = &Borrow> + '_ {
        self.borrows.iter()
    }

    /// Borrows whose exclusion may overlap a condition, given the box of its time interval and arguments.
    pub(crate) fn potentially_overlapping<'a>(
        &'a self,
        fluent: &'a String,
        condition_box: BoxRef<'a>,
    ) -> impl Iterator<Item = BorrowId> + 'a {
        self.boxes.find_overlapping_with(fluent, condition_box).copied()
    }
}

#[cfg(test)]
mod tests {
    use crate::{constraints::HasValueAt, symbols::ObjectEncoding, *};
    use aries_solver::prelude::*;

    fn lock() -> StateVar {
        StateVar {
            fluent: "lock".to_string(),
            args: vec![],
        }
    }

    /// A problem where `lock` is true from the origin, with one task per (borrow or condition).
    struct Pb {
        sched: Sched,
    }

    impl Pb {
        fn new() -> Self {
            let mut sched = Sched::new(1, ObjectEncoding::empty(), FluentsEncoding::empty());
            let mutex_end = sched.new_timepoint();
            sched.add_effect(Effect {
                transition_start: sched.origin,
                transition_end: sched.origin,
                mutex_end,
                state_var: lock(),
                operation: EffectOp::TRUE_ASSIGNMENT,
                prez: Lit::TRUE,
                source: None,
            });
            Pb { sched }
        }

        fn task(&mut self, start: IntCst, end: IntCst) -> TaskId {
            self.sched.add_task(Task {
                name: "task".to_string(),
                start: start.into(),
                end: end.into(),
                args: vec![],
                presence: Lit::TRUE,
            })
        }

        /// Condition `[start, end] lock == true` of the given task
        fn condition(&mut self, task: TaskId, start: IntCst, end: IntCst, borrow: Option<BorrowId>) {
            self.sched.add_constraint(HasValueAt {
                state_var: lock(),
                value: IntTerm::TRUE,
                period: TimeItv {
                    start: start.into(),
                    end: end.into(),
                },
                prez: Lit::TRUE,
                source: Some(task),
                borrow,
            });
        }

        /// Borrow of the lock by the task: required on `[cond_start, ts]` and held (unavailable to others) on `]ts, te[`
        fn borrow(&mut self, task: TaskId, cond_start: IntCst, ts: IntCst, te: IntCst) {
            let id = self.sched.add_borrow(Borrow {
                state_var: lock(),
                start: ts.into(),
                end: (te - 1).into(),
                prez: Lit::TRUE,
                source: Some(task),
            });
            self.condition(task, cond_start, te - 1, Some(id));
        }

        fn solvable(&self) -> bool {
            self.sched.solve().is_some()
        }
    }

    /// A borrow by a task `[ts, te]`, and a condition of another task
    fn borrow_and_condition(ts: IntCst, te: IntCst, cond_start: IntCst, cond_end: IntCst, same_task: bool) -> bool {
        let mut pb = Pb::new();
        let a = pb.task(ts, te);
        pb.borrow(a, ts, ts, te);
        let b = if same_task { a } else { pb.task(cond_start, cond_end) };
        pb.condition(b, cond_start, cond_end, None);
        pb.solvable()
    }

    #[test]
    fn single_borrow() {
        let mut pb = Pb::new();
        let a = pb.task(2, 5);
        pb.borrow(a, 2, 2, 5);
        assert!(pb.solvable());
    }

    #[test]
    fn overlapping_borrows() {
        let check = |ts2, te2| {
            let mut pb = Pb::new();
            let a = pb.task(2, 5);
            pb.borrow(a, 2, 2, 5);
            let b = pb.task(ts2, te2);
            pb.borrow(b, ts2, ts2, te2);
            pb.solvable()
        };
        assert!(!check(3, 6));
        assert!(!check(1, 3));
        assert!(!check(2, 5));
        assert!(!check(4, 4 + 1));
        assert!(!check(1, 6));
        // back to back
        assert!(check(5, 8));
        assert!(check(0, 2));
    }

    #[test]
    fn condition_of_other_task() {
        let ok = |s, e| borrow_and_condition(2, 5, s, e, false);
        // before or after the borrow
        assert!(ok(0, 1));
        assert!(ok(5, 7));
        // at the start of the borrow (PDDL mutex) or during it
        assert!(!ok(0, 2));
        assert!(!ok(2, 2));
        assert!(!ok(3, 3));
        assert!(!ok(4, 4));
        assert!(!ok(4, 6));
        assert!(!ok(0, 8));
    }

    #[test]
    fn condition_of_same_task() {
        let ok = |s, e| borrow_and_condition(2, 5, s, e, true);
        // up to the start of the borrow (conditions are evaluated before the effect starts)
        assert!(ok(0, 2));
        assert!(ok(2, 2));
        assert!(ok(5, 5));
        // during the borrow
        assert!(!ok(3, 3));
        assert!(!ok(2, 3));
        assert!(!ok(4, 4));
    }
}
