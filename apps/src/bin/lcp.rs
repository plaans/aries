#![allow(unreachable_code, unused_mut, dead_code, unused_variables, unused_imports)] // TODO: remove
#![allow(clippy::all)]

use anyhow::*;

use aries_planning::chronicles::{Condition, Effect, FiniteProblem, Time, VarKind};

use aries_collections::ref_store::{Ref, RefVec};
use aries_planning::chronicles::constraints::ConstraintType;
use aries_sat::all::{BVar, Lit};
use aries_sat::SatProblem;
use aries_smt::solver::SMTSolver;
use aries_smt::*;
use aries_tnet::stn::{Edge, IncSTN, Timepoint};
use aries_tnet::*;
use std::collections::HashMap;
use std::path::Path;
use structopt::StructOpt;

/// Generates chronicles from a PDDL problem specification.
#[derive(Debug, StructOpt)]
#[structopt(name = "lcp", rename_all = "kebab-case")]
struct Opt {
    /// File containing a JSON encoding of the finite problem to solve.
    problem: String,
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Copy, Clone)]
enum Var {
    Boolean(Lit, Timepoint),
    Integer(Timepoint),
}

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();
    let _start_time = std::time::Instant::now();

    let problem_file = Path::new(&opt.problem);
    ensure!(
        problem_file.exists(),
        "Problem file {} does not exist",
        problem_file.display()
    );

    let json = std::fs::read_to_string(problem_file)?;
    let pb: FiniteProblem<usize> = serde_json::from_str(&json)?;

    let (mut solver, cor) = encode(&pb)?;

    if let Some(model) = solver.solve_eager() {
        println!("SOLUTION FOUND");
        print(&pb, &solver, &cor);
    } else {
        println!("NO SOLUTION");
    }

    Ok(())
}

fn print(problem: &FiniteProblem<usize>, solver: &SMT, cor: &RefVec<usize, Var>) {
    let domain = |v: Var| match v {
        Var::Boolean(_, i) => (solver.theory.lb(i), solver.theory.ub(i)),
        Var::Integer(i) => (solver.theory.lb(i), solver.theory.ub(i)),
    };
    let fmt_time = |t: Time<usize>| {
        let (lb, ub) = domain(cor[t.time_var]);
        if lb <= ub {
            format!("{}", lb + t.delay)
        } else {
            "NONE".to_string()
        }
    };
    let fmt_var = |v: usize| {
        let (lb, ub) = domain(cor[v]);
        if lb == ub {
            format!("{}", lb)
        } else if lb < ub {
            format!("[{}, {}]", lb, ub)
        } else {
            "NONE".to_string()
        }
    };

    for (instance_id, instance) in problem.chronicles.iter().enumerate() {
        println!(
            "INSTANCE {}: present: {}",
            instance_id,
            fmt_var(instance.chronicle.presence)
        );
        println!("  EFFECTS:");
        for effect in &instance.chronicle.effects {
            print!(
                "    ]{}, {}] ",
                fmt_time(effect.transition_start),
                fmt_time(effect.persistence_start)
            );
            for &x in &effect.state_var {
                print!("{} ", fmt_var(x))
            }
            println!(":= {}", fmt_var(effect.value))
        }
        println!("  CONDITIONS: ");
        for conditions in &instance.chronicle.conditions {
            print!("    [{}, {}] ", fmt_time(conditions.start), fmt_time(conditions.end));
            for &x in &conditions.state_var {
                print!("{} ", fmt_var(x))
            }
            println!("= {}", fmt_var(conditions.value))
        }
    }
}

type SMT = SMTSolver<Edge<i32>, IncSTN<i32>>;

fn effects<X: Ref>(pb: &FiniteProblem<X>) -> impl Iterator<Item = (X, &Effect<X>)> {
    pb.chronicles
        .iter()
        .flat_map(|ch| ch.chronicle.effects.iter().map(move |eff| (ch.chronicle.presence, eff)))
}

fn conditions<X: Ref>(pb: &FiniteProblem<X>) -> impl Iterator<Item = (X, &Condition<X>)> {
    pb.chronicles.iter().flat_map(|ch| {
        ch.chronicle
            .conditions
            .iter()
            .map(move |cond| (ch.chronicle.presence, cond))
    })
}

const ORIGIN: i32 = 0;
const HORIZON: i32 = 999999;

fn encode(pb: &FiniteProblem<usize>) -> anyhow::Result<(SMT, RefVec<usize, Var>)> {
    let mut smt = SMT::default();

    let mut cor = RefVec::new();
    let mut cor_back = HashMap::new();

    let true_timepoint = smt.theory.add_timepoint(1, 1);
    let false_timepoint = smt.theory.add_timepoint(0, 0);

    for (v, meta) in pb.variables.entries() {
        match meta.domain.kind {
            VarKind::Boolean => {
                let bool_var = if meta.domain.min == meta.domain.max {
                    if meta.domain.min == 0 {
                        // false
                        Var::Boolean(smt.contradiction(), false_timepoint)
                    } else {
                        assert!(meta.domain.min == 1);
                        Var::Boolean(smt.tautology(), true_timepoint)
                    }
                } else {
                    let tp = smt.theory.add_timepoint(0, 1);
                    let ge_one = aries_tnet::min_delay(smt.theory.origin(), tp, 1);
                    let lit = smt.literal_of(ge_one);
                    Var::Boolean(lit, tp)
                };
                cor.set_next(v, bool_var);
                cor_back.insert(bool_var, v);
            }
            _ => {
                let ivar = smt.theory.add_timepoint(meta.domain.min, meta.domain.max);
                cor.set_next(v, Var::Integer(ivar));
                cor_back.insert(Var::Integer(ivar), v);
            }
        }
    }

    let bool = |x| match cor[x] {
        Var::Boolean(y, _) => y,
        Var::Integer(_) => panic!(),
    };
    let int = |x| match cor[x] {
        Var::Boolean(_, i) => i,
        Var::Integer(i) => i,
    };
    let neq = |smt: &mut SMT, a, b| {
        let clause = [strictly_before(a, b).embed(smt), strictly_before(b, a).embed(smt)];
        smt.reified_or(&clause)
    };
    let eq = |smt, a, b| !neq(smt, a, b);
    let leq_with_delays = |ta, da, tb, db| aries_tnet::max_delay(tb, ta, db - da);
    let leq = |a: Time<_>, b: Time<_>| leq_with_delays(int(a.time_var), a.delay, int(b.time_var), b.delay);

    let effs: Vec<_> = effects(&pb).collect();
    let conds: Vec<_> = conditions(&pb).collect();
    let eff_ends: Vec<_> = effs.iter().map(|_| smt.theory.add_timepoint(ORIGIN, HORIZON)).collect();

    for &(prez_cond, cond) in &conds {
        let timepoint_order = [leq(cond.start, cond.end).embed(&mut smt)];
        smt.add_clause(&timepoint_order);
    }
    for ieff in 0..effs.len() {
        let (prez_eff, eff) = effs[ieff];
        let persistence_timepoints_order = [leq_with_delays(
            int(eff.persistence_start.time_var),
            eff.persistence_start.delay,
            eff_ends[ieff],
            0,
        )
        .embed(&mut smt)];
        smt.add_clause(&persistence_timepoints_order);
        let transition_timepoints_order = [leq(eff.transition_start, eff.persistence_start).embed(&mut smt)];
        smt.add_clause(&transition_timepoints_order);
    }

    let unifiable_vars = |a, b| {
        let dom_a = pb.variables[a].domain;
        let dom_b = pb.variables[b].domain;
        dom_a.intersects(&dom_b)
    };

    let unifiable_sv = |sv1: &[usize], sv2: &[usize]| {
        if sv1.len() != sv2.len() {
            false
        } else {
            for (&a, &b) in sv1.iter().zip(sv2) {
                if !unifiable_vars(a, b) {
                    return false;
                }
            }
            true
        }
    };

    // coherence constraints
    let mut clause = Vec::with_capacity(32);
    for (i, &(p1, e1)) in effs.iter().enumerate() {
        for j in i + 1..effs.len() {
            let &(p2, e2) = &effs[j];
            clause.clear();
            clause.push(!bool(p1));
            clause.push(!bool(p2));
            if !unifiable_sv(&e1.state_var, &e2.state_var) {
                continue;
            }
            assert_eq!(e1.state_var.len(), e2.state_var.len());
            for idx in 0..e1.state_var.len() {
                let a = int(e1.state_var[idx]);
                let b = int(e2.state_var[idx]);
                // enforce different : a < b || a > b
                // if they are the same variable, there is nothing we can do to separate them
                if a != b {
                    clause.push(strictly_before(a, b).embed(&mut smt));
                    clause.push(strictly_before(b, a).embed(&mut smt));
                }
            }
            clause.push(
                leq_with_delays(
                    eff_ends[j],
                    0,
                    int(e1.transition_start.time_var),
                    e1.transition_start.delay,
                )
                .embed(&mut smt),
            );
            clause.push(
                leq_with_delays(
                    eff_ends[i],
                    0,
                    int(e2.transition_start.time_var),
                    e2.transition_start.delay,
                )
                .embed(&mut smt),
            );
            smt.add_clause(&clause);
            println!("Coherence clause: {:?}", &clause);
        }
    }

    // support constraints
    for (prez_cond, cond) in conds {
        let mut supported = Vec::with_capacity(128);
        // either the condition is not present
        supported.push(!bool(prez_cond));

        for (eff_id, &(prez_eff, eff)) in effs.iter().enumerate() {
            let mut is_support_possible = true;
            let mut supported_by_eff_conjunction = Vec::with_capacity(32);
            supported_by_eff_conjunction.push(bool(prez_eff));
            if !unifiable_sv(&cond.state_var, &eff.state_var) {
                continue;
            }
            if !unifiable_vars(cond.value, eff.value) {
                continue;
            }
            assert_eq!(cond.state_var.len(), eff.state_var.len());
            // same state variable
            for idx in 0..cond.state_var.len() {
                let a = int(cond.state_var[idx]);
                let b = int(eff.state_var[idx]);
                supported_by_eff_conjunction.push(before_eq(a, b).embed(&mut smt));
                supported_by_eff_conjunction.push(before_eq(b, a).embed(&mut smt));
            }
            // same value
            let condition_value = int(cond.value);
            let effect_value = int(eff.value);
            supported_by_eff_conjunction.push(before_eq(condition_value, effect_value).embed(&mut smt));
            supported_by_eff_conjunction.push(before_eq(effect_value, condition_value).embed(&mut smt));

            // effect's persistence contains condition
            supported_by_eff_conjunction.push(leq(eff.persistence_start, cond.start).embed(&mut smt));
            supported_by_eff_conjunction
                .push(leq_with_delays(int(cond.end.time_var), cond.end.delay, eff_ends[eff_id], 0).embed(&mut smt));

            supported.push(smt.reified_and(&supported_by_eff_conjunction));
        }

        smt.add_clause(&supported);
        println!("Added support clause {:?}", supported);
    }

    // chronicle constraints
    for instance in &pb.chronicles {
        for constraint in &instance.chronicle.constraints {
            match constraint.tpe {
                ConstraintType::InTable { .. } => unimplemented!(),
            }
        }
    }

    Ok((smt, cor))
}
