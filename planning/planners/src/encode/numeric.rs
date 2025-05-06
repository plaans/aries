use std::collections::HashMap;

use crate::encode::*;
use crate::encoding::*;
use crate::Solver;
use aries::core::state::Conflict;
use aries::core::*;
use aries::model::extensions::AssignmentExt;
use aries::model::lang::FAtom;
use aries::model::lang::{expr::*, Atom, Type};
use aries_planning::chronicles::*;
use itertools::Itertools;

/// Parameter that activates additional constraints for borrow patterns.
pub static BORROW_PATTERN_CONSTRAINT: EnvParam<bool> = EnvParam::new("ARIES_BORROW_PATTERN_CONSTRAINT", "true");

/// A borrow pattern is a pattern where a state variable is decreased by x at the start of a
/// chronicle and then increased by x at the end of the chronicle.
#[derive(Eq, PartialEq, Clone, Debug)]
struct BorrowPattern {
    /// The first effect of the borrow pattern
    pub fst_eff: Effect,
    /// The second effect of the borrow pattern
    pub snd_eff: Effect,
    /// The presence of the borrow pattern
    pub presence: Lit,
}
impl BorrowPattern {
    pub fn new(fst_eff: Effect, snd_eff: Effect, presence: Lit) -> Self {
        assert_eq!(fst_eff.state_var, snd_eff.state_var);
        assert!(if let (EffectOp::Increase(fst_val), EffectOp::Increase(snd_val)) =
            (fst_eff.operation.clone(), snd_eff.operation.clone())
        {
            fst_val == -snd_val
        } else {
            false
        });
        Self {
            fst_eff,
            snd_eff,
            presence,
        }
    }

    pub fn state_var(&self) -> &StateVar {
        &self.fst_eff.state_var
    }

    pub fn value(&self) -> LinearSum {
        if let EffectOp::Increase(fst_val) = self.fst_eff.operation.clone() {
            fst_val
        } else {
            unreachable!()
        }
    }

    pub fn transition_start(&self) -> FAtom {
        if self.fst_eff.transition_start < self.snd_eff.transition_start {
            self.fst_eff.transition_start
        } else {
            self.snd_eff.transition_start
        }
    }

    pub fn transition_end(&self) -> FAtom {
        if self.fst_eff.transition_end > self.snd_eff.transition_end {
            self.fst_eff.transition_end
        } else {
            self.snd_eff.transition_end
        }
    }
}

pub fn add_numeric_constraints(
    solver: &mut Solver,
    pb: &FiniteProblem,
    encoding: &mut Encoding,
    eff_mutex_ends: &HashMap<EffID, FVar>,
) -> Result<(), Conflict> {
    let assigns = assignments(pb).collect_vec();
    add_assignment_coherence_constraints(solver, &assigns)?;

    let incs = increases(pb).collect_vec();
    let inc_conds = get_increase_coherence_conditions(solver, &incs)?;

    let conds = conditions(pb)
        .filter(|(_, _, cond)| is_numeric(&cond.state_var))
        .map(|(cond_id, prez, cond)| (cond_id, prez, cond.clone()))
        .chain(inc_conds)
        .collect_vec();
    add_condition_support_constraints(solver, encoding, eff_mutex_ends, &conds, &assigns, &incs)?;

    if BORROW_PATTERN_CONSTRAINT.get() {
        let borrow_patterns = find_borrow_patterns(pb);
        add_borrow_pattern_constraints(solver, pb, &borrow_patterns)?;
    }

    Ok(())
}

fn add_assignment_coherence_constraints(
    solver: &mut Solver,
    assigns: &[(EffID, Lit, &Effect)],
) -> Result<(), Conflict> {
    let span = tracing::span!(tracing::Level::TRACE, "numeric assignment coherence");
    let _span = span.enter();
    let mut num_numeric_assignment_coherence_constraints = 0;

    for &(_, prez, ass) in assigns {
        if !is_numeric(&ass.state_var) {
            continue;
        }
        let Type::Int { lb, ub } = ass.state_var.fluent.return_type() else {
            unreachable!()
        };
        let EffectOp::Assign(val) = ass.operation else {
            unreachable!()
        };
        if let Atom::Int(val) = val {
            solver.enforce(geq(val, lb), [prez]);
            solver.enforce(leq(val, ub), [prez]);
        } else if let Atom::Fixed(val) = val {
            solver.enforce(f_geq(val, FAtom::new((lb * val.denom).into(), val.denom)), [prez]);
            solver.enforce(f_leq(val, FAtom::new((ub * val.denom).into(), val.denom)), [prez]);
        } else {
            unreachable!();
        }
        num_numeric_assignment_coherence_constraints += 1;
    }

    tracing::debug!(%num_numeric_assignment_coherence_constraints);
    solver.propagate()?;
    Ok(())
}

fn get_increase_coherence_conditions(
    solver: &mut Solver,
    incs: &[(EffID, Lit, &Effect)],
) -> Result<Vec<(CondID, Lit, Condition)>, Conflict> {
    let span = tracing::span!(tracing::Level::TRACE, "numeric increase coherence");
    let _span = span.enter();
    let mut num_numeric_increase_coherence_constraints = 0;

    let mut increase_coherence_conditions: Vec<(CondID, Lit, Condition)> = Vec::with_capacity(incs.len());
    for &(inc_id, prez, inc) in incs {
        assert!(is_numeric(&inc.state_var));
        assert!(
            inc.transition_start + FAtom::EPSILON == inc.transition_end && inc.min_mutex_end.is_empty(),
            "Only instantaneous increases are supported"
        );

        let Type::Int { lb, ub } = inc.state_var.fluent.return_type() else {
            unreachable!()
        };

        if lb == INT_CST_MIN && ub == INT_CST_MAX {
            continue;
        }
        let var = solver
            .model
            .new_ivar(lb, ub, Container::Instance(inc_id.instance_id) / VarType::Reification);
        // Check that the state variable value is equals to the new variable `var`.
        // It will force the state variable to be in the bounds of the new variable after the increase.
        increase_coherence_conditions.push((
            CondID::new_post_increase(inc_id.instance_id, inc_id.eff_id),
            prez,
            Condition {
                start: inc.transition_end,
                end: inc.transition_end,
                state_var: inc.state_var.clone(),
                value: var.into(),
            },
        ));
        num_numeric_increase_coherence_constraints += 1;
    }

    tracing::debug!(%num_numeric_increase_coherence_constraints);
    solver.propagate()?;
    Ok(increase_coherence_conditions)
}

fn add_condition_support_constraints(
    solver: &mut Solver,
    encoding: &mut Encoding,
    eff_mutex_ends: &HashMap<EffID, FVar>,
    conds: &[(CondID, Lit, Condition)],
    assigns: &[(EffID, Lit, &Effect)],
    incs: &[(EffID, Lit, &Effect)],
) -> Result<(), Conflict> {
    let span = tracing::span!(tracing::Level::TRACE, "numeric support");
    let _span = span.enter();
    let mut num_numeric_support_constraints = 0;

    for (cond_id, cond_prez, cond) in conds {
        // skip conditions on non-numeric state variables, they have already been supported by support constraints
        assert!(is_numeric(&cond.state_var));
        if solver.model.entails(!*cond_prez) {
            continue;
        }
        let cond_val = match cond.value {
            Atom::Int(val) => FAtom::new(val, 1),
            Atom::Fixed(val) => val,
            _ => unreachable!(),
        };
        let mut supported: Vec<Lit> = Vec::with_capacity(128);
        let mut inc_support: HashMap<EffID, Vec<Lit>> = HashMap::new();

        for &(ass_id, ass_prez, ass) in assigns {
            if solver.model.entails(!ass_prez) {
                continue;
            }
            if solver.model.state.exclusive(*cond_prez, ass_prez) {
                continue;
            }
            if !unifiable_sv(&solver.model, &cond.state_var, &ass.state_var) {
                continue;
            }
            let EffectOp::Assign(ass_val) = ass.operation else {
                unreachable!()
            };
            let Atom::Int(ass_val) = ass_val else { unreachable!() };
            let mut supported_by_conjunction: Vec<Lit> = Vec::with_capacity(32);
            // the condition is present
            supported_by_conjunction.push(*cond_prez);
            // the assignment is present
            supported_by_conjunction.push(ass_prez);
            // the assignment's persistence contains the condition
            supported_by_conjunction.push(solver.reify(f_leq(ass.transition_end, cond.start)));
            supported_by_conjunction.push(solver.reify(f_leq(cond.end, eff_mutex_ends[&ass_id])));
            // the assignment and the condition have the same state variable
            for idx in 0..cond.state_var.args.len() {
                let a = cond.state_var.args[idx];
                let b = ass.state_var.args[idx];
                supported_by_conjunction.push(solver.reify(eq(a, b)));
            }

            // compute the supported by literal
            let supported_by = solver.reify(and(supported_by_conjunction));
            if solver.model.entails(!supported_by) {
                continue;
            }
            encoding.tag(supported_by, Tag::Support(*cond_id, ass_id));

            // the expected condition value
            let mut cond_val_sum = LinearSum::from(ass_val) - cond_val;

            for &(inc_id, inc_prez, inc) in incs {
                if solver.model.entails(!inc_prez) {
                    continue;
                }
                if solver.model.state.exclusive(*cond_prez, inc_prez) {
                    continue;
                }
                if !unifiable_sv(&solver.model, &cond.state_var, &inc.state_var) {
                    continue;
                }
                let EffectOp::Increase(inc_val) = inc.operation.clone() else {
                    unreachable!()
                };
                let mut active_inc_conjunction: Vec<Lit> = Vec::with_capacity(32);
                // the condition is present
                active_inc_conjunction.push(*cond_prez);
                // the assignment is present
                active_inc_conjunction.push(ass_prez);
                // the increase is present
                active_inc_conjunction.push(inc_prez);
                // the increase is after the assignment's transition end
                active_inc_conjunction.push(solver.reify(f_leq(ass.transition_end, inc.transition_start)));
                // the increase is before the condition's start
                active_inc_conjunction.push(solver.reify(f_leq(inc.transition_end, cond.start)));
                // TODO: If borrow pattern: cond.start <= borrow.end
                // the increase and the condition have the same state variable
                for idx in 0..cond.state_var.args.len() {
                    let a = cond.state_var.args[idx];
                    let b = inc.state_var.args[idx];
                    active_inc_conjunction.push(solver.reify(eq(a, b)));
                }
                // each term of the increase value is present
                for term in inc_val.terms() {
                    let p = solver.model.presence_literal(term.var());
                    active_inc_conjunction.push(p);
                }
                // compute wether the increase is active in the condition value
                let active_inc = solver.reify(and(active_inc_conjunction));
                if solver.model.entails(!active_inc) {
                    continue;
                }
                inc_support.entry(inc_id).or_default().push(active_inc);
                for term in inc_val.terms() {
                    // compute some static implication for better propagation
                    let p = solver.model.presence_literal(term.var());
                    if !solver.model.entails(p) {
                        solver.model.state.add_implication(active_inc, p);
                    }
                }
                cond_val_sum += linear_sum_mul_lit(&mut solver.model, inc_val.clone(), active_inc);
            }

            // enforce the condition value to be the sum of the assignment values and the increase values
            for term in cond_val_sum.terms() {
                // compute some static implication for better propagation
                let p = solver.model.presence_literal(term.var());
                if !solver.model.entails(p) {
                    solver.model.state.add_implication(supported_by, p);
                }
            }
            let cond_val_sum = linear_sum_mul_lit(&mut solver.model, cond_val_sum, supported_by);
            solver.model.enforce(cond_val_sum.clone().leq(0), [*cond_prez]);
            solver.model.enforce(cond_val_sum.clone().geq(0), [*cond_prez]);

            // add the support literal to the support clause
            supported.push(supported_by);
            num_numeric_support_constraints += 1;
        }

        for (inc_id, inc_support) in inc_support {
            let supported_by_inc = solver.reify(or(inc_support));
            encoding.tag(supported_by_inc, Tag::Support(*cond_id, inc_id));
        }

        solver.enforce(or(supported), [*cond_prez]);
    }

    tracing::debug!(%num_numeric_support_constraints);
    solver.propagate()?;
    Ok(())
}

fn find_borrow_patterns(pb: &FiniteProblem) -> Vec<BorrowPattern> {
    // Borrow patterns are patterns where a state variable is decreased by x at the start of a
    // chronicle and then increased by x at the end of the chronicle.
    // Morevoer, the state variable is assigned only at the initial state.

    // Find the fluents that are candidates for borrow patterns.
    // A fluent is a candidate for a borrow pattern if the only assignment is done at the initial state.
    let fluents_with_assign_out_init = pb
        .chronicles
        .iter()
        .filter(|ch| ch.origin != ChronicleOrigin::Original)
        .flat_map(|ch| ch.chronicle.effects.iter())
        .filter(|eff| matches!(eff.operation, EffectOp::Assign(_)))
        .map(|eff| eff.state_var.fluent.clone())
        .collect_vec();
    let candidate_fluents = pb
        .chronicles
        .iter()
        .flat_map(|ch| ch.chronicle.effects.iter())
        .map(|eff| eff.state_var.fluent.clone())
        .filter(|f| !fluents_with_assign_out_init.contains(f))
        .collect_vec();

    // Collect all the borrow patterns from the chronicles.
    pb.chronicles
        .iter()
        .filter(|ch| ch.origin != ChronicleOrigin::Original)
        .flat_map(|ch| {
            // Collect the increase effects of the chronicle that are candidates for borrow patterns.
            // Group them by state variable, and only keep the groups with 2 effects such that the value of
            // the second effect is the negative of the first effect.
            // The resulting group represents a borrow pattern.
            ch.chronicle
                .effects
                .iter()
                .filter(|eff| matches!(eff.operation, EffectOp::Increase(_)))
                .filter(|eff| candidate_fluents.contains(&eff.state_var.fluent))
                .fold(BTreeMap::<StateVar, Vec<_>>::new(), |mut acc, eff| {
                    acc.entry(eff.state_var.clone()).or_default().push(eff);
                    acc
                })
                .into_values()
                .filter(|effs| effs.len() == 2)
                .filter(|effs| {
                    if let (EffectOp::Increase(fst_val), EffectOp::Increase(snd_val)) =
                        (effs[0].operation.clone(), effs[1].operation.clone())
                    {
                        fst_val == -snd_val
                    } else {
                        false
                    }
                })
                .map(|effs| {
                    let (fst_eff, snd_eff) = (effs[0].clone(), effs[1].clone());
                    BorrowPattern::new(fst_eff, snd_eff, ch.chronicle.presence)
                })
                .collect_vec()
        })
        .collect_vec()
}

fn add_borrow_pattern_constraints(
    solver: &mut Solver,
    pb: &FiniteProblem,
    borrow_patterns: &[BorrowPattern],
) -> Result<(), Conflict> {
    let span = tracing::span!(tracing::Level::TRACE, "borrow patterns");
    let _span = span.enter();
    let mut num_borrow_patterns = 0;

    let initial_values_map = pb
        .chronicles
        .iter()
        .filter(|ch| ch.origin == ChronicleOrigin::Original)
        .flat_map(|ch| ch.chronicle.effects.iter())
        .filter(|eff| matches!(eff.operation, EffectOp::Assign(_)))
        .filter(|eff| eff.state_var.fluent.return_type().is_numeric())
        .map(|eff| {
            if let EffectOp::Assign(val) = eff.operation {
                (eff.state_var.clone(), val.int_view().unwrap())
            } else {
                unreachable!()
            }
        })
        .collect::<BTreeMap<_, _>>();

    // For each borrow pattern, create a post-decrease condition representing the contribution of the
    // different borrow patterns over this state variable.
    for p1 in borrow_patterns {
        if solver.model.entails(!p1.presence) {
            continue;
        }

        let Type::Int { lb, ub } = p1.state_var().fluent.return_type() else {
            unreachable!()
        };
        if lb == INT_CST_MIN && ub == INT_CST_MAX {
            continue;
        }

        let mut sum = p1.value().clone();
        sum += *initial_values_map.get(p1.state_var()).unwrap();
        for p2 in borrow_patterns {
            if ptr::eq(p1, p2) {
                continue;
            }
            if solver.model.entails(!p2.presence) {
                continue;
            }
            if solver.model.state.exclusive(p1.presence, p2.presence) {
                continue;
            }
            if !unifiable_sv(&solver.model, p1.state_var(), p2.state_var()) {
                continue;
            }

            let mut contribution: Vec<Lit> = Vec::with_capacity(32);
            // both patterns are present
            contribution.push(p1.presence);
            contribution.push(p2.presence);
            // the second pattern contains the first pattern start
            contribution.push(solver.reify(f_leq(p2.transition_start(), p1.transition_start())));
            contribution.push(solver.reify(f_lt(p1.transition_start(), p2.transition_end())));
            // both patterns have the same state variable
            for idx in 0..p1.state_var().args.len() {
                let a = p1.state_var().args[idx];
                let b = p2.state_var().args[idx];
                contribution.push(solver.reify(eq(a, b)));
            }

            let contribution_lit = solver.reify(and(contribution));
            sum += linear_sum_mul_lit(&mut solver.model, p2.value(), contribution_lit);
        }

        solver.model.enforce(sum.clone().leq(ub), [p1.presence]);
        solver.model.enforce(sum.clone().geq(lb), [p1.presence]);
        num_borrow_patterns += 1;
    }

    tracing::debug!(%num_borrow_patterns);
    solver.propagate()?;
    Ok(())
}
