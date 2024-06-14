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

fn supported_by_psp(metadata: &Metadata) -> bool {
    !metadata.class.is_hierarchical()
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
    let mut causal_link: HashMap<(ChronicleId, CondID), Lit> = Default::default();
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
        causal_link.insert((eff.instance_id, cond), link_active);
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
    let supports = |ch: ChronicleId, cond: CondID| causal_link.get(&(ch, cond)).copied().unwrap_or(Lit::FALSE);

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

        for (i, instance) in instances.iter().copied().enumerate() {
            let mut clause = Vec::with_capacity(64);
            if i > 0 {
                let prev = instances[i - 1];

                // the chronicle is allowed to support a condition only if the previous chronicle
                // supports a condition at an earlier level
                for (cond_index, cond) in conditions.iter().enumerate() {
                    clause.clear();
                    clause.push(!supports(*instance, *cond));

                    for prev_cond in &conditions[0..cond_index] {
                        clause.push(supports(*prev, *prev_cond));
                    }
                    model.enforce(or(clause.as_slice()), []);
                }
            }
            clause.clear();
            if discard_useless_supports {
                // enforce that a chronicle be present only if it supports at least one condition
                clause.push(!pb.chronicles[*instance].chronicle.presence);
                for cond in conditions {
                    clause.push(supports(*instance, *cond))
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
    let cond = &ch.chronicle.conditions[cid.cond_id];
    let s = model.shape.symbols.format(&[cond.state_var.fluent.sym]);
    print!("  {:?}:{}", ch.origin, s)
}
