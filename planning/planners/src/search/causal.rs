use crate::encoding::{CondID, EffID, Encoding, Tag};
use crate::fmt::format_partial_name;
use crate::Model;
use aries::backtrack::{Backtrack, DecLvl, DecisionLevelTracker};
use aries::model::extensions::AssignmentExt;
use aries::solver::search::{Decision, SearchControl};
use aries::solver::stats::Stats;
use aries_planning::chronicles::{ChronicleInstance, Condition, Effect, FiniteProblem, VarLabel};
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone)]
pub struct CausalSearch {
    problem: Arc<FiniteProblem>,
    encoding: Arc<Encoding>,
    dec_lvl: DecisionLevelTracker,
}

impl CausalSearch {
    pub fn new(pb: Arc<FiniteProblem>, encoding: Arc<Encoding>) -> CausalSearch {
        CausalSearch {
            problem: pb,
            encoding,
            dec_lvl: DecisionLevelTracker::default(),
        }
    }

    fn ch(&self, chronicle_id: usize) -> &ChronicleInstance {
        &self.problem.chronicles[chronicle_id]
    }
    fn cond(&self, cond_id: CondID) -> &Condition {
        &self.ch(cond_id.instance_id).chronicle.conditions[cond_id.cond_id]
    }
    fn eff(&self, eff_id: EffID) -> &Effect {
        &self.ch(eff_id.instance_id).chronicle.effects[eff_id.eff_id]
    }
}

impl SearchControl<VarLabel> for CausalSearch {
    fn next_decision(&mut self, _stats: &Stats, model: &Model) -> Option<Decision> {
        let mut options = Vec::new();
        let mut prev = Option::None;

        let supported = |cond: CondID| {
            self.encoding.tags.iter().any(|(tag, lit)| {
                let Tag::Support(c, _) = *tag;
                c == cond && model.entails(*lit)
            })
        };

        let pending_conditions: HashSet<_> = self
            .encoding
            .tags
            .iter()
            .map(|t| {
                let Tag::Support(cond, _eff) = t.0;
                cond
            })
            .filter(|c| model.entails(self.ch(c.instance_id).chronicle.presence))
            .filter(|c| !supported(*c))
            .collect();

        for &(tag, lig) in &self.encoding.tags {
            let Tag::Support(cond, eff) = tag;
            if !pending_conditions.contains(&cond) {
                continue;
            }

            if model.entails(self.ch(cond.instance_id).chronicle.presence) {
                if prev != Some(cond) {
                    println!();
                    println!(
                        "{}",
                        format_partial_name(&self.ch(cond.instance_id).chronicle.name, model).unwrap()
                    );
                    println!("  {}", format_partial_name(&self.cond(cond).state_var, model).unwrap());
                    prev = Some(cond)
                }

                if model.entails(!lig) {
                    continue;
                }
                if model.entails(lig) {
                    print!("    + ")
                } else {
                    print!("    ? {}  ", options.len());
                    options.push(lig);
                }
                print!(" {}", format_partial_name(&self.eff(eff).state_var, model).unwrap());
                println!(
                    "   / [{}] {}",
                    eff.instance_id,
                    format_partial_name(&self.ch(eff.instance_id).chronicle.name, model).unwrap()
                );
            }
        }
        if options.is_empty() {
            return None;
        }
        let mut answer = String::new();
        // println!("> ");
        // std::io::stdin().read_line(&mut answer).unwrap();
        // println!("ANSWER: '{answer}'");
        let id: usize = answer.trim().parse().unwrap_or(0);
        println!("\n\n\n");
        return Some(Decision::SetLiteral(options[id]));
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<VarLabel> + Send> {
        Box::new(self.clone())
    }
}

impl Backtrack for CausalSearch {
    fn save_state(&mut self) -> DecLvl {
        self.dec_lvl.save_state()
    }

    fn num_saved(&self) -> u32 {
        self.dec_lvl.num_saved()
    }

    fn restore_last(&mut self) {
        self.dec_lvl.restore_last()
    }
}
