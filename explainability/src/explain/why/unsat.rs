use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use aries::core::Lit;
use aries::model::Label;

use crate::explain::explanation::{
    EssenceIndex, Essence, Substance, Explanation, ExplanationFilter, ModelIndex, SubstanceIndex,
};
use crate::explain::presupposition::{Presupposition, PresuppositionKind, PresuppositionStatusCause};
use crate::explain::{ModelAndVocab, Query, Question, Situation};
use crate::musmcs_enumeration::marco::simple_marco::SimpleMarco;
use crate::musmcs_enumeration::marco::Marco;
use crate::musmcs_enumeration::MusMcsEnumerationConfig;

pub struct QwhyUnsat<Lbl> {
    model_and_vocab: ModelAndVocab<Lbl>,
    situ: Situation,
    query: Query,
}

impl<Lbl: Label> QwhyUnsat<Lbl> {
    pub fn new(
        model_and_vocab: ModelAndVocab<Lbl>,
        situ: impl IntoIterator<Item = Lit>,
        query: impl IntoIterator<Item = Lit>,
    ) -> Self {
        Self {
            model_and_vocab,
            situ: situ.into_iter().collect(),
            query: query.into_iter().collect(),
        }
    }
}

// TODO: a "problem" / "encoding" structure allowing to keep an eye on exprs (be it query, situ, or vocab / model)
// and their reification literals / index in vector. Also maybe a function like "make_model_for_question"

impl<Lbl: Label> Question<Lbl> for QwhyUnsat<Lbl> {
    fn check_presuppositions(&mut self) -> Result<(), PresuppositionStatusCause> {
        Presupposition {
            kind: PresuppositionKind::ModelSituUnsatWithQuery,
            model: Arc::new(self.model_and_vocab.model_with_enforced_vocab()),
            situ: self.situ.clone(),
            query: self.query.clone(),
        }.check(false, None)?;

        Ok(())
    }

    fn compute_explanation(&mut self) -> Explanation<Lbl> {
        let mut essences = Vec::<Essence>::new();
        let mut substances = Vec::<Substance>::new();
        let mut table = BTreeMap::<EssenceIndex, BTreeMap<SubstanceIndex, BTreeSet<ModelIndex>>>::new();
        let filter = ExplanationFilter {
            map: None,
            default: true,
        };

        let mut marco = SimpleMarco::<Lbl>::new_with_soft_constrs_reif_lits(
            self.model_and_vocab.model_with_enforced_vocab(),
            self.query.iter().chain(&self.situ).cloned(),
            MusMcsEnumerationConfig {
                return_muses: true,
                return_mcses: false,
            },
        );
        let query_situ_muses = marco.run().muses.unwrap();

        let situ_as_set = BTreeSet::from_iter(self.situ.iter().cloned());
        for (i, mus) in query_situ_muses.into_iter().enumerate() {
            essences.push(Essence(
                mus.difference(&situ_as_set).cloned().collect(),
                mus.intersection(&situ_as_set).cloned().collect(),
            ));

            let mut marco = SimpleMarco::<Lbl>::new_with_soft_constrs_reif_lits(
                self.model_and_vocab.model_with_enforced(mus),
                self.model_and_vocab.vocab.clone(),
                MusMcsEnumerationConfig {
                    return_muses: false,
                    return_mcses: true,
                },
            );
            let model_vocab_mcses = marco.run().mcses.unwrap();

            for mcs in model_vocab_mcses {
                let sub = Substance::ModelConstraints(mcs);
                let j = substances.iter().position(|s| s == &sub).unwrap_or_else(|| {
                    substances.push(sub);
                    substances.len() - 1
                });
                table.entry(i).or_default().insert(j, BTreeSet::from([0]));
            }
        }

        Explanation {
            models: vec![self.model_and_vocab.clone()],
            essences,
            substances,
            table,
            filter,
        }
    }
}

#[cfg(test)]
mod tests {

    use std::collections::{BTreeMap, BTreeSet, HashSet};
    use std::sync::Arc;

    use aries::core::{Lit, INT_CST_MAX, INT_CST_MIN};
    use aries::model::lang::expr::{and, implies};
    use aries::model::lang::linear::LinearSum;

    use crate::explain::explanation::{Essence, Substance};
    use crate::explain::ModelAndVocab;

    use super::Question;

    type Model = aries::model::Model<String>;
    type QwhyUnsat = super::QwhyUnsat<String>;

    #[test]
    fn test_qwhy_unsat() {
        let mut model = Model::new();

        let p_a = model.new_presence_variable(Lit::TRUE, "p_a").true_lit();
        let p_b = model.new_presence_variable(Lit::TRUE, "p_b").true_lit();
        let p_c = model.new_presence_variable(Lit::TRUE, "p_c").true_lit();
        let p_d = model.new_presence_variable(Lit::TRUE, "p_d").true_lit();
        let p_e = model.new_presence_variable(Lit::TRUE, "p_e").true_lit();

        let a = model.new_ivar(0, INT_CST_MAX, "a");
        let b = model.new_ivar(0, INT_CST_MAX, "b");
        let c = model.new_ivar(0, INT_CST_MAX, "c");
        let d = model.new_ivar(0, INT_CST_MAX, "d");
        let e = model.new_ivar(0, INT_CST_MAX, "e");

        let voc = vec![
            model.new_presence_variable(Lit::TRUE, "a=1").true_lit(),
            model.new_presence_variable(Lit::TRUE, "b=2").true_lit(),
            model.new_presence_variable(Lit::TRUE, "d=3").true_lit(),
            model.new_presence_variable(Lit::TRUE, "e=0").true_lit(),
            model.new_presence_variable(Lit::TRUE, "cost<=5").true_lit(),
        ];

        let expr = and([model.reify(implies(p_a, a.leq(1))), model.reify(implies(p_a, a.geq(1)))]);
        model.enforce(expr, [voc[0]]);
        let expr = and([model.reify(implies(p_b, b.leq(2))), model.reify(implies(p_b, b.geq(2)))]);
        model.enforce(expr, [voc[1]]);
        let expr = and([model.reify(implies(p_c, c.leq(1))), model.reify(implies(p_c, c.geq(1)))]);
        model.enforce(expr, []);
        let expr = and([model.reify(implies(p_d, d.leq(3))), model.reify(implies(p_d, d.geq(3)))]);
        model.enforce(expr, [voc[2]]);
        let expr = and([model.reify(implies(p_e, e.leq(0))), model.reify(implies(p_e, e.geq(0)))]);
        model.enforce(expr, [voc[3]]);

        let r = model.new_ivar(INT_CST_MIN, 0, "r");
        model.enforce(and([r.leq(-5), r.geq(-5)]), [voc[4]]);

        let total_weight = LinearSum::of(vec![a, b, c, d, e, r]);
        model.enforce(total_weight.leq(0), []);

        let mut question = QwhyUnsat::new(
            ModelAndVocab::new(Arc::new(model), voc.clone()),
            [p_d, p_e],
            [p_a, p_b, p_c],
        );

        let expl = question.try_answer().unwrap();

        let essences: HashSet<Essence> = expl.essences.iter().cloned().collect();
        debug_assert_eq!(
            essences,
            HashSet::from([
                Essence(BTreeSet::from([p_a, p_b]), BTreeSet::from([p_d])),
                Essence(BTreeSet::from([p_b, p_c]), BTreeSet::from([p_d])),
            ]),
        );

        let substances: HashSet<Substance> = expl.substances.iter().cloned().collect();
        debug_assert_eq!(
            substances,
            HashSet::from([
                Substance::ModelConstraints(BTreeSet::from([voc[0]])),
                Substance::ModelConstraints(BTreeSet::from([voc[1]])),
                Substance::ModelConstraints(BTreeSet::from([voc[2]])),
                Substance::ModelConstraints(BTreeSet::from([voc[4]])),
            ]),
        );

        let idxe0 = expl.essences.iter().position(|e| *e == Essence(BTreeSet::from([p_a, p_b]), BTreeSet::from([p_d]))).unwrap();
        let idxe1 = expl.essences.iter().position(|e| *e == Essence(BTreeSet::from([p_b, p_c]), BTreeSet::from([p_d]))).unwrap();
        let idxs0 = expl.substances.iter().position(|s| *s == Substance::ModelConstraints(BTreeSet::from([voc[0]]))).unwrap();
        let idxs1 = expl.substances.iter().position(|s| *s == Substance::ModelConstraints(BTreeSet::from([voc[1]]))).unwrap();
        let idxs2 = expl.substances.iter().position(|s| *s == Substance::ModelConstraints(BTreeSet::from([voc[2]]))).unwrap();
        let idxs3 = expl.substances.iter().position(|s| *s == Substance::ModelConstraints(BTreeSet::from([voc[4]]))).unwrap();

        let table = expl.table;
        debug_assert_eq!(
            table,
            BTreeMap::from([
                (idxe0, BTreeMap::from([
                    (idxs0, BTreeSet::from([0])),
                    (idxs1, BTreeSet::from([0])),
                    (idxs2, BTreeSet::from([0])),
                    (idxs3, BTreeSet::from([0])),
                ])),
                (idxe1, BTreeMap::from([
                    (idxs1, BTreeSet::from([0])),
                    (idxs2, BTreeSet::from([0])),
                    (idxs3, BTreeSet::from([0])),
                ])),
            ]),
        );
    }
}