use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use aries::core::{IntCst, Lit, VarRef};
use aries::model::extensions::{AssignmentExt, Shaped};
use aries::model::lang::expr::{and, or};
use aries::model::{Label, Model};
use aries::solver::Solver;
use itertools::Itertools;

use crate::explain::explanation::{
    EssenceIndex, Essence, Substance, Explanation, ExplanationFilter, ModelIndex, SubstanceIndex,
};
use crate::explain::presupposition::{Presupposition, PresuppositionKind, PresuppositionStatusCause};
use crate::explain::why::unsat::QwhyUnsat;
use crate::explain::{Query, Question, Situation, Vocab};
use crate::musmcs_enumeration::marco::simple_marco::SimpleMarco;
use crate::musmcs_enumeration::marco::Marco;
use crate::musmcs_enumeration::MusMcsEnumerationConfig;

pub struct QwhyNotEntail<Lbl> {
    model: Arc<Model<Lbl>>,
    situ: Situation,
    query: Query,
    vocab: Vocab,
    not_entailed_due_to_unsat: Option<bool>,
    limit_num_counterexamples_per_essence: usize,
}

impl<Lbl: Label> QwhyNotEntail<Lbl> {
    fn new(
        model: Arc<Model<Lbl>>,
        situ: impl IntoIterator<Item = Lit>,
        query: impl IntoIterator<Item = Lit>,
        vocab: impl IntoIterator<Item = Lit>,
        limit_num_counterexamples_per_essence: u32,
    ) -> Self {
        Self {
            model,
            situ: situ.into_iter().collect(),
            query: query.into_iter().collect(),
            vocab: vocab.into_iter().collect(),
            not_entailed_due_to_unsat: None,
            limit_num_counterexamples_per_essence: limit_num_counterexamples_per_essence as usize,
        }
    }

    fn trust_not_entailed_due_to_unsat(&mut self) {
        self.not_entailed_due_to_unsat = Some(true);
    }

    fn trust_not_entailed_due_to_counterexamples(&mut self) {
        self.not_entailed_due_to_unsat = Some(false);
    }
}

impl<Lbl: Label> Question<Lbl> for QwhyNotEntail<Lbl> {
    fn check_presuppositions(&mut self) -> Result<(), PresuppositionStatusCause> {
        let mut model = (*self.model).clone();

        model.enforce_all(self.vocab.iter().cloned(), []);
        let model = Arc::new(model);

        let presupp_status_cause = Presupposition {
            kind: PresuppositionKind::ModelSituNotEntailQuery,
            model,
            situ: self.situ.clone(),
            query: self.query.clone(),
        }.check(false, None)?;

        match presupp_status_cause {
            PresuppositionStatusCause::ModelSituQueryUnsat => self.trust_not_entailed_due_to_unsat(),
            PresuppositionStatusCause::ModelSituNegQuerySat => self.trust_not_entailed_due_to_counterexamples(),
            _ => panic!(),
        }
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

        assert!(
            self.not_entailed_due_to_unsat != None,
            "Before computing the explanation for why not entailed, we want to already know if it's due to unsat or due to counterexamples."
        );

        // If we the reason for non-entailment is unsatisfiability
        // then the explanation is the same as that for "why unsatisfiable".
        if self.not_entailed_due_to_unsat == Some(true) {
            return QwhyUnsat::new(
                self.model.clone(),
                self.situ.clone(),
                self.query.clone(),
                self.vocab.clone(),
            )
            .compute_explanation();
        }
        // In case of satisfiability, non-entailment is explained with counterexamples.
        // We find the "minimally non entailed" (under situation `situ`) subsets of `query`
        // and compute counterexamples for them, isolating the part that is contradicting `query`.
        debug_assert!(self.not_entailed_due_to_unsat == Some(false));

        let mut num_counterexamples_per_essence = BTreeMap::<usize, usize>::new();

        let relevant_vars: BTreeSet<VarRef> = self.query
            .iter()
            .chain(&self.situ)
            .flat_map(|&lit| self.model.get_reified_expr(lit).map_or(vec![lit.variable()], |re| re.variables()))
            .collect();

        let mut model = (*self.model).clone();
        model.enforce_all(self.vocab.iter().cloned(), []);

        let query_neg = !model.reify(and(self.query.iter().cloned().collect_vec()));
        let situ_u_query_neg = self.situ.iter().chain(&[query_neg]).copied().collect_vec();

        let solve_with_situ_n_query_neg_fn = |m: Model<Lbl>| {
            let mut s = Solver::<Lbl>::new(m);
            s.enforce_all(situ_u_query_neg.clone(), []);
            s.solve().expect("Solver interrupted")
        };

        // Look through solutions satisfying the situation, but not the query.
        while let Some(doms) = solve_with_situ_n_query_neg_fn(model.clone()) {

            // Key: reification for (var <= val & var >= val) ; Value: (var, val) pair
            // We reify (var = val) instead of using (var <= val) & (var >= val) directly,
            // because it should to result in fewer `shrink` calls in MARCO.
            let sol: BTreeMap<Lit, (VarRef, IntCst)> = relevant_vars
                .iter()
                .map(|&var| {
                    let (lb, ub) = doms.domain_of(var);
                    debug_assert_eq!(lb, ub);
                    (model.reify(and([var.geq(lb), var.leq(ub)])), (var, ub))
                })
                .collect();

            let sol_u_situ: BTreeSet<Lit> = sol
                .keys()
                .chain(&self.situ)
                .copied()
                .collect();

            let mut marco = SimpleMarco::<Lbl>::new_with_soft_constrs_reif_lits(
                model.clone(),
                self.query.iter().chain(&sol_u_situ).cloned(),
                MusMcsEnumerationConfig {
                    return_muses: true,
                    return_mcses: false,
                },
            );

            let conflicts_partitioned = marco
                .run()
                .muses
                .unwrap()
                .into_iter()
                .map(|mus| {
                    let mut mus_d_sol_u_situ = BTreeSet::<Lit>::new();
                    let mut mus_n_situ = BTreeSet::<Lit>::new();
                    let mut mus_n_sol = BTreeSet::<(VarRef, IntCst)>::new();

                    for l in mus {
                        if self.situ.contains(&l) {
                            mus_n_situ.insert(l);
                        } else if let Some(&(var, val)) = sol.get(&l) {
                            mus_n_sol.insert((var, val));
                        } else {
                            mus_d_sol_u_situ.insert(l);
                        }
                    }
                    (mus_d_sol_u_situ, mus_n_situ, mus_n_sol)
                }).collect_vec();

            for (mus_d_sol_u_situ, mus_n_situ, mus_n_sol) in conflicts_partitioned {
                let ess = Essence(mus_d_sol_u_situ, mus_n_situ);
                let sub = Substance::CounterExample(mus_n_sol.clone());

                let i = essences.iter().position(|e| e == &ess).unwrap_or_else(|| {
                    essences.push(ess);
                    essences.len() - 1
                });
                let j = substances.iter().position(|s| s == &sub).unwrap_or_else(|| {
                    substances.push(sub);
                    substances.len() - 1
                });
                table.entry(i).or_default().insert(j, BTreeSet::from([0]));

                // Increase the number of counterexamples found for this essence.
                num_counterexamples_per_essence.entry(i).and_modify(|v| *v += 1).or_insert(1);

                // Prevent this counterexample from being discovered again later.
                // NOTE: by "counterexample" we mean a minimal(!) *part* of a solution contradicting
                //       a subset of the query, NOT that *whole* solution.
                model.enforce(
                    or(mus_n_sol.iter().flat_map(|&(var, val)| [var.lt(val), var.gt(val)]).collect_vec()),
                    [],
                );
            }

            for (i, ess) in essences.iter().enumerate() {
                // If we already reached our limit of counterexamples for the given essence
                // (aka "minimally not-entailed" subset of query), then forbid that essence from being found again further.
                if num_counterexamples_per_essence[&i] >= self.limit_num_counterexamples_per_essence {
                    model.enforce(or(ess.0.union(&ess.1).map(|&l| !l).collect_vec()), []);
                }
                // NOTE: this loop needs to be after the previous one, because the same essence
                //       could be found multiple times while analyzing `conflicts_partitioned`
            }
        }
        Explanation {
            models: vec![self.model.clone()],
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

    use aries::core::state::Term;
    use aries::core::Lit;
    use aries::model::lang::expr::implies;

    use crate::explain::explanation::{Essence, Substance};

    use super::Question;

    type Model = aries::model::Model<String>;
    type QwhyNotEntail = super::QwhyNotEntail<String>;

    // TODO: need a new test / example where the non entailment could be resolved by relaxing the situation...!

    #[test]
    fn test_qwhy_not_entail() {
        let mut model = Model::new();

        let x = model.new_ivar(0, 15, "x");
        let y = model.new_ivar(0, 15, "y");

        let voc = vec![
            model.new_presence_variable(Lit::TRUE, "c1").true_lit(),
            model.new_presence_variable(Lit::TRUE, "c2").true_lit(),
            model.new_presence_variable(Lit::TRUE, "c3").true_lit(),
            model.new_presence_variable(Lit::TRUE, "c4").true_lit(),
            model.new_presence_variable(Lit::TRUE, "c5").true_lit(),
        ];

        // [x <= 4] -> [y <= 8]
        let expr = implies(x.leq(4), y.leq(8));
        model.enforce(expr, [voc[0]]);
        // [x <= 3] -> [y <= 6]
        let expr = implies(x.leq(3), y.leq(6));
        model.enforce(expr, [voc[1]]);
        // [x <= 2] -> [y <= 4]
        let expr = implies(x.leq(2), y.leq(4));
        model.enforce(expr, [voc[2]]);
        // [x <= 1] -> [y <= 2]
        let expr = implies(x.leq(1), y.leq(2));
        model.enforce(expr, [voc[3]]);
        // [y >= -1] -> [x >= -1]
        let expr = implies(y.gt(-1), x.gt(-1));
        model.enforce(expr, [voc[4]]);

        let mut question = QwhyNotEntail::new(
            Arc::new(model),
            [x.leq(3), x.geq(3)],
            [y.leq(4)],
            voc.clone(),
            3,
        );

        let expl = question.try_answer().unwrap();

        let essences: HashSet<Essence> = expl.essences.iter().cloned().collect();
        debug_assert_eq!(
            essences,
            HashSet::from([Essence(BTreeSet::from([y.leq(4)]), BTreeSet::from([]))]),
        );

        let substances: HashSet<Substance> = expl.substances.iter().cloned().collect();
        debug_assert_eq!(
            substances,
            HashSet::from([
                Substance::CounterExample(BTreeSet::from([(y.variable(), 5)])),
                Substance::CounterExample(BTreeSet::from([(y.variable(), 6)])),
            ]),
        );

        let table = expl.table;
        debug_assert_eq!(
            table,
            BTreeMap::from([(0, BTreeMap::from([(0, BTreeSet::from([0])), (1, BTreeSet::from([0]))]))]),
        );
    }
}
