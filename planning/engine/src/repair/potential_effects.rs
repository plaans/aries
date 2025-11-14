use aries::{core::Lit, utils::StreamingIterator};
use itertools::Itertools;
use planx::*;
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct PotentialEffects {
    pub(crate) effs: BTreeMap<ActionRef, Vec<(FluentId, Vec<Param>, Lit)>>,
}

impl PotentialEffects {
    pub fn compute(model: &Model, mut create_lit: impl FnMut() -> Lit) -> PotentialEffects {
        let mut effs: BTreeMap<ActionRef, Vec<(FluentId, Vec<Param>, Lit)>> = BTreeMap::new();
        for a in model.actions.iter() {
            println!("{:?}", a.name);
            for (fluent_id, fluent) in model.env.fluents.iter_with_id() {
                println!("  {:?}", fluent.name);
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
                let mut instanciations = aries::utils::enumerate(candidate_params);
                while let Some(instanciation) = instanciations.next() {
                    let params: Vec<Param> = instanciation.iter().cloned().cloned().collect();
                    println!("    {instanciation:?}");
                    effs.entry(a.name.clone())
                        .or_default()
                        .push((fluent_id, params, create_lit()));
                }
            }
        }
        PotentialEffects { effs }
    }

    pub fn for_action(&self, act_name: &planx::Sym) -> &[(FluentId, Vec<Param>, Lit)] {
        self.effs.get(act_name).map(|x| x.as_slice()).unwrap_or(&[])
    }
}
