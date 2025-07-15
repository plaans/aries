use crate::{
    core::state::Domains,
    reasoners::eq_alt::{propagators::Propagator, relation::EqRelation},
};

use super::AltEqTheory;

impl AltEqTheory {
    /// Check for paths which exist but don't propagate correctly on constraint literals
    fn check_path_propagation(&self, model: &Domains) -> Vec<&Propagator> {
        let mut problems = vec![];
        for source in self.active_graph.iter_nodes() {
            for target in self.active_graph.iter_nodes() {
                if self.active_graph.eq_path_exists(source, target) {
                    self.constraint_store
                        .iter()
                        .filter(|(_, p)| p.a == source && p.b == target && p.relation == EqRelation::Neq)
                        .for_each(|(_, p)| {
                            // Check necessarily inactive or maybe invalid
                            if !model.entails(!p.enabler.active) && model.entails(p.enabler.valid) {
                                problems.push(p)
                            }
                        });
                }
                if self.active_graph.neq_path_exists(source, target) {
                    self.constraint_store
                        .iter()
                        .filter(|(_, p)| p.a == source && p.b == target && p.relation == EqRelation::Eq)
                        .for_each(|(_, p)| {
                            if !model.entails(!p.enabler.active) && model.entails(p.enabler.valid) {
                                problems.push(p)
                            }
                        });
                }
            }
        }
        problems
    }

    /// Check for active and valid constraints which aren't modeled by a path in the graph
    fn check_active_constraint_in_graph(&self, model: &Domains) -> i32 {
        let mut problems = 0;
        self.constraint_store
            .iter()
            .filter(|(_, p)| model.entails(p.enabler.active) && model.entails(p.enabler.valid))
            .for_each(|(_, p)| match p.relation {
                EqRelation::Neq => {
                    if !self.active_graph.neq_path_exists(p.a, p.b) {
                        problems += 1;
                    }
                }
                EqRelation::Eq => {
                    if !self.active_graph.eq_path_exists(p.a, p.b) {
                        problems += 1;
                    }
                }
            });
        problems
    }

    pub fn check_propagations(&self, model: &Domains) {
        let path_prop_problems = self.check_path_propagation(model);
        assert_eq!(
            path_prop_problems.len(),
            0,
            "Path propagation problems: {:#?}\nGraph:\n{}\n{}",
            path_prop_problems,
            self.active_graph.to_graphviz(),
            self.undecided_graph.to_graphviz(),
        );

        let constraint_problems = self.check_active_constraint_in_graph(model);
        assert_eq!(
            constraint_problems,
            0,
            "{} constraint problems\nGraph:\n{}\n{}",
            constraint_problems,
            self.active_graph.to_graphviz(),
            self.undecided_graph.to_graphviz()
        );
    }
}
