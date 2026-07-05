pub mod ground;
pub mod lprelax;

use std::{collections::HashSet, ops::Index};

use aries_solver::core::{INT_CST_MAX, IntCst, LongCst};
use itertools::Itertools;

use crate::{
    EffectId,
    encoder::{CondId, SchedEncoder},
};

pub(crate) type Source = Option<crate::TaskId>;

/// Collects conditions and effects whose (non-constant) terms do not all appear in their source's args.
/// For each such effect, also ignores all conditions that effect could support.
///
/// These "ambiguous" conditions / effects are to be relaxed / ignored when collecting transitions to encode the LP relaxation.
///
/// Notably, a condition / effect using a reified variable as a term will be considered ambiguous.
pub fn collect_ambiguous_conditions_and_effects_to_relax(ctx: &SchedEncoder) -> (HashSet<CondId>, HashSet<EffectId>) {
    let (mut ambiguous_conditions, mut ambiguous_effects) = (HashSet::new(), HashSet::new());

    let get_source_terms = |src| {
        if let Some(task_id) = src {
            ctx.sched.tasks[task_id].args.as_slice()
        } else {
            ctx.sched.global_args.as_slice()
        }
    };

    for (eff_id, e) in ctx.sched.effects.iter().enumerate() {
        let source_terms = get_source_terms(e.source);
        match e.operation {
            crate::EffectOp::Assign(term) => {
                if e.state_var
                    .args
                    .iter()
                    .chain(&[term])
                    .any(|term| !term.is_constant() && !source_terms.iter().map(|(t, _)| *t).contains(term))
                {
                    ambiguous_effects.insert(eff_id);
                }
            }
            crate::EffectOp::Step(_term) => {
                ambiguous_effects.insert(eff_id);
            }
        }
    }

    for cl in ctx.causal_links.get_links() {
        if ambiguous_conditions.contains(&cl.cond_id) {
            continue;
        }
        if ambiguous_effects.contains(&cl.eff_id) {
            ambiguous_conditions.insert(cl.cond_id);
        } else {
            let c = &ctx.causal_links.conditions[cl.cond_id];
            let source_terms = get_source_terms(c.source);
            if c.state_var
                .args
                .iter()
                .chain(&[c.value])
                .any(|term| !term.is_constant() && !source_terms.iter().map(|(t, _)| *t).contains(term))
            {
                ambiguous_conditions.insert(cl.cond_id);
            }
        }
    }

    (ambiguous_conditions, ambiguous_effects)
}

pub type GroundingFlatId = Option<usize>;
/// A wrapper around a vector of constants.
/// Can be flattened into a integer id given the first value and dimension of each "column".
/// In practice, these come from the integer encoding ranges of state functions' parameter types.
#[derive(Debug, Clone)]
pub struct Grounding(Vec<IntCst>);

impl Grounding {
    // fn to_vec_id<VecId: From<Vec<usize>>>(&self, startvals: &[IntCst]) -> VecId {
    //     debug_assert!(self.0.len() == startvals.len());
    //     VecId::from(Vec::from_iter(self.0.iter().zip(startvals.iter()).map(
    //         |(&x, &startval)| {
    //             debug_assert!((x as LongCst - startval as LongCst) + 1 >= 0);
    //             debug_assert!((x as LongCst - startval as LongCst) < INT_CST_MAX as LongCst);
    //             usize::try_from(x - startval + 1).unwrap()
    //         },
    //     )))
    // }
    /*fn to_flat_id<FlatId: From<Option<NonZeroU32>>(&self, startvals: &[IntCst], dims: &[usize]) -> FlatId {
        debug_assert!(self.0.len() == startvals.len());
        debug_assert!(self.0.len() == dims.len());
        let mut res = 0;
        let mut factor = 1;
        for ((&x, &startval), &d) in self.0.iter().zip(startvals.iter()).zip(dims.iter()).rev() {
            debug_assert!((x as LongCst - startval as LongCst) + 1 >= 0);
            debug_assert!((x as LongCst - startval as LongCst) < INT_CST_MAX as LongCst);

            debug_assert!(usize::try_from(x - startval + 1).unwrap() <= d);
            res += usize::try_from(x - startval + 1).unwrap() * factor;
            factor *= d;
        }
        FlatId::from(res)
    }*/
    fn to_flat_id(&self, ranges: &[(IntCst, IntCst)]) -> GroundingFlatId {
        debug_assert!(self.0.len() == ranges.len());

        if self.0.is_empty() {
            return None;
        }

        let mut res = 0;
        let mut factor = 1;
        for (&n, &(lb, ub)) in self.0.iter().zip(ranges).rev() {
            debug_assert!((ub as LongCst - lb as LongCst) + 1 >= 0, "{lb} {ub}");
            debug_assert!((ub as LongCst - lb as LongCst) < INT_CST_MAX as LongCst, "{lb} {ub}");
            let (first, dim) = (lb, usize::try_from(ub - lb + 1).unwrap());

            debug_assert!((n as LongCst - first as LongCst) >= 0, "{n} {first}");
            debug_assert!(
                (n as LongCst - first as LongCst) <= INT_CST_MAX as LongCst,
                "{n} {first}"
            );
            debug_assert!(usize::try_from(n - first).unwrap() <= dim, "{n} {first}");

            res += usize::try_from(n - first).unwrap() * factor;
            factor *= dim;
        }
        Some(res)
    }
    // fn from_vec_id<VecId: AsRef<[usize]>>(vec_id: &VecId, startvals: &[IntCst]) -> Self {
    //     debug_assert!(vec_id.as_ref().len() == startvals.len());
    //     Self(Vec::from_iter(vec_id.as_ref().iter().zip(startvals.iter()).map(
    //         |(&n, &startval)| {
    //             debug_assert!((n as LongCst + startval as LongCst) > INT_CST_MIN as LongCst);
    //             debug_assert!((n as LongCst + startval as LongCst) - 1 <= INT_CST_MAX as LongCst);
    //             IntCst::try_from(n).unwrap() + startval - 1
    //         },
    //     )))
    // }
    // fn from_flat_id<FlatId: Into<usize>>(flat_id: &FlatId, dims: &[usize], startvals: &[IntCst]) -> Self {
    //     todo!()
    // }
}
impl Index<usize> for Grounding {
    type Output = IntCst;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
