#![allow(unreachable_code, unused_mut, dead_code, unused_variables, unused_imports)] // TODO: remove
#![allow(clippy::all)]

use anyhow::*;

use aries_planning::chronicles::{Effect, FiniteProblem, VarKind};

use aries_collections::ref_store::{Ref, RefVec};
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

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash)]
enum Var {
    Boolean(BVar),
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

    println!("{} {}", pb.origin, pb.horizon);

    Ok(())
}

type SMT = SMTSolver<Edge<i32>, IncSTN<i32>>;

fn effects<X: Ref>(pb: &FiniteProblem<X>) -> impl Iterator<Item = (X, &Effect<X>)> {
    pb.chronicles
        .iter()
        .flat_map(|ch| ch.chronicle.effects.iter().map(move |eff| (ch.chronicle.presence, eff)))
}

const ORIGIN: i32 = 0;
const HORIZON: i32 = 999999;

fn encode(pb: &FiniteProblem<usize>) -> anyhow::Result<SMT> {
    let mut smt = SMT::default();
    let sat_params = aries_sat::solver::SearchParams::default();
    let mut sat = aries_sat::solver::Solver::new(sat_params);
    let mut stn = IncSTN::new();

    let mut cor = RefVec::new();
    let mut cor_back = HashMap::new();

    for (v, meta) in pb.variables.entries() {
        match meta.domain.kind {
            VarKind::Boolean => {
                let bvar = sat.add_var();
                cor.set_next(v, Var::Boolean(bvar));
                cor_back.insert(Var::Boolean(bvar), v);
            }
            _ => {
                let ivar = stn.add_timepoint(meta.domain.min, meta.domain.max);
                cor.set_next(v, Var::Integer(ivar));
                cor_back.insert(Var::Integer(ivar), v);
            }
        }
    }

    let b = |x| match cor[x] {
        Var::Boolean(y) => y.true_lit(),
        Var::Integer(_) => panic!(),
    };
    let i = |x| match cor[x] {
        Var::Boolean(_) => panic!(),
        Var::Integer(i) => i,
    };
    let neq = |smt: &mut SMT, a, b| {
        let ab = aries_tnet::strictly_before(a, b);
        let ab = smt.literal_of(ab);
        let ba = aries_tnet::strictly_before(b, a);
        let ba = smt.literal_of(ba);
        smt.reified_or(&[ab, ba])
    };
    let eq = |smt, a, b| !neq(smt, a, b);

    let effs: Vec<_> = effects(&pb).collect();
    let eff_ends = effs.iter().map(|_| stn.add_timepoint(ORIGIN, HORIZON));

    for ieff in 0..effs.len() {}

    let mut clause = Vec::with_capacity(32);
    for (x, &(p1, e1)) in effs.iter().enumerate() {
        for &(p2, e2) in &effs[x + 1..] {
            clause.clear();
            clause.push(!b(p1));
            clause.push(!b(p2));
            if e1.state_var.len() != e2.state_var.len() {
                continue;
            }
            for idx in 0..e1.state_var.len() {
                let a = i(e1.state_var[idx]);
                let b = i(e2.state_var[idx]);
                // enforce different : a < b || a > b
                clause.push(smt.literal_of(strictly_before(a, b)));
                clause.push(smt.literal_of(strictly_before(b, a)));
            }
            smt.add_clause(&clause)
        }
    }

    unimplemented!()
}
