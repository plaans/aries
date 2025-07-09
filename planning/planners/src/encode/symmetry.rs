use crate::encode::analysis;
use crate::encoding::{ChronicleId, CondID, EffID, Encoding, Tag};
use analysis::CausalSupport;
use aries::core::Lit;
use aries::model::extensions::AssignmentExt;
use aries::model::lang::expr::{and, f_leq, implies, or};
use aries_planning::chronicles::analysis::Metadata;
use aries_planning::chronicles::{ChronicleOrigin, FiniteProblem};
use env_param::EnvParam;
use itertools::Itertools;
use std::collections::{BTreeMap, HashSet};

use crate::Model;

/// Parameter that defines the symmetry breaking strategy to use.
/// The value of this parameter is loaded from the environment variable `ARIES_LCP_SYMMETRY_BREAKING`.
/// Possible values are `none` and `simple` (default).
pub static SYMMETRY_BREAKING: EnvParam<SymmetryBreakingType> = EnvParam::new("ARIES_LCP_SYMMETRY_BREAKING", "psp");
pub static USELESS_SUPPORTS: EnvParam<bool> = EnvParam::new("ARIES_USELESS_SUPPORTS", "true");
pub static DETRIMENTAL_SUPPORTS: EnvParam<bool> = EnvParam::new("ARIES_DETRIMENTAL_SUPPORTS", "true");
pub static PSP_ABSTRACTION_HIERARCHY: EnvParam<bool> = EnvParam::new("ARIES_PSP_ABSTRACTION_HIERARCHY", "true");

/// The type of symmetry breaking to apply to problems.
#[derive(Copy, Clone)]
pub enum SymmetryBreakingType {
    /// no symmetry breaking
    None,
    /// Simple form of symmetry breaking described in the LCP paper (CP 2018).
    /// This enforces that for any two instances of the same template. The first one (in arbitrary total order)
    ///  - is always present if the second instance is present
    ///  - starts before the second instance
    Simple,
    /// Symmetry breaking based on the causal graph, essentially ensuring that swapping two instances of the same template
    /// does not result in an isomorphic causal graph.
    /// Ref: Towards Canonical and Minimal Solutions in a Constraint-based Plan-Space Planner
    PlanSpace,
}

impl std::str::FromStr for SymmetryBreakingType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(SymmetryBreakingType::None),
            "simple" => Ok(SymmetryBreakingType::Simple),
            "plan-space" | "planspace" | "psp" => Ok(SymmetryBreakingType::PlanSpace),
            x => Err(format!("Unknown symmetry breaking type: {x}")),
        }
    }
}

fn supported_by_psp(meta: &Metadata) -> bool {
    !meta.class.is_hierarchical()
}

pub fn add_symmetry_breaking(pb: &FiniteProblem, model: &mut Model, encoding: &Encoding) {
    let tpe: SymmetryBreakingType = SYMMETRY_BREAKING.get();

    let tpe = match tpe {
        SymmetryBreakingType::PlanSpace if !supported_by_psp(&pb.meta) => SymmetryBreakingType::Simple,
        other => other,
    };

    match tpe {
        SymmetryBreakingType::None => {}
        SymmetryBreakingType::Simple => {
            let chronicles = || {
                pb.chronicles.iter().filter_map(|c| match c.origin {
                    ChronicleOrigin::FreeAction {
                        template_id,
                        generation_id,
                    } => Some((c, template_id, generation_id)),
                    _ => None,
                })
            };
            for (instance1, template_id1, generation_id1) in chronicles() {
                for (instance2, template_id2, generation_id2) in chronicles() {
                    if template_id1 == template_id2 && generation_id1 < generation_id2 {
                        let p1 = instance1.chronicle.presence;
                        let p2 = instance2.chronicle.presence;
                        model.enforce(implies(p1, p2), []);
                        model.enforce(f_leq(instance1.chronicle.start, instance2.chronicle.start), [p1, p2]);
                    }
                }
            }
        }
        SymmetryBreakingType::PlanSpace => add_plan_space_symmetry_breaking(pb, model, encoding),
    };
}

/// Symmetry breaking based on the causal graph, essentially ensuring that swapping two instances of the same template
/// does not result in an isomorphic causal graph.
/// Ref: Towards Canonical and Minimal Solutions in a Constraint-based Plan-Space Planner
fn add_plan_space_symmetry_breaking(pb: &FiniteProblem, model: &mut Model, encoding: &Encoding) {
    let discard_useless_supports = USELESS_SUPPORTS.get();
    let discard_detrimental_supports = DETRIMENTAL_SUPPORTS.get();
    let sort_by_hierarchy_level = PSP_ABSTRACTION_HIERARCHY.get();

    let template_id = |instance_id: usize| match pb.chronicles[instance_id].origin {
        ChronicleOrigin::FreeAction { template_id, .. } => Some(template_id),
        _ => None,
    };
    let is_primary_support = |c: CondID, eff: EffID| {
        let Some(c_template) = template_id(c.instance_id) else {
            return true;
        };
        let Some(e_template) = template_id(eff.instance_id) else {
            return true;
        };
        let causal = CausalSupport::new(e_template, eff.eff_id, c_template, c.cond_id);
        // return true if the potential causal link is not flagged as useless
        !pb.meta.detrimental_supports.contains(&causal)
    };
    #[derive(Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
    struct CausalLinkId {
        eff: EffID,
        cond: CondID,
    }

    type TemplateID = usize;
    struct ActionOrigin {
        template: TemplateID,
    }
    // returns all actions instanciated from templates
    let actions: BTreeMap<ChronicleId, _> = pb
        .chronicles
        .iter()
        .enumerate()
        .filter_map(|(id, c)| match c.origin {
            ChronicleOrigin::FreeAction { template_id, .. } => Some((id, ActionOrigin { template: template_id })),
            _ => None,
        })
        .collect();

    let templates: Vec<TemplateID> = pb
        .chronicles
        .iter()
        .filter_map(|c| match c.origin {
            ChronicleOrigin::FreeAction { template_id, .. } => Some(template_id),
            _ => None,
        })
        .sorted()
        .dedup()
        .collect_vec();

    #[derive(Clone, Copy)]
    struct Link {
        active: Lit,
    }

    // gather all causal links, together with a literal stating whether they are are active (including the requirement for both actions to be present)
    let mut cls: BTreeMap<CausalLinkId, Link> = Default::default();
    for &(k, v) in &encoding.tags {
        let Tag::Support(cond, eff) = k else {
            panic!("Unsupported tag: {k:?}");
        };
        if model.entails(!v) {
            continue; // link can never be achieved => ignore
        }
        let instance = eff.instance_id;
        let ch = &pb.chronicles[instance];
        let ChronicleOrigin::FreeAction { .. } = ch.origin else {
            continue;
        };
        if discard_detrimental_supports && !is_primary_support(cond, eff) {
            continue; // remove non-primary supports
        }
        // non-optional literal that is true iff the causal link is active
        let link_active = model.reify(and([v, model.presence_literal(v.variable())]));
        // list of outgoing causal links of the supporting action
        let link = Link { active: link_active };
        cls.insert(CausalLinkId { eff, cond }, link);
    }
    let sort_key = |c: &CausalLinkId| {
        // Penalty for increase effects tend to be very uninformative because:
        //  - they may not actually contribute to the condition (the condition could be satisfied even without it due to overshooting)
        //  - they typically persist for a very long time even if canceled out
        // This penalty is such that they are placed last in the queue
        let penalty = if c.eff.is_assign { 0 } else { 1 };
        // Goals (conditions in the original chronicle) should come first,
        // Other conditions are grouped by abstraction level
        let lvl = if let Some(template) = template_id(c.cond.instance_id) {
            1 + if sort_by_hierarchy_level {
                pb.meta.action_hierarchy[&template]
            } else {
                0
            }
        } else {
            0
        };
        // to finalize the ordering, group by condition
        (penalty, lvl, c.cond, c.eff)
    };

    // gather all causal links in a single vector that we will use to force lexicographic order upon permitation
    let links = cls
        .keys()
        .sorted_by_cached_key(|cl| sort_key(cl))
        .copied()
        .collect_vec();

    // Return the causal link that we would have if were were to swap the `original` instance with the `replacement` instance (and vice-versa).
    // Returns None if the swap is a no-op (the two actions appear neither in the condition nor the effect)
    let swap = |cl: &CausalLinkId, original: usize, replacement: usize| {
        // effect after the swap
        let new_eff = if cl.eff.instance_id == original {
            EffID {
                instance_id: replacement,
                eff_id: cl.eff.eff_id,
                is_assign: cl.eff.is_assign,
            }
        } else if cl.eff.instance_id == replacement {
            EffID {
                instance_id: original,
                eff_id: cl.eff.eff_id,
                is_assign: cl.eff.is_assign,
            }
        } else {
            cl.eff
        };
        // condition after the swap
        let new_cond = if cl.cond.instance_id == original {
            CondID {
                instance_id: replacement,
                cond_id: cl.cond.cond_id,
            }
        } else if cl.cond.instance_id == replacement {
            CondID {
                instance_id: original,
                cond_id: cl.cond.cond_id,
            }
        } else {
            cl.cond
        };
        // swapped causal link
        let new_cl = CausalLinkId {
            eff: new_eff,
            cond: new_cond,
        };
        if &new_cl != cl {
            Some(new_cl)
        } else {
            None
        }
    };

    for template_id in &templates {
        // collect all instances of this action template
        // note: sorted to ensure reproducibility
        let instances: Vec<_> = actions
            .iter()
            .filter_map(|(id, orig)| if orig.template == *template_id { Some(id) } else { None })
            .sorted()
            .dedup()
            .collect();

        // // detailed printing for debugging
        // if let Some(ch) = instances.first() {
        //     let ch = &pb.chronicles[**ch];
        //     let s = format_partial_name(&ch.chronicle.name, model).unwrap();
        //     println!("{template_id} {s}   ({})", instances.len());
        //     // for cond_id in conditions {
        //     //     print_cond(*cond_id, pb, model);
        //     //     println!();
        //     // }
        // }

        // An instance of the template is allowed to support a condition only if the previous instance
        // supports a condition at an earlier level or at the same level.
        //
        for (i, crt_instance) in instances.iter().copied().enumerate() {
            let mut clause: Vec<Lit> = Vec::with_capacity(128);
            if i > 0 {
                let prv_instance = instances[i - 1];

                // we need break symmetries between this instance and the previous.
                // We build the support signature of the current instance
                // Noting X = [x_1, x_2, ..., x_n] where x_i <=> an effect of the previous instance supports the i-th condition
                //
                // The vector Y = [y_1, y_2, ..., y_n] is obtained by swapping previous/current over the vector X
                // We also maintain a vector Excu = [exclu_1, exclu_2, ..., exclu_n] of boolean values where exclu_i is true iff x_i and y_i are mutually exclusive

                // set of causal links pairs (x_i, y_i) that have already been added to the vector.
                // This enables an optimization where adding (y_i, x_i) is redundant and can be avoided
                let mut previously_handled = HashSet::new();
                // The three vectors [X,Y,Exclu] are built into a single one where each element is of the form (x_i, y_i, exclu_i)
                let mut vec = Vec::with_capacity(128);

                for &cl in &links {
                    // build the vectors, ignoring cases where (x_i = y_i) (for which swap would return None)
                    if let Some(swapped) = swap(&cl, *crt_instance, *prv_instance) {
                        debug_assert_eq!(cl.eff.is_assign, swapped.eff.is_assign);
                        let exclusive = cl.eff.is_assign && cl.cond == swapped.cond;
                        if previously_handled.contains(&(swapped, cl)) {
                            // Lemma 1 of Rintannen et al, ECAI 2024
                            // there is no need to consider if we have already considered the reverse
                            continue;
                        }
                        if cl.eff.instance_id != *crt_instance && swapped.eff.instance_id != *crt_instance {
                            // only keep effects of the action
                            continue;
                        }
                        previously_handled.insert((cl, swapped));
                        let cl_lit = cls[&cl].active; // x_i
                        let swapped_lit = cls[&swapped].active; // y_i
                        let entry = (cl_lit, swapped_lit, exclusive);
                        vec.push(entry);
                    }
                }

                // Enforce X >= Y
                //
                // This is done recursively for 1 <= j <= n:
                //     X[1:j] >= Y[1:j] <=> (x_j >= y_j OR there exists i < j such that x_i > y_i)
                //                          AND X[1:j-1] >= Y[1:j-1]   (enforced by previous iteration of the recursion)
                //
                // Because we are dealing with literals, we can simplify the comparison to:
                //     x > y <=> x AND !y    if x and y are not exclusive
                //     x > y <=> x           if x and y are exclusive
                //     x >= y <=> x OR !y    if x and y are not exclusive
                //     x >= y <=> !y         if x and y are exclusive
                for (j, (x_j, y_j, exclu_j)) in vec.iter().copied().enumerate() {
                    clause.clear(); // Stores the disjunction for the first part of X[1:j] >= Y[1:j]

                    // x_j >= y_j  <=>  x_j OR !y_j    (but only !y_j if x_j and y_j are exclusive)
                    clause.push(!y_j);
                    if !exclu_j {
                        // only necessary if  not exclusive
                        clause.push(x_j);
                    }

                    for &(x_i, y_i, exclu_i) in &vec[0..j] {
                        // x_i > y_i
                        if exclu_i {
                            // x_i > y_i <=> x_k (exclusive)
                            clause.push(x_i);
                        } else {
                            // x_i > y_i <=> x_i AND !y_i
                            clause.push(model.reify(and([x_i, !y_i])));
                        }
                    }

                    // (x_j >= y_j OR there exists i < j such that x_i > y_i)
                    model.enforce(or(clause.as_slice()), []);
                    // X[1:j-1] >= Y[1:j-1] has been enforced by the previous iteration of the loop
                }
            }
            clause.clear();
            if discard_useless_supports {
                // enforce that a chronicle be present only if it supports at least one external condition
                clause.push(!pb.chronicles[*crt_instance].chronicle.presence);
                // when discard_useless_supports is enabled, links only contain useful links
                // State that (if present) the chronicle must support at least one condition from another chronicle
                links
                    .iter()
                    .filter(|l| l.eff.instance_id == *crt_instance) // restrict to own effect
                    .filter(|l| l.cond.instance_id != *crt_instance) // only consider conditions from other actions
                    .for_each(|l| {
                        let lit = cls[l].active;
                        clause.push(lit);
                    });
                model.enforce(or(clause.as_slice()), []);
            }
        }
    }
}

#[allow(unused)]
fn print_cond(cid: CondID, pb: &FiniteProblem, model: &Model) {
    let ch = &pb.chronicles[cid.instance_id];
    let state_var = match cid.cond_id {
        analysis::CondOrigin::ExplicitCondition(cond_id) => &ch.chronicle.conditions[cond_id].state_var,
        analysis::CondOrigin::PostIncrease(eff_id) => &ch.chronicle.effects[eff_id].state_var,
    };
    let s = model.shape.symbols.format(&[state_var.fluent.sym]);
    print!("  {:?}:{}", ch.origin, s)
}
