use aries::{core::Lit, utils::StreamingIterator};
use itertools::Itertools;
use planx::*;
use std::collections::BTreeMap;
use timelines::boxes::{BBox, Segment};

use crate::repair::required_values::RequiredValues;

pub(crate) type PotentialEffect = (FluentId, Vec<Param>, bool, Lit);

#[derive(Debug)]
pub(crate) struct PotentialEffects {
    pub(crate) effs: BTreeMap<ActionRef, Vec<PotentialEffect>>,
}

impl PotentialEffects {
    /// Computes the set of all potential effect, provided as a list of potential effects for each action
    ///
    /// # Parameters
    ///
    ///  - reqs: bounding boxes of the values that may appear as a condition or goal of the problem. We will avoid generating potential effects that do provide such a value
    ///  - bounds: domain of the parameter of each action, used to determine the values achievable by each potential effect.
    pub fn compute(
        model: &Model,
        reqs: &RequiredValues,
        bounds: impl Fn(&ActionRef, &Param) -> Segment,
        mut create_lit: impl FnMut() -> Lit,
    ) -> PotentialEffects {
        // helper function to compute a value box capturing the values possibly required by other parts of the problem
        let eff_val_box = |act: &ActionRef, params: &[Param], value: bool| {
            let mut segments = Vec::new();
            segments.push(Segment::all()); // Time, asssumed to be anything
            for p in params {
                segments.push(bounds(act, p));
            }
            segments.push(if value { Segment::point(1) } else { Segment::point(0) });
            BBox::new(segments)
        };
        let mut effs: BTreeMap<ActionRef, Vec<PotentialEffect>> = BTreeMap::new();
        println!("\n=== Potential effects ===\n");
        for a in model.actions.iter() {
            println!("{:?}", a.name);
            for (fluent_id, fluent) in model.env.fluents.iter_with_id() {
                println!("  {:?}", fluent.name);

                // for each paramater of the fluent, gather a list of parameters of teh action that could be used for it
                let mut candidate_params = Vec::with_capacity(fluent.parameters.len());
                for param in &fluent.parameters {
                    candidate_params.push(
                        a.parameters
                            .iter()
                            .filter(|act_param| act_param.tpe().is_subtype_of(param.tpe()))
                            .collect_vec()
                            .into_iter(),
                    );
                }

                // loop over all instanciation, i.e., unique substition of a fluent parameter by an action parameter
                let mut instanciations = aries::utils::enumerate(candidate_params);
                while let Some(instanciation) = instanciations.next() {
                    let params: Vec<Param> = instanciation.iter().cloned().cloned().collect();

                    for value in [true, false] {
                        // compute the value box, i.e., the bounding box the values it may establish
                        let vbox = eff_val_box(&a.name, &params, value);
                        if reqs.overlaps(fluent_id, vbox.as_ref()) {
                            // may actual achieve a condition/goal, add it to the set
                            println!("    {instanciation:?}");
                            effs.entry(a.name.clone()).or_default().push((
                                fluent_id,
                                params.clone(),
                                value,
                                create_lit(),
                            ));
                        } else {
                            // this instanciation can never be used, skip it
                            // println!("    {instanciation:?}    --- skipped ");
                        }
                    }
                }
            }
        }
        println!("\n");
        PotentialEffects { effs }
    }

    pub fn for_action(&self, act_name: &planx::Sym) -> &[PotentialEffect] {
        self.effs.get(act_name).map(|x| x.as_slice()).unwrap_or(&[])
    }
}
