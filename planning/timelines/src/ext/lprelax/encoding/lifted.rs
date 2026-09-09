use crate::{
    encoder::SchedEncoder,
    ext::lprelax::{LpRelaxEncoder, transitions::TransitionId},
};

#[derive(Clone, Default)]
pub(super) struct LpRelaxEncodingLiftedSupportsSorted {
    sorted_out: Vec<(TransitionId, TransitionId)>,
    sorted_in: Vec<(TransitionId, TransitionId)>,
}
impl LpRelaxEncodingLiftedSupportsSorted {
    pub fn from(encoder: &LpRelaxEncoder, ctx: &SchedEncoder) -> Self {
        let mut sorted_out = vec![];
        let mut sorted_in = vec![];

        for ((out_transition_id, in_transition_id), _) in encoder.iter_supports(ctx) {
            sorted_out.push((out_transition_id, in_transition_id));
            sorted_in.push((in_transition_id, out_transition_id));
        }
        sorted_out.sort_unstable();
        sorted_in.sort_unstable();

        Self { sorted_out, sorted_in }
    }

    pub fn out(&self) -> &[(TransitionId, TransitionId)] {
        &self.sorted_out
    }
    pub fn in_(&self) -> &[(TransitionId, TransitionId)] {
        &self.sorted_in
    }
}
