use aries::core::views::Term;
use aries::prelude::*;
use aries::utils::StreamingIterator;
use idmap::intid::IntegerId;
use itertools::Itertools;

use crate::TaskId;
use crate::encoder::SchedEncoder;
use crate::transitions::{Transition, Transitions, find_empty_source_linterms};

fn points_compatible(points1: &[IntCst], points2: &[IntCst], index_mapping: &[usize]) -> bool {
    index_mapping
        .iter()
        .filter_map(|&i| Some((points1[i], points2.get(i)?)))
        .all_equal()
}
fn get_linterm_dim(linterm: &LinTerm, ctx: &SchedEncoder) -> usize {
    let (lb, ub) = ctx.store.bounds(linterm.variable());
    usize::try_from(ub - lb + 1).unwrap()
}

pub type LinTermGroundingIndex = usize;
pub type LinTermGroundingId = usize;

pub struct LinTermGrounding {
    pub linterm: LinTerm,
    pub assignment: IntCst,
    pub id: LinTermGroundingId,
}

pub type TransitionGroundingIndex = Vec<usize>;
pub type TransitionGroundingId = usize;

pub struct TransitionGrounding {
    pub transition: Transition,
    pub assignment: Vec<IntCst>,
    pub id: TransitionGroundingId,
    pub(crate) indices: TransitionGroundingIndex,
    pub(crate) dims: Vec<usize>,
    pub(crate) positions_in_source: Vec<usize>,
}

impl TransitionGrounding {
    pub fn to_linterm_groundings(&self, ctx: &SchedEncoder) -> Vec<LinTermGrounding> {
        self.transition
            .get_args_and_vals(ctx)
            .into_iter()
            .enumerate()
            .map(|(i, linterm)| LinTermGrounding {
                linterm,
                assignment: self.assignment[i],
                id: self.indices[i],
            })
            .collect_vec()
    }
    pub fn flat_index(&self) -> usize {
        let mut res = 0;
        let mut factor = 1;
        for (&i, &d) in self.indices.iter().zip(self.dims.iter()).rev() {
            res += i * factor;
            factor *= d;
        }
        res
    }

    fn default(
        transition: Transition,
        source: Option<TaskId>,
        ctx: &SchedEncoder,
        ctx_empty_source_linterms: &[LinTerm],
    ) -> Self {
        let args_and_vals = transition.get_args_and_vals(ctx);

        let assignment = args_and_vals
            .iter()
            .map(|linterm| linterm.ith_value(0, &ctx.store).unwrap())
            .collect_vec();
        let indices = vec![0; args_and_vals.arity()];
        let dims = args_and_vals
            .iter()
            .map(|linterm| get_linterm_dim(linterm, ctx))
            .collect_vec();
        let positions_in_source = {
            let (args_pos, fromval_pos, toval_pos) =
                args_and_vals.find_positions_in_source(source, ctx, ctx_empty_source_linterms);
            let mut res = args_pos;
            if let Some(fromval_pos) = fromval_pos {
                res.push(fromval_pos);
            }
            if let Some(toval_pos) = toval_pos {
                res.push(toval_pos);
            }
            res
        };

        Self {
            transition,
            assignment,
            id: 0,
            indices,
            dims,
            positions_in_source,
        }
    }
    fn ith_linterm(&self, i: usize, ctx: &SchedEncoder) -> Option<LinTerm> {
        let args_and_vals = self.transition.get_args_and_vals(ctx);
        if i < args_and_vals.args().len() {
            Some(args_and_vals.args()[i])
        } else if i == args_and_vals.args().len() && args_and_vals.valfrom().is_some() {
            args_and_vals.valfrom()
        } else if (i == args_and_vals.args().len() && args_and_vals.valto().is_some())
            || i == args_and_vals.args().len() + 1
        {
            args_and_vals.valto()
        } else {
            None
        }
    }
    fn next(&mut self, ctx: &SchedEncoder) -> Result<(), ()> {
        let mut i = self.indices.len() - 1;
        loop {
            if self.indices[i] == self.dims[i] - 1 {
                if i == 0 {
                    return Err(());
                }
                self.indices[i] = 0;
                self.assignment[i] = self.ith_linterm(i, ctx).unwrap().ith_value(0, &ctx.store).unwrap();

                i -= 1;
            } else {
                self.indices[i] += 1;
                self.assignment[i] = self
                    .ith_linterm(i, ctx)
                    .unwrap()
                    .ith_value(self.indices[i], &ctx.store)
                    .unwrap();

                return Ok(());
            }
        }
    }
}

pub type SourceGroundingIndex = Vec<usize>;
pub type SourceGroundingId = usize;

pub struct SourceGrounding {
    pub source: Option<TaskId>,
    pub assignment: Vec<IntCst>,
    pub id: SourceGroundingId,
    pub(crate) indices: SourceGroundingIndex,
    pub(crate) dims: Vec<usize>,
}
impl SourceGrounding {
    fn default(source: Option<TaskId>, ctx: &SchedEncoder, ctx_empty_source_linterms: &Vec<LinTerm>) -> Self {
        let assignment = if let Some(task_id) = source {
            ctx.sched.tasks[task_id]
                .args
                .iter()
                .map(|linterm| linterm.ith_value(0, &ctx.store).unwrap())
                .collect_vec()
        } else {
            ctx_empty_source_linterms
                .iter()
                .map(|linterm| linterm.ith_value(0, &ctx.store).unwrap())
                .collect_vec()
        };
        let indices = if let Some(task_id) = source {
            vec![0; ctx.sched.tasks[task_id].args.len()]
        } else {
            vec![0; ctx_empty_source_linterms.len()]
        };
        let dims = {
            if let Some(task_id) = source {
                &ctx.sched.tasks[task_id].args
            } else {
                ctx_empty_source_linterms
            }
            .iter()
            .map(|linterm| get_linterm_dim(linterm, ctx))
            .collect_vec()
        };

        Self {
            source,
            assignment,
            id: 0,
            indices,
            dims,
        }
    }
    fn ith_linterm(&self, i: usize, ctx: &SchedEncoder, ctx_empty_source_linterms: &[LinTerm]) -> Option<LinTerm> {
        if let Some(task_id) = self.source {
            ctx.sched.tasks[task_id].args.get(i).copied()
        } else {
            ctx_empty_source_linterms.get(i).copied()
        }
    }
    fn next(&mut self, ctx: &SchedEncoder, ctx_empty_source_linterms: &[LinTerm]) -> Result<(), ()> {
        let mut i = self.indices.len() - 1;
        loop {
            if self.indices[i] == self.dims[i] - 1 {
                if i == 0 {
                    return Err(());
                }
                self.indices[i] = 0;
                self.assignment[i] = self
                    .ith_linterm(i, ctx, ctx_empty_source_linterms)
                    .unwrap()
                    .ith_value(0, &ctx.store)
                    .unwrap();

                i -= 1;
            } else {
                self.indices[i] += 1;
                self.assignment[i] = self
                    .ith_linterm(i, ctx, ctx_empty_source_linterms)
                    .unwrap()
                    .ith_value(self.indices[i], &ctx.store)
                    .unwrap();

                return Ok(());
            }
        }
    }
}

pub struct TransitionsGroundingsEnumerator<'a> {
    transitions: &'a Transitions,
    ctx: &'a SchedEncoder,
    ctx_empty_source_linterms: Vec<LinTerm>,

    current: (TransitionGrounding, SourceGrounding),
    current_transition_index_in_source: usize,

    is_started: bool,
    is_finished: bool,
}
impl<'a> TransitionsGroundingsEnumerator<'a> {
    pub fn new(transitions: &'a Transitions, ctx: &'a SchedEncoder) -> Result<Self, ()> {
        let ctx_empty_source_linterms = Vec::from_iter(find_empty_source_linterms(ctx));

        let transition = *transitions
            .store
            .get(*transitions.of_empty_source.first().ok_or(())?)
            .ok_or(())?;

        let current = (
            TransitionGrounding::default(transition, None, ctx, &ctx_empty_source_linterms),
            SourceGrounding::default(None, ctx, &ctx_empty_source_linterms),
        );

        Ok(Self {
            transitions,
            ctx,
            ctx_empty_source_linterms,
            current,
            current_transition_index_in_source: 0,
            is_started: true,
            is_finished: false,
        })
    }

    fn renew_current_transition_grounding(&mut self) {
        let transition = self.transitions.store
            [self.transitions.of_source(&self.current.1.source)[self.current_transition_index_in_source]];
        self.current.0 = TransitionGrounding::default(
            transition,
            self.current.1.source,
            self.ctx,
            &self.ctx_empty_source_linterms,
        );
    }
    fn renew_current_source_grounding(&mut self) {
        let source = if let Some(task_id) = self.current.1.source {
            Some(TaskId::from_int(1 + task_id.to_int()))
        } else {
            Some(TaskId::from_int(0))
        };
        self.current.1 = SourceGrounding::default(source, self.ctx, &self.ctx_empty_source_linterms);
    }
    fn is_compatible(&self) -> bool {
        points_compatible(
            &self.current.0.assignment,
            &self.current.1.assignment,
            &self.current.0.positions_in_source,
        )
        // TODO: also check whether grounding is trivially impossible
    }
}

impl<'a> StreamingIterator for TransitionsGroundingsEnumerator<'a> {
    type Item = (TransitionGrounding, SourceGrounding);

    fn advance(&mut self) {
        if self.is_finished {
            return;
        }
        if !self.is_started {
            self.is_started = true;
            if self.is_compatible() {
                debug_assert!(self.current.0.id == 0 && self.current.1.id == 0);
                return;
            }
        }
        loop {
            if self.current.0.next(self.ctx).is_err() {
                self.current_transition_index_in_source += 1;

                if self.current_transition_index_in_source < self.transitions.of_source(&self.current.1.source).len()
                    || self.current.1.next(self.ctx, &self.ctx_empty_source_linterms).is_ok()
                {
                    self.renew_current_transition_grounding();
                } else if self.current.1.source.map_or(0, |task_id| 1 + task_id.to_int() as usize)
                    < self.transitions.of_concrete_source.len()
                {
                    self.current_transition_index_in_source = 0;

                    self.renew_current_source_grounding();
                    self.renew_current_transition_grounding();
                } else {
                    self.is_finished = true;
                    return;
                }
            }
            if self.is_compatible() {
                self.current.0.id += 1;
                self.current.1.id += 1;
                return;
            }
        }
    }

    fn get(&self) -> Option<&Self::Item> {
        if self.is_finished || !self.is_started {
            None
        } else {
            Some(&self.current)
        }
    }
}
