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
use std::collections::{HashMap, HashSet};

use crate::Model;

/// Parameter that defines the symmetry breaking strategy to use.
/// The value of this parameter is loaded from the environment variable `ARIES_LCP_SYMMETRY_BREAKING`.
/// Possible values are `none` and `simple` (default).
pub static SYMMETRY_BREAKING: EnvParam<SymmetryBreakingType> = EnvParam::new("ARIES_LCP_SYMMETRY_BREAKING", "psp");
pub static USELESS_SUPPORTS: EnvParam<bool> = EnvParam::new("ARIES_USELESS_SUPPORTS", "true");
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
    /// Deprecated implementation of the plan-space symmetry breaking. It works well but makes some hard to satisfy assumptions which
    /// prevents mosts tweakings of the order of conditions
    PlanSpaceDeprecated,
    PlanSpace,
}

impl std::str::FromStr for SymmetryBreakingType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(SymmetryBreakingType::None),
            "simple" => Ok(SymmetryBreakingType::Simple),
            "plan-space" | "planspace" | "psp" => Ok(SymmetryBreakingType::PlanSpace),
            "plan-space-deprecated" | "psp-dep" => Ok(SymmetryBreakingType::PlanSpaceDeprecated),
            x => Err(format!("Unknown symmetry breaking type: {x}")),
        }
    }
}

fn supported_by_psp(meta: &Metadata) -> bool {
    !meta.class.is_hierarchical()
}

pub fn add_symmetry_breaking(pb: &FiniteProblem, model: &mut Model, encoding: &Encoding) {
    let tpe: SymmetryBreakingType = SYMMETRY_BREAKING.get();

    use SymmetryBreakingType::{PlanSpace, PlanSpaceDeprecated, Simple};
    let tpe = match tpe {
        PlanSpace | PlanSpaceDeprecated if !supported_by_psp(&pb.meta) => Simple,
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
        SymmetryBreakingType::PlanSpaceDeprecated => add_deprecated_plan_space_symmetry_breaking(pb, model, encoding),
        SymmetryBreakingType::PlanSpace => add_plan_space_symmetry_breaking(pb, model, encoding),
    };
}

#[derive(Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
struct CausalLinkId {
    eff: EffID,
    cond: CondID,
}

impl std::fmt::Debug for CausalLinkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}/{}=>{}/{:?}",
            self.eff.instance_id, self.eff.eff_id, self.cond.instance_id, self.cond.cond_id
        )
    }
}

fn add_plan_space_symmetry_breaking(pb: &FiniteProblem, model: &mut Model, encoding: &Encoding) {
    let discard_useless_supports = USELESS_SUPPORTS.get();
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

    struct ActionOrigin {
        template: usize,
        #[allow(unused)]
        gen: usize,
    }
    let actions: HashMap<ChronicleId, _> = pb
        .chronicles
        .iter()
        .enumerate()
        .filter_map(|(id, c)| match c.origin {
            ChronicleOrigin::FreeAction {
                template_id,
                generation_id,
            } => Some((
                id,
                ActionOrigin {
                    template: template_id,
                    gen: generation_id,
                },
            )),
            _ => None,
        })
        .collect();

    type TemplateID = usize;
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
    let mut cls: HashMap<CausalLinkId, Link> = Default::default();
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
        if discard_useless_supports && !is_primary_support(cond, eff) {
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
        //  - they may not actually contribute to the condition (the condition could be satisfied even without it due to overshouting)
        //  - they typically persist for a very long time even if canceled out
        // This penalty is such that they are placed last in the queue
        let penalty = if c.eff.is_assign { 1 } else { 0 };
        // Goals (conditions in the original chronicle) should first,
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
    let links = cls
        .keys()
        .sorted_by_cached_key(|cl| sort_key(cl))
        .copied()
        .collect_vec();

    let swap = |cl: &CausalLinkId, original: usize, replacement: usize| {
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
        let instances: Vec<_> = actions
            .iter()
            .filter_map(|(id, orig)| if orig.template == *template_id { Some(id) } else { None })
            .sorted()
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
        // Noting X = [x_1, x_2, ..., x_n] where x_i <=> previous instance supports condition i
        // and    Y = [y_1, y_2, ..., y_n] where y_i <=> current instance supports condition i
        // We ensure that X >= Y in the lexicographic order.
        //
        // This is done recursively for 1 <= j <= n:
        //     X[1:j] >= Y[1:j] <=> (x_j >= y_j OR there exists i < j such that x_i > y_i)
        //                       AND X[1:j-1] >= Y[1:j-1]
        //
        // Because we are dealing with literals, we can simplify the comparison to:
        //     x > y <=> x AND !y    if x and y are not exclusive
        //     x > y <=> x           if x and y are exclusive
        //     x >= y <=> x OR !y    if x and y are not exclusive
        //     x >= y <=> !y         if x and y are exclusive
        for (i, crt_instance) in instances.iter().copied().enumerate() {
            let mut clause: Vec<Lit> = Vec::with_capacity(128);
            if i > 0 {
                let prv_instance = instances[i - 1];

                let mut previously_handled = HashSet::new();
                // the vector of (x_i, y_i, exclu_i)
                let mut vec = Vec::with_capacity(128);

                //println!("\n{} ===> {}", crt_instance, prv_instance);
                for &cl in &links {
                    if let Some(swapped) = swap(&cl, *crt_instance, *prv_instance) {
                        debug_assert_eq!(cl.eff.is_assign, swapped.eff.is_assign);
                        let exclusive = cl.eff.is_assign && cl.cond == swapped.cond;
                        if previously_handled.contains(&(swapped, cl)) {
                            // Lemma 1 of Rintannen et al, ECAI 2024
                            // there is no need to consider if we have already considered the reverse
                            continue;
                        }
                        if cl.eff.instance_id != *crt_instance && swapped.eff.instance_id != *crt_instance {
                            //only keep effects of the action
                            continue;
                        }
                        previously_handled.insert((cl, swapped));
                        let cl_lit = cls[&cl].active; // x_i
                        let swapped_lit = cls[&swapped].active; // y_i
                        let entry = (cl_lit, swapped_lit, exclusive);
                        vec.push(entry);
                        //println!("  {cl:?}  ->  {swapped:?}    {}", if exclusive { "EXCLU" } else { "" });
                        // println!("    {entry:?}");
                    } else {
                        // println!("{cl:?}");
                    }
                }

                for (j, (x_j, y_j, exclu_j)) in vec.iter().copied().enumerate() {
                    clause.clear(); // Stores the disjunction for the first part of X[1:j] >= Y[1:j]

                    // x_j >= y_j  <=>  x_j OR !y_j    (but only !y_j if x_j and y_j are exclusive)
                    clause.push(!y_j);
                    if !exclu_j {
                        // only necessary if  (not exclusive)
                        clause.push(x_j);
                    }

                    for &(x_i, y_i, exclu_i) in &vec[0..j] {
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

fn add_deprecated_plan_space_symmetry_breaking(pb: &FiniteProblem, model: &mut Model, encoding: &Encoding) {
    let discard_useless_supports = USELESS_SUPPORTS.get();
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

    struct ActionOrigin {
        template: usize,
        #[allow(unused)]
        gen: usize,
    }
    let actions: HashMap<ChronicleId, _> = pb
        .chronicles
        .iter()
        .enumerate()
        .filter_map(|(id, c)| match c.origin {
            ChronicleOrigin::FreeAction {
                template_id,
                generation_id,
            } => Some((
                id,
                ActionOrigin {
                    template: template_id,
                    gen: generation_id,
                },
            )),
            _ => None,
        })
        .collect();

    type TemplateID = usize;
    let templates = pb
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
        exclusive: bool,
    }
    let mut causal_link: HashMap<(ChronicleId, CondID), Link> = Default::default();
    let mut conds_by_templates: HashMap<TemplateID, HashSet<CondID>> = Default::default();
    for template in &templates {
        conds_by_templates.insert(*template, HashSet::new());
    }
    for &(k, v) in &encoding.tags {
        let Tag::Support(cond, eff) = k else {
            panic!("Unsupported tag: {k:?}");
        };
        if model.entails(!v) {
            continue; // link can never be achieved => ignore
        }
        let instance = eff.instance_id;
        let ch = &pb.chronicles[instance];
        let ChronicleOrigin::FreeAction { template_id, .. } = ch.origin else {
            continue;
        };
        if discard_useless_supports && !is_primary_support(cond, eff) {
            continue; // remove non-primary supports
        }
        // record that this template may contribute to this condition
        conds_by_templates.get_mut(&template_id).unwrap().insert(cond);
        // non-optional literal that is true iff the causal link is active
        let link_active = model.reify(and([v, model.presence_literal(v.variable())]));
        // list of outgoing causal links of the supporting action
        causal_link.insert(
            (eff.instance_id, cond),
            Link {
                active: link_active,
                exclusive: eff.is_assign,
            },
        );
    }

    let sort = |conds: HashSet<CondID>| {
        if sort_by_hierarchy_level {
            let sort_key = |c: &CondID| {
                // get the level, reserving the lvl 0 for non-templates
                if let Some(template) = template_id(c.instance_id) {
                    let lvl = pb.meta.action_hierarchy[&template];
                    (lvl + 1, c.instance_id)
                } else {
                    (0, c.instance_id)
                }
            };
            conds.into_iter().sorted_by_key(sort_key).collect_vec()
        } else {
            conds.into_iter().sorted().collect_vec()
        }
    };
    let conds_by_templates: HashMap<TemplateID, Vec<CondID>> =
        conds_by_templates.into_iter().map(|(k, v)| (k, sort(v))).collect();
    let supports = |ch: ChronicleId, cond: CondID| {
        causal_link.get(&(ch, cond)).copied().unwrap_or(Link {
            active: Lit::FALSE,
            exclusive: true,
        })
    };

    for template_id in &templates {
        let conditions = &conds_by_templates[template_id];
        let instances: Vec<_> = actions
            .iter()
            .filter_map(|(id, orig)| if orig.template == *template_id { Some(id) } else { None })
            .sorted()
            .collect();

        // // detailed printing for debugging
        // if let Some(ch) = instances.first() {
        //     let ch = &pb.chronicles[**ch];
        //     let s = format_partial_name(&ch.chronicle.name, model).unwrap();
        //     println!("{template_id} {s}   ({})", instances.len());
        //     for cond_id in conditions {
        //         print_cond(*cond_id, pb, model);
        //         println!();
        //     }
        // }

        // An instance of the template is allowed to support a condition only if the previous instance
        // supports a condition at an earlier level or at the same level.
        //
        // Noting X = [x_1, x_2, ..., x_n] where x_i <=> previous instance supports condition i
        // and    Y = [y_1, y_2, ..., y_n] where y_i <=> current instance supports condition i
        // We ensure that X >= Y in the lexicographic order.
        //
        // This is done recursively for 1 <= j <= n:
        //     X[1:j] >= Y[1:j] <=> (x_j >= y_j OR there exists i < j such that x_i > y_i)
        //                       AND X[1:j-1] >= Y[1:j-1]
        //
        // Because we are dealing with literals, we can simplify the comparison to:
        //     x > y <=> x AND !y    if x and y are not exclusive
        //     x > y <=> x           if x and y are exclusive
        //     x >= y <=> x OR !y    if x and y are not exclusive
        //     x >= y <=> !y         if x and y are exclusive
        for (i, crt_instance) in instances.iter().copied().enumerate() {
            let mut clause = Vec::with_capacity(conditions.len());
            if i > 0 {
                let prv_instance = instances[i - 1];

                for (j, crt_cond) in conditions.iter().enumerate() {
                    clause.clear(); // Stores the disjunction for the first part of X[1:j] >= Y[1:j]
                    let prv_link = supports(*prv_instance, *crt_cond); // x_j
                    let crt_link = supports(*crt_instance, *crt_cond); // y_j
                    clause.push(!crt_link.active); // x_j >= y_j
                    if !(crt_link.exclusive && prv_link.exclusive) {
                        // x_j >= y_j (not exclusive)
                        clause.push(prv_link.active);
                    }

                    for prv_cond in &conditions[0..j] {
                        let prv_link = supports(*prv_instance, *prv_cond); // x_k
                        let crt_link = supports(*crt_instance, *prv_cond); // y_k
                        if crt_link.exclusive && prv_link.exclusive {
                            // x_k > y_k (exclusive)
                            clause.push(prv_link.active);
                        } else {
                            // x_k > y_k (not exclusive)
                            clause.push(model.reify(and([prv_link.active, !crt_link.active])));
                        }
                    }

                    // (x_j >= y_j OR there exists i < j such that x_i > y_i)
                    model.enforce(or(clause.as_slice()), []);
                    // X[1:j-1] >= Y[1:j-1] has been enforced by the previous iteration of the loop
                }
            }
            clause.clear();
            if discard_useless_supports {
                // enforce that a chronicle be present only if it supports at least one condition
                clause.push(!pb.chronicles[*crt_instance].chronicle.presence);
                for cond in conditions {
                    clause.push(supports(*crt_instance, *cond).active);
                }
                model.enforce(or(clause.as_slice()), []);
            }
        }
    }

    // println!("\n================\n");
    // hierarchy(pb);
    // println!("\n================\n");
    // std::process::exit(1)
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
