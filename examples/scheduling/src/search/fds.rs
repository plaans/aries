// =============== Failure-directed search ========

use crate::search::Model;
use crate::Var;
use aries_backtrack::{Backtrack, DecLvl};
use aries_collections::id_map::IdMap;
use aries_collections::Next;
use aries_core::literals::Disjunction;
use aries_core::state::Explainer;
use aries_core::{IntCst, Lit, VarRef};
use aries_model::extensions::SavedAssignment;
use aries_solver::solver::search::{Decision, SearchControl};
use aries_solver::solver::stats::Stats;

#[derive(Clone)]
enum LastChoice {
    None,
    Dec { dec: Lit, num_unbound: usize, lvl: DecLvl },
}

#[derive(Clone)]
pub struct FDSBrancher {
    saved: DecLvl,
    vars: Vec<VarRef>,
    last: LastChoice,
    ratings: IdMap<VarRef, (f64, f64)>,
    lvl_avg: Vec<(f64, usize)>,
}

fn pos(v: VarRef) -> Lit {
    v.geq(1)
}
fn neg(v: VarRef) -> Lit {
    !pos(v)
}

const FDS_DECAY: f64 = 0.95;

impl FDSBrancher {
    pub fn new() -> Self {
        FDSBrancher {
            saved: DecLvl::ROOT,
            vars: Default::default(),
            last: LastChoice::None,
            ratings: Default::default(),
            lvl_avg: vec![],
        }
    }

    fn num_unbound(&self, model: &Model) -> usize {
        let mut cnt = 0;
        for v in &self.vars {
            if !model.state.is_bound(*v) {
                cnt += 1;
            }
        }
        cnt
    }

    fn into_global_rating(&mut self, local_rating: f64, lvl: DecLvl) -> f64 {
        let lvl = lvl.to_int() as usize;
        while lvl >= self.lvl_avg.len() {
            self.lvl_avg.push((0_f64, 0))
        }
        let (prev, inum) = self.lvl_avg[lvl];
        let num = inum as f64;
        let new = (prev * num + local_rating) / (num + 1_f64);
        self.lvl_avg[lvl] = (new, inum + 1);
        local_rating / new
    }

    fn set_rating(&mut self, lit: Lit, local_rating: f64, lvl: DecLvl) {
        let new_rating = self.into_global_rating(local_rating, lvl);
        let var = lit.variable();
        let (pos_rating, neg_rating) = self.ratings.get_mut(var).expect("Setting rating for unknown var");
        let rating = if lit == pos(var) {
            pos_rating
        } else {
            assert!(lit == neg(var));
            neg_rating
        };
        *rating = (FDS_DECAY * *rating) + (1_f64 - FDS_DECAY) * new_rating;
    }

    fn process_dec(&mut self, model: &Model) {
        if let LastChoice::Dec { dec, num_unbound, lvl } = self.last {
            let curr_lvl = model.state.current_decision_level();
            // assert!(curr_lvl == lvl.next() || curr_lvl == lvl); // in case of an asserted lit, we might be at the same level

            let unbound_after = self.num_unbound(model);
            // assert!(num_unbound > unbound_after);
            if (curr_lvl == lvl.next() || curr_lvl == lvl) && num_unbound > unbound_after {
                let bound_by_choice = num_unbound - unbound_after;
                // R = (2^num_unbound_after) / 2^num_unbound
                // R = 1 / (2^bound_by_choice)
                let rating = 1_f64 + (0.5_f64).powf(bound_by_choice as f64);
                // let rating = 1_f64;
                self.set_rating(dec, rating, lvl);
            }
        }
        self.last = LastChoice::None;
    }
    fn process_conflict(&mut self, _model: &Model) {
        if let LastChoice::Dec { dec, lvl, .. } = self.last {
            self.set_rating(dec, 0_f64, lvl);
        }
        self.last = LastChoice::None;
    }
}

impl SearchControl<Var> for FDSBrancher {
    fn import_vars(&mut self, model: &aries_model::Model<Var>) {
        if !self.vars.is_empty() {
            return;
        }
        for v in model.state.variables() {
            if let Some(Var::Prec(_, _, _, _)) = model.shape.labels.get(v) {
                self.vars.push(v);
                self.ratings.insert(v, (1.0, 1.0));
            }
        }
    }
    fn next_decision(&mut self, _stats: &Stats, model: &Model) -> Option<Decision> {
        self.process_dec(model);
        // println!("unbound: {}", self.num_unbound(model));
        let mut best_lit = None;
        let mut best_score = f64::INFINITY;
        for &v in &self.vars {
            if !model.state.is_bound(v) {
                let (pos_score, neg_score) = self.ratings[v];
                let var_score = pos_score + neg_score;
                if var_score < best_score {
                    best_score = var_score;
                    best_lit = Some(if pos_score < neg_score { pos(v) } else { neg(v) });
                }
            }
        }
        if let Some(dec) = best_lit {
            // println!("{:?} DEC {:?}", model.state.current_decision_level(), dec);
            self.last = LastChoice::Dec {
                dec,
                num_unbound: self.num_unbound(model),
                lvl: model.state.current_decision_level(),
            };
            Some(Decision::SetLiteral(dec))
        } else {
            for v in model.state.variables() {
                if !model.state.is_bound(v) {
                    let dec = Lit::leq(v, model.state.lb(v));
                    // println!("DEC {:?}", dec);
                    return Some(Decision::SetLiteral(dec));
                }
            }
            None
        }
    }

    /// Notifies the search control that a new assignment has been found (either if itself or by an other solver running in parallel).
    fn new_assignment_found(&mut self, _objective_value: IntCst, _assignment: std::sync::Arc<SavedAssignment>) {
        // println!("SOL");
    }

    /// Invoked by search when facing a conflict in the search
    fn conflict(&mut self, _clause: &Disjunction, model: &Model, _: &mut dyn Explainer) {
        // println!("\tconflict {:?}", clause);
        self.process_conflict(model);
        self.last = LastChoice::None;
    }
    fn asserted_after_conflict(&mut self, lit: Lit, model: &Model) {
        // println!("{:?} ASS {:?}", model.state.current_decision_level(), lit);

        assert!(matches!(self.last, LastChoice::None));
        // ignore if this is not a variable on which we might choose, ignore
        if self.ratings.contains_key(lit.variable()) {
            self.last = LastChoice::Dec {
                dec: lit,
                num_unbound: self.num_unbound(model),
                lvl: model.state.current_decision_level(),
            };
        }
    }

    fn clone_to_box(&self) -> Box<dyn SearchControl<Var> + Send> {
        Box::new(self.clone())
    }
}

impl Backtrack for FDSBrancher {
    fn save_state(&mut self) -> DecLvl {
        self.saved += 1;
        self.saved
    }

    fn num_saved(&self) -> u32 {
        self.saved.to_int()
    }

    fn restore_last(&mut self) {
        self.saved -= 1;
    }
}
