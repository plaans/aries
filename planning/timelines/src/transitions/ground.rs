use aries::core::views::Term;
use aries::prelude::*;
use aries::utils::StreamingIterator;

use itertools::Itertools;

use crate::TaskId;
use crate::encoder::SchedEncoder;
use crate::transitions::{Transition, TransitionId, Transitions, find_empty_source_linterms};

fn points_compatible(points1: &[IntCst], points2: &[IntCst], index_mapping: &[Option<usize>]) -> bool {
    index_mapping
        .iter()
        .enumerate()
        .filter_map(|(i, &mapped_i)| mapped_i.map(|mapped_i| (i, mapped_i)))
        .all(|(i, mapped_i)| points1[i] == points2[mapped_i])
}
fn get_linterm_dim(linterm: &LinTerm, ctx: &SchedEncoder) -> usize {
    let (lb, ub) = ctx.store.bounds(linterm.variable());
    usize::try_from(ub - lb + 1).unwrap()
}

pub type TransitionGroundingId = Vec<usize>;
#[derive(Debug)]
pub struct TransitionGrounding {
    pub transition_id: TransitionId,
    pub transition: Transition,
    pub assignment: Vec<IntCst>,
    indices: TransitionGroundingId,
    dims: Vec<usize>,
    positions_in_source: Vec<Option<usize>>,
}

impl TransitionGrounding {
    fn default(
        transition_id: TransitionId,
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
            transition_id,
            transition,
            assignment,
            indices,
            dims,
            positions_in_source,
        }
    }

    fn advance(&mut self, ctx: &SchedEncoder) -> Result<(), ()> {
        if self.indices.is_empty() {
            return Err(());
        }
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

    pub fn id(&self) -> usize {
        let mut res = 0;
        let mut factor = 1;
        for (&i, &d) in self.indices.iter().zip(self.dims.iter()).rev() {
            res += i * factor;
            factor *= d;
        }
        res
    }

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
}

pub type LinTermGroundingId = usize;
#[derive(Debug)]
pub struct LinTermGrounding {
    pub linterm: LinTerm,
    pub assignment: IntCst,
    pub id: LinTermGroundingId,
}

pub type SourceGroundingId = Vec<usize>;
#[derive(Debug)]
pub struct SourceGrounding {
    pub source: Option<TaskId>,
    pub assignment: Vec<IntCst>,
    indices: SourceGroundingId,
    dims: Vec<usize>,
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
            indices,
            dims,
        }
    }

    fn advance(&mut self, ctx: &SchedEncoder, ctx_empty_source_linterms: &[LinTerm]) -> Result<(), ()> {
        if self.indices.is_empty() {
            return Err(());
        }
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

    fn ith_linterm(&self, i: usize, ctx: &SchedEncoder, ctx_empty_source_linterms: &[LinTerm]) -> Option<LinTerm> {
        if let Some(task_id) = self.source {
            ctx.sched.tasks[task_id].args.get(i).copied()
        } else {
            ctx_empty_source_linterms.get(i).copied()
        }
    }

    pub fn id(&self) -> usize {
        let mut res = 0;
        let mut factor = 1;
        for (&i, &d) in self.indices.iter().zip(self.dims.iter()).rev() {
            res += i * factor;
            factor *= d;
        }
        res
    }
}

pub struct TransitionsGroundingsEnumerator<'a> {
    // transitions: &'a Transitions,
    ctx: &'a SchedEncoder,
    ctx_empty_source_linterms: Vec<LinTerm>,
    ctx_lifted_iter: Box<dyn Iterator<Item = ((TransitionId, Transition), Option<TaskId>)> + 'a>,

    current: Option<(TransitionGrounding, SourceGrounding)>,
    is_started: bool,
    is_finished: bool,
}
impl<'a> TransitionsGroundingsEnumerator<'a> {
    pub fn new(transitions: &'a Transitions, ctx: &'a SchedEncoder) -> Self {
        let ctx_empty_source_linterms = Vec::from_iter(find_empty_source_linterms(ctx));
        let mut ctx_lifted_iter = Box::new(transitions.iter());
        let current = if let Some(((transition_id, transition), source)) = ctx_lifted_iter.next() {
            Some((
                TransitionGrounding::default(transition_id, transition, source, ctx, &ctx_empty_source_linterms),
                SourceGrounding::default(source, ctx, &ctx_empty_source_linterms),
            ))
        } else {
            None
        };
        let is_started = false;
        let is_finished = current.is_none();

        let mut res = Self {
            // transitions,
            ctx,
            ctx_empty_source_linterms,
            ctx_lifted_iter,
            current,
            is_started,
            is_finished,
        };
        while !res.is_finished && !res.current_is_compatible() {
            res.advance();
        }
        res.is_started = false;
        res
    }

    fn current_is_compatible(&self) -> bool {
        if let Some((current_tr_gr, current_src_gr)) = self.current.as_ref() {
            // TODO: check whether source grounding is trivially impossible. If so, early return false.
            points_compatible(
                &current_tr_gr.assignment,
                &current_src_gr.assignment,
                &current_tr_gr.positions_in_source,
            )
        } else {
            false
        }
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
            return;
        }
        loop {
            let (current_tr_gr, current_src_gr) = self.current.as_mut().unwrap();

            if current_tr_gr.advance(self.ctx).is_err() {
                if current_src_gr
                    .advance(self.ctx, &self.ctx_empty_source_linterms)
                    .is_ok()
                {
                    *current_tr_gr = TransitionGrounding::default(
                        current_tr_gr.transition_id,
                        current_tr_gr.transition,
                        current_src_gr.source,
                        self.ctx,
                        &self.ctx_empty_source_linterms,
                    );
                } else {
                    let Some(((transition_id, transition), source)) = self.ctx_lifted_iter.next() else {
                        self.current = None;
                        self.is_finished = true;
                        return;
                    };
                    debug_assert!(transition != current_tr_gr.transition || source != current_src_gr.source);
                    *current_src_gr = SourceGrounding::default(source, self.ctx, &self.ctx_empty_source_linterms);
                    *current_tr_gr = TransitionGrounding::default(
                        transition_id,
                        transition,
                        source,
                        self.ctx,
                        &self.ctx_empty_source_linterms,
                    );
                }
            }
            if self.current_is_compatible() {
                return;
            }
        }
    }

    fn get(&self) -> Option<&Self::Item> {
        self.current.as_ref()
    }
}
