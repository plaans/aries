use crate::encoding::{CondID, EffID, Encoding, Tag};
use crate::fmt::format_partial_name;
use crate::Model;
use aries::backtrack::{Backtrack, DecLvl, DecisionLevelTracker};
use aries::model::extensions::AssignmentExt;
use aries::model::lang::{SAtom, Type};
use aries::model::symbols::TypedSym;
use aries::solver::search::{Decision, SearchControl};
use aries::solver::stats::Stats;
use aries_planning::chronicles::{ChronicleInstance, Condition, Effect, FiniteProblem, StateVar, VarLabel};
use env_param::EnvParam;
use std::collections::HashSet;
use std::sync::Arc;

static PRINT_CAUSAL: EnvParam<bool> = EnvParam::new("ARIES_PRINT_CAUSAL", "false");

/// If the `PRINT_CAUSAL` parameter is true, then this brancher will prompt the user for a
/// a choice among potential supporters for pending conditions.
#[derive(Clone)]
pub struct ManualCausalSearch {
    problem: Arc<FiniteProblem>,
    encoding: Arc<Encoding>,
    dec_lvl: DecisionLevelTracker,
}

impl ManualCausalSearch {
    pub fn new(pb: Arc<FiniteProblem>, encoding: Arc<Encoding>) -> ManualCausalSearch {
        ManualCausalSearch {
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

impl SearchControl<VarLabel> for ManualCausalSearch {
    fn next_decision(&mut self, _stats: &Stats, model: &Model) -> Option<Decision> {
        let print = PRINT_CAUSAL.get();
        if !print {
            return None;
        }
        let mut options = Vec::new();
        let mut prev = None;

        let supported = |cond: CondID| {
            self.encoding.tags.iter().any(|(tag, lit)| {
                if let Tag::Support(c, _) = *tag {
                    c == cond && model.entails(*lit)
                } else {
                    false
                }
            })
        };

        let pending_conditions: HashSet<_> = self
            .encoding
            .tags
            .iter()
            .filter_map(|t| {
                if let Tag::Support(cond, _eff) = t.0 {
                    Some(cond)
                } else {
                    None
                }
            })
            .filter(|c| model.entails(self.ch(c.instance_id).chronicle.presence))
            .filter(|c| !supported(*c))
            .collect();

        for &(tag, lig) in &self.encoding.tags {
            let Tag::Support(cond, eff) = tag else  {continue };
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
                    println!(
                        "  {}",
                        format_partial_name(&state_var_signature(&self.cond(cond).state_var), model).unwrap()
                    );

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

                print!(
                    " {}",
                    format_partial_name(&state_var_signature(&self.eff(eff).state_var), model).unwrap()
                );
                println!(
                    "   / [{}] {}",
                    eff.instance_id,
                    format_partial_name(&self.ch(eff.instance_id).chronicle.name, model).unwrap()
                );
            }
        }
        if !options.is_empty() {
            let mut answer = String::new();
            print!("> ");
            std::io::stdin().read_line(&mut answer).unwrap();
            if let Ok(id) = answer.trim().parse::<usize>() {
                println!("\n\n\n");
                return Some(Decision::SetLiteral(options[id]));
            }
        }

        None
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<VarLabel> + Send> {
        Box::new(self.clone())
    }
}

fn state_var_signature(sv: &StateVar) -> Vec<SAtom> {
    let fluent = SAtom::Cst(TypedSym {
        sym: sv.fluent.sym,
        tpe: if let Type::Sym(tpe) = sv.fluent.return_type() {
            tpe
        } else {
            panic!()
        },
    });
    let mut sv_sig: Vec<SAtom> = vec![fluent];
    sv_sig.extend_from_slice(&sv.args);
    sv_sig
}

impl Backtrack for ManualCausalSearch {
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
