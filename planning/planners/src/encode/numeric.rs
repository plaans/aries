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
    /// Whether both effects are statically temporally ordered
    /// (i.e. the first effect is before the second effect for sure)
    pub statically_ordered: bool,
}
impl BorrowPattern {
    pub fn new(fst_eff: Effect, snd_eff: Effect, presence: Lit) -> Self {
        // Same state variable
        debug_assert_eq!(fst_eff.state_var, snd_eff.state_var);
        // Oposite values
        debug_assert!(if let (EffectOp::Increase(fst_val), EffectOp::Increase(snd_val)) =
            (fst_eff.operation.clone(), snd_eff.operation.clone())
        {
            fst_val == -snd_val
        } else {
            false
        });
        // Instantaneous effects
        debug_assert_eq!(fst_eff.transition_start + FAtom::EPSILON, fst_eff.transition_end);
        debug_assert_eq!(snd_eff.transition_start + FAtom::EPSILON, snd_eff.transition_end);

        if fst_eff.transition_start < snd_eff.transition_start {
            Self {
                fst_eff,
                snd_eff,
                presence,
                statically_ordered: true,
            }
        } else if snd_eff.transition_start < fst_eff.transition_start {
            Self {
                fst_eff: snd_eff,
                snd_eff: fst_eff,
                presence,
                statically_ordered: true,
            }
        } else {
            Self {
                fst_eff,
                snd_eff,
                presence,
                statically_ordered: false,
            }
        }
    }

    pub fn state_var(&self) -> &StateVar {
        &self.fst_eff.state_var
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

    let mut borrows = Vec::new();
    if BORROW_PATTERN_CONSTRAINT.get() {
        borrows = find_borrow_patterns(pb);
        add_borrow_pattern_constraints(solver, pb, &borrows)?;
    }

    let conds = conditions(pb)
        .filter(|(_, _, cond)| is_numeric(&cond.state_var))
        .map(|(cond_id, prez, cond)| (cond_id, prez, cond.clone()))
        .chain(inc_conds)
        .collect_vec();
    add_condition_support_constraints(solver, encoding, eff_mutex_ends, &conds, &assigns, &incs, &borrows)?;

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
    borrows: &[BorrowPattern],
) -> Result<(), Conflict> {
    let span = tracing::span!(tracing::Level::TRACE, "numeric support");
    let _span = span.enter();
    let mut num_numeric_support_constraints = 0;

    for (cond_id, cond_prez, cond) in conds {
        debug_assert!(is_numeric(&cond.state_var));
        assert!(
            cond.start == cond.end,
            "Only instantaneous numerical conditions are supported"
        );
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
            add_condition_support_increase_contribution(
                solver,
                cond_prez,
                cond,
                &mut cond_val_sum,
                &ass_prez,
                ass,
                &mut inc_support,
                incs,
                borrows,
            );
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

#[allow(clippy::too_many_arguments)]
fn add_condition_support_increase_contribution(
    solver: &mut Solver,
    cond_prez: &Lit,
    cond: &Condition,
    cond_val_sum: &mut LinearSum,
    ass_prez: &Lit,
    ass: &Effect,
    inc_support: &mut HashMap<EffID, Vec<Lit>>,
    incs: &[(EffID, Lit, &Effect)],
    borrows: &[BorrowPattern],
) {
    // Assert !BORROW_PATTERN_CONSTRAINT => no borrows
    debug_assert!(BORROW_PATTERN_CONSTRAINT.get() || borrows.is_empty());

    // Only keep the increases that are not part of a borrow pattern or are the first increase of a borrow pattern
    // If BORROW_PATTERN_CONSTRAINT is false, every increases are not part of a borrow
    let increases = incs.iter().filter_map(|&(inc_id, inc_prez, inc)| {
        let fst_bp = borrows.iter().find(|bp| bp.fst_eff == *inc);
        let snd_bp = borrows.iter().find(|bp| bp.snd_eff == *inc);
        if fst_bp.is_none() && snd_bp.is_none() {
            // Not part of a borrow pattern
            Some((inc_id, inc_prez, inc, None))
        } else if fst_bp.is_some() && snd_bp.is_none() {
            // Is the first increase of a borrow pattern
            Some((inc_id, inc_prez, inc, fst_bp))
        } else {
            // Is the second increase of a borrow pattern
            // Or both fst and snd are present which should not happen
            None
        }
    });
    debug_assert!(BORROW_PATTERN_CONSTRAINT.get() || increases.clone().count() == incs.len());

    for (inc_id, inc_prez, inc, bp) in increases {
        if solver.model.entails(!inc_prez) {
            continue;
        }
        if solver.model.state.exclusive(*cond_prez, inc_prez) {
            continue;
        }
        if !unifiable_sv(&solver.model, &cond.state_var, &inc.state_var) {
            continue;
        }
        let mut active_inc_conjunction: Vec<Lit> = Vec::with_capacity(32);
        // the condition is present
        active_inc_conjunction.push(*cond_prez);
        // the assignment is present
        active_inc_conjunction.push(*ass_prez);
        // the increase is present
        active_inc_conjunction.push(inc_prez);

        // the condition is temporally supported by the borrow/increase
        // also retreive the value of the borrow/increase
        let inc_val = if let Some(bp) = bp {
            // The increase is part of a borrow pattern
            // The whole borrow should be after the assignment's transition end
            active_inc_conjunction.push(solver.reify(f_leq(ass.transition_end, bp.fst_eff.transition_start)));
            active_inc_conjunction.push(solver.reify(f_leq(ass.transition_end, bp.snd_eff.transition_start)));
            // The whole condition should be contained by the borrow
            if bp.statically_ordered {
                // CASE 1: We can statically order the timepoints of the borrow
                active_inc_conjunction.push(solver.reify(f_leq(bp.fst_eff.transition_end, cond.start)));
                active_inc_conjunction.push(solver.reify(f_leq(cond.end, bp.snd_eff.transition_start)));
                if let EffectOp::Increase(val) = bp.fst_eff.operation.clone() {
                    val
                } else {
                    unreachable!()
                }
            } else {
                // CASE 2: The timepoints cannot be statically ordered
                // CASE 2.1: The first effect of the borrow is effectively the first one
                let fst_before_snd = solver.reify(f_lt(bp.fst_eff.transition_end, bp.snd_eff.transition_start));
                let fst_before_cond = solver.reify(f_leq(bp.fst_eff.transition_end, cond.start));
                let cond_before_snd = solver.reify(f_leq(cond.end, bp.snd_eff.transition_start));
                let cond_is_contained = solver.reify(and([fst_before_cond, cond_before_snd]));
                active_inc_conjunction.push(solver.reify(implies(fst_before_snd, cond_is_contained)));
                let fst_val = if let EffectOp::Increase(val) = bp.fst_eff.operation.clone() {
                    val
                } else {
                    unreachable!()
                };
                let fst_val = linear_sum_mul_lit(&mut solver.model, fst_val, fst_before_snd);
                // CASE 2.2: The second effect of the borrow is in reality the first one
                let snd_before_fst = solver.reify(f_lt(bp.snd_eff.transition_end, bp.fst_eff.transition_start));
                let snd_before_cond = solver.reify(f_leq(bp.snd_eff.transition_end, cond.start));
                let cond_before_fst = solver.reify(f_leq(cond.end, bp.fst_eff.transition_start));
                let cond_is_contained = solver.reify(and([snd_before_cond, cond_before_fst]));
                active_inc_conjunction.push(solver.reify(implies(snd_before_fst, cond_is_contained)));
                let snd_val = if let EffectOp::Increase(val) = bp.snd_eff.operation.clone() {
                    val
                } else {
                    unreachable!()
                };
                let snd_val = linear_sum_mul_lit(&mut solver.model, snd_val, snd_before_fst);
                // The value of the increase is the sum of the two values
                fst_val + snd_val
            }
        } else {
            // The increase is outside a borrow pattern
            // It should be between the assignment's transition end and the condition's start
            active_inc_conjunction.push(solver.reify(f_leq(ass.transition_end, inc.transition_start)));
            active_inc_conjunction.push(solver.reify(f_leq(inc.transition_end, cond.start)));
            if let EffectOp::Increase(val) = inc.operation.clone() {
                val
            } else {
                unreachable!()
            }
        };

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
        *cond_val_sum += linear_sum_mul_lit(&mut solver.model, inc_val.clone(), active_inc);
    }
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
        .flat_map(|ch| {
            // Collect the increase effects of the chronicle that are candidates for borrow patterns.
            // Group them by state variable, then by groups of 2 effects such that the value of
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
                .flat_map(|effs| {
                    let mut effs = effs.clone();
                    let mut groups = Vec::new();
                    let mut new_group = true;
                    while new_group {
                        new_group = false;
                        for i in 0..effs.len() {
                            for j in (i + 1)..effs.len() {
                                if let (EffectOp::Increase(val1), EffectOp::Increase(val2)) =
                                    (effs[i].operation.clone(), effs[j].operation.clone())
                    {
                                    if val1 == -val2 {
                                        groups.push((effs[i].clone(), effs[j].clone()));
                                        new_group = true;
                                        effs.remove(j);
                                        effs.remove(i);
                                        break;
                                    }
                    } else {
                                    unreachable!();
                                }
                            }
                            if new_group {
                                break;
                            }
                        }
                    }
                    groups
                })
                .map(|effs| BorrowPattern::new(effs.0, effs.1, ch.chronicle.presence))
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

        let mut set_constraint = true;
        let mut sum = if let EffectOp::Increase(val) = p1.fst_eff.operation.clone() {
            val
        } else {
            unreachable!()
        };
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
            if p1.statically_ordered && p2.statically_ordered {
                contribution.push(solver.reify(f_leq(p2.fst_eff.transition_end, p1.fst_eff.transition_start)));
                contribution.push(solver.reify(f_lt(p1.fst_eff.transition_end, p2.snd_eff.transition_start)));
            } else {
                set_constraint = false;
                break;
            }
            // both patterns have the same state variable
            for idx in 0..p1.state_var().args.len() {
                let a = p1.state_var().args[idx];
                let b = p2.state_var().args[idx];
                contribution.push(solver.reify(eq(a, b)));
            }

            let contribution_lit = solver.reify(and(contribution));
            let p2_val = if let EffectOp::Increase(val) = p2.fst_eff.operation.clone() {
                val
            } else {
                unreachable!()
            };
            sum += linear_sum_mul_lit(&mut solver.model, p2_val, contribution_lit);
        }

        if set_constraint {
            solver.model.enforce(sum.clone().leq(ub), [p1.presence]);
            solver.model.enforce(sum.clone().geq(lb), [p1.presence]);
            num_borrow_patterns += 1;
        }
    }

    tracing::debug!(%num_borrow_patterns);
    solver.propagate()?;
    Ok(())
}
