use itertools::Itertools;

use crate::{
    core::state::Domains,
    reasoners::eq_alt::{
        graph::{
            transforms::{EqExt, EqNeqExt, EqNode},
            traversal::Graph,
        },
        node::Node,
        propagators::Propagator,
        relation::EqRelation,
    },
};

use super::AltEqTheory;

impl AltEqTheory {
    /// Check if source -=-> target in active graph
    fn eq_path_exists(&self, source: &Node, target: &Node) -> bool {
        let source_id = self.active_graph.get_id(source).unwrap();
        let target_id = self.active_graph.get_id(target).unwrap();
        self.active_graph
            .outgoing
            .eq()
            .traverse(source_id)
            .any(|n| n == target_id)
    }

    /// Check if source -!=-> target in active graph
    fn neq_path_exists(&self, source: &Node, target: &Node) -> bool {
        let source_id = self.active_graph.get_id(source).unwrap();
        let target_id = self.active_graph.get_id(target).unwrap();
        self.active_graph
            .outgoing
            .eq_neq()
            .traverse(EqNode::new(source_id))
            .any(|n| n == EqNode(target_id, EqRelation::Neq))
    }

    /// Check for paths which exist but don't propagate correctly on constraint literals
    fn check_path_propagation(&self, model: &Domains) -> Vec<&Propagator> {
        let mut problems = vec![];
        for source in self.active_graph.iter_nodes().collect_vec() {
            for target in self.active_graph.iter_nodes().collect_vec() {
                if self.eq_path_exists(&source, &target) {
                    self.constraint_store
                        .iter()
                        .filter(|(_, p)| p.a == source && p.b == target && p.relation == EqRelation::Neq)
                        .for_each(|(_, p)| {
                            if !model.entails(!p.enabler.active)
                                && model.entails(model.presence(p.a))
                                && model.entails(model.presence(p.b))
                            {
                                problems.push(p)
                            }
                        });
                }
                if self.neq_path_exists(&source, &target) {
                    self.constraint_store
                        .iter()
                        .filter(|(_, p)| p.a == source && p.b == target && p.relation == EqRelation::Eq)
                        .for_each(|(_, p)| {
                            if !model.entails(!p.enabler.active)
                                && model.entails(model.presence(p.a))
                                && model.entails(model.presence(p.b))
                            {
                                problems.push(p)
                            }
                        });
                }
            }
        }
        problems
    }

    /// Check for active and valid constraints which aren't modeled by a path in the graph
    fn check_active_constraint_in_graph(&mut self, model: &Domains) -> i32 {
        let mut problems = 0;
        self.constraint_store
            .iter()
            .filter(|(_, p)| model.entails(p.enabler.active) && model.entails(p.enabler.valid))
            .for_each(|(_, p)| match p.relation {
                EqRelation::Neq => {
                    if !self.neq_path_exists(&p.a, &p.b) {
                        problems += 1;
                    }
                }
                EqRelation::Eq => {
                    if !self.eq_path_exists(&p.a, &p.b) {
                        problems += 1;
                    }
                }
            });
        problems
    }

    pub fn check_propagations(&mut self, model: &Domains) {
        let path_prop_problems = self.check_path_propagation(model);
        assert_eq!(
            path_prop_problems.len(),
            0,
            "Path propagation problems: {:#?}\nGraph:\n{}\nDebug: {:?}",
            path_prop_problems,
            self.active_graph.clone().to_graphviz(),
            self.constraint_store
                .iter()
                .find(|(_, prop)| prop == path_prop_problems.first().unwrap()) // model.entails(!path_prop_problems.first().unwrap().enabler.active) // self.undecided_graph
                                                                               // .contains_edge((*path_prop_problems.first().unwrap()).clone().into())
        );

        let constraint_problems = self.check_active_constraint_in_graph(model);
        assert_eq!(
            constraint_problems,
            0,
            "{} constraint problems\nGraph:\n{}",
            constraint_problems,
            self.active_graph.to_graphviz(),
        );
    }
}
