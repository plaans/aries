use crate::backtrack::{Backtrack, DecLvl, DecisionLevelTracker};
use crate::core::state::{Domains, Explanation, InferenceCause};
use crate::core::{IntCst, Lit, VarRef};
use crate::reasoners::eq::{DenseEqTheory, Node, ReifyEq};
use crate::reasoners::{Contradiction, ReasonerId, Theory};
use itertools::Itertools;
use std::collections::HashMap;

type PartId = u16;

/// Equality logic theory that partition variables into potentially unifiable variables.
/// Each such group is handled by ad Dense Equality theory
#[derive(Clone, Default)]
pub struct SplitEqTheory {
    parts: Vec<Option<DenseEqTheory>>,
    part_of: HashMap<VarRef, PartId>,
    lvl: DecisionLevelTracker,
}

impl SplitEqTheory {
    pub fn add_edge(&mut self, a: VarRef, b: VarRef, model: &mut impl ReifyEq) -> Lit {
        debug_assert_eq!(self.lvl.num_saved(), 0, "Adding an edge but not at the root");

        let l = model.reify_eq(Node::Var(a), Node::Var(b));

        let domains = model.domains();

        if domains.value(l) != Some(false) && domains.present(a) != Some(false) && domains.present(b) != Some(false) {
            // the edge may be true and active, the two variables should be in the same part

            match (self.part_of.get(&a).copied(), self.part_of.get(&b).copied()) {
                (Some(pa), None) => {
                    self.parts[pa as usize].as_mut().unwrap().add_node(b, model);
                    self.part_of.insert(b, pa);
                    debug_assert!(self.parts[pa as usize].as_ref().unwrap().variables().contains(&b))
                }
                (None, Some(pb)) => {
                    self.parts[pb as usize].as_mut().unwrap().add_node(a, model);
                    self.part_of.insert(a, pb);
                    debug_assert!(self.parts[pb as usize].as_ref().unwrap().variables().contains(&a))
                }
                (None, None) => {
                    let id: PartId =
                        if let Some((id, _)) = self.parts.iter().enumerate().find(|(_, item)| item.is_none()) {
                            id as u16
                        } else {
                            debug_assert!(self.parts.len() < u16::MAX as usize);
                            self.parts.push(None);
                            (self.parts.len() - 1) as u16
                        };
                    // the nodes are not part of any group, create a new group for them
                    let mut group = DenseEqTheory::new(id);
                    group.add_node(a, model);
                    group.add_node(b, model);
                    group.add_edge(a, b, model);

                    self.part_of.insert(a, id);
                    self.part_of.insert(b, id);
                    self.parts[id as usize] = Some(group);
                    debug_assert!(self.parts[id as usize].as_ref().unwrap().variables().contains(&a));
                    debug_assert!(self.parts[id as usize].as_ref().unwrap().variables().contains(&b));
                }
                (Some(pa), Some(pb)) if pa == pb => {
                    // they are already in the same group, nothing to do
                }
                (Some(pa), Some(pb)) => {
                    debug_assert_ne!(pa, pb);
                    // they are in two distinct group, we need to merge them
                    let (first, second) = if pa < pb { (pa, pb) } else { (pb, pa) };
                    debug_assert!(self.parts[second as usize].is_some());
                    let to_delete = std::mem::replace(&mut self.parts[second as usize], None).unwrap();
                    let final_group = self.parts[first as usize].as_mut().unwrap();
                    for var in to_delete.variables() {
                        final_group.add_node(var, model);
                        self.part_of.insert(var, first);
                    }
                    debug_assert!(self.part_of.iter().all(|(_var, part)| *part != second));
                }
            }
        }

        l
    }

    pub fn add_val_edge(&mut self, var: VarRef, val: IntCst, model: &mut impl ReifyEq) -> Lit {
        debug_assert_eq!(self.lvl.num_saved(), 0, "Adding an edge but not at the root");

        let l = model.reify_eq(Node::Var(var), Node::Val(val));

        let domains = model.domains();
        if domains.value(l) != Some(false) && domains.present(var) != Some(false) {
            // the edge may be true and the variable may be present
            // make sure the variable is in a group (necessary for domain propagation

            match self.part_of.get(&var) {
                Some(pid) => {
                    // nothing to do, already in a group
                    self.parts[*pid as usize].as_mut().unwrap().add_node(val, model);
                }
                None => {
                    let id: PartId =
                        if let Some((id, _)) = self.parts.iter().enumerate().find(|(_, item)| item.is_none()) {
                            id as u16
                        } else {
                            debug_assert!(self.parts.len() < u16::MAX as usize);
                            self.parts.push(None);
                            (self.parts.len() - 1) as u16
                        };
                    // the nodes are not part of any group, create a new group for them
                    let mut group = DenseEqTheory::new(id);
                    group.add_node(var, model);
                    group.add_node(val, model);
                    self.part_of.insert(var, id);
                    self.parts[id as usize] = Some(group);
                }
            }
        }
        l
    }

    pub fn parts(&self) -> impl Iterator<Item = &DenseEqTheory> + '_ {
        self.parts.iter().filter_map(|item| item.as_ref())
    }

    pub fn parts_mut(&mut self) -> impl Iterator<Item = &mut DenseEqTheory> + '_ {
        self.parts.iter_mut().filter_map(|item| item.as_mut())
    }
}

impl Backtrack for SplitEqTheory {
    fn save_state(&mut self) -> DecLvl {
        for part in self.parts_mut() {
            part.save_state();
        }
        self.lvl.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.lvl.num_saved()
    }

    fn restore_last(&mut self) {
        for p in self.parts_mut() {
            p.restore_last()
        }
        self.lvl.restore_last();
    }
}

impl Theory for SplitEqTheory {
    fn identity(&self) -> ReasonerId {
        ReasonerId::Eq(0)
    }

    fn propagate(&mut self, model: &mut Domains) -> Result<(), Contradiction> {
        for part in self.parts_mut() {
            part.propagate(model)?;
        }
        Ok(())
    }

    fn explain(&mut self, literal: Lit, context: InferenceCause, model: &Domains, out_explanation: &mut Explanation) {
        let ReasonerId::Eq(part_id) = context.writer else {
            panic!()
        };
        self.parts[part_id as usize]
            .as_mut()
            .unwrap()
            .explain(literal, context, model, out_explanation);
    }

    fn print_stats(&self) {
        let mut stats = crate::reasoners::eq::dense::Stats::default();
        for part in self.parts() {
            stats += part.stats.clone();
        }
        println!("{stats:?}");
    }

    fn clone_box(&self) -> Box<dyn Theory> {
        Box::new(self.clone())
    }
}
