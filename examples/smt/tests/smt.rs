use aries::backtrack::Backtrack;
use aries::core::state::OptDomain;
use aries::core::Lit;
use aries::model::extensions::AssignmentExt;
use aries::model::lang::expr::*;
use aries::model::lang::IVar;
use aries_stn::theory::{StnConfig, StnTheory};
use itertools::Itertools;

type Model = aries::model::Model<String>;
type Solver = aries_solver::solver::Solver<String>;

#[test]
fn sat() {
    let mut model = Model::new();
    let a = model.new_bvar("a").true_lit();
    let b = model.new_bvar("b").true_lit();

    let mut solver = Solver::new(model);
    solver.enforce(a);
    assert!(solver.solve().unwrap().is_some());
    assert_eq!(solver.model.boolean_value_of(a), Some(true));
    solver.reset();
    solver.enforce(implies(a, b));
    assert!(solver.solve().unwrap().is_some());
    assert_eq!(solver.model.boolean_value_of(a), Some(true));
    assert_eq!(solver.model.boolean_value_of(b), Some(true));

    solver.enforce(!b);

    assert!(solver.solve().unwrap().is_none());
}

#[test]
fn diff_logic() {
    let mut model = Model::new();
    let a = model.new_ivar(0, 10, "a");
    let b = model.new_ivar(0, 10, "b");
    let c = model.new_ivar(0, 10, "c");

    let constraints = vec![lt(a, b), lt(b, c), lt(c, a)];

    let mut solver = Solver::new(model);

    solver.add_theory(|tok| StnTheory::new(tok, StnConfig::default()));
    solver.enforce_all(constraints);
    assert!(solver.solve().unwrap().is_none());
}

#[test]
fn minimize() {
    let mut model = Model::new();
    let a = model.new_ivar(0, 10, "a");
    let b = model.new_ivar(0, 10, "b");
    let c = model.new_ivar(0, 10, "c");

    model.enforce(lt(a, b));
    model.enforce(lt(b, c));
    model.enforce(lt(a, c));
    let x = model.reify(geq(b, 6));
    let y = model.reify(geq(b, 8));
    model.enforce(or([x, y]));

    let mut solver = Solver::new(model);
    solver.add_theory(|tok| StnTheory::new(tok, StnConfig::default()));

    assert!(solver.solve().unwrap().is_some());
    match solver.minimize(c).unwrap() {
        None => panic!(),
        Some((val, _)) => assert_eq!(val, 7),
    }
}

#[test]
fn minimize_small() {
    let mut model = Model::new();
    let a = model.new_ivar(0, 10, "a");

    let x = model.reify(geq(a, 6));
    let y = model.reify(geq(a, 8));

    model.enforce(or([x, y]));

    let mut solver = Solver::new(model);

    solver.add_theory(|tok| StnTheory::new(tok, StnConfig::default()));
    assert!(solver.solve().unwrap().is_some());
    match solver.minimize(a).unwrap() {
        None => panic!(),
        Some((val, _)) => assert_eq!(val, 6),
    }
    solver.print_stats()
}

#[test]
fn int_bounds() {
    let mut model = Model::new();
    let a = model.new_ivar(0, 10, "a");
    let b = model.new_ivar(0, 10, "b");
    let c = model.new_ivar(0, 10, "c");
    let d = model.new_ivar(0, 10, "d");
    let e = model.new_ivar(0, 10, "e");
    let f = model.new_ivar(0, 10, "f");
    let g = model.new_ivar(0, 10, "g");
    let h = model.new_ivar(0, 10, "h");

    let constraints = vec![
        leq(a, 8),
        leq(2, a),
        lt(1, b),
        lt(b, 9),
        geq(c, 2),
        geq(8, c),
        gt(d, 1),
        gt(9, d),
        !gt(e, 8),
        !gt(2, e),
        !geq(1, f),
        !geq(f, 9),
        !lt(g, 2),
        !lt(8, g),
        !leq(h, 1),
        !leq(9, h),
    ];

    let mut solver = Solver::new(model);
    solver.enforce_all(constraints);
    assert!(solver.propagate_and_backtrack_to_consistent());
    let check_dom = |v, lb, ub| {
        assert_eq!(solver.model.domain_of(v), (lb, ub));
    };
    check_dom(a, 2, 8);
    check_dom(b, 2, 8);
    check_dom(c, 2, 8);
    check_dom(d, 2, 8);
    check_dom(e, 2, 8);
    check_dom(f, 2, 8);
    check_dom(g, 2, 8);
    check_dom(h, 2, 8);
}

#[test]
fn bools_as_ints() {
    let mut model = Model::new();
    let a = model.new_bvar("a");
    let ia: IVar = a.into();
    let b = model.new_bvar("b");
    let ib: IVar = b.into();
    let c = model.new_bvar("c");
    let ic: IVar = c.into();
    let d = model.new_bvar("d");
    let id: IVar = d.into();

    let mut solver = Solver::new(model);
    solver.add_theory(|tok| StnTheory::new(tok, StnConfig::default()));

    assert!(solver.propagate_and_backtrack_to_consistent());
    assert_eq!(solver.model.boolean_value_of(a), None);
    assert_eq!(solver.model.domain_of(ia), (0, 1));
    assert_eq!(solver.model.boolean_value_of(a), None);
    assert_eq!(solver.model.domain_of(ia), (0, 1));
    assert_eq!(solver.model.boolean_value_of(a), None);
    assert_eq!(solver.model.domain_of(ia), (0, 1));
    assert_eq!(solver.model.boolean_value_of(a), None);
    assert_eq!(solver.model.domain_of(ia), (0, 1));

    solver.enforce(a.true_lit());
    solver.enforce(b.false_lit());
    solver.enforce(geq(ic, 1));
    solver.enforce(leq(id, 0));

    solver.propagate().unwrap();
    assert_eq!(solver.model.boolean_value_of(a), Some(true));
    assert_eq!(solver.model.domain_of(ia), (1, 1));
    assert_eq!(solver.model.boolean_value_of(b), Some(false));
    assert_eq!(solver.model.domain_of(ib), (0, 0));
    assert_eq!(solver.model.boolean_value_of(c), Some(true));
    assert_eq!(solver.model.domain_of(ic), (1, 1));
    assert_eq!(solver.model.boolean_value_of(d), Some(false));
    assert_eq!(solver.model.domain_of(id), (0, 0));
}

#[test]
fn ints_and_bools() {
    let mut model = Model::new();
    let a = model.new_bvar("a");
    let ia: IVar = a.into();
    let i = model.new_ivar(-10, 10, "i");

    let mut solver = Solver::new(model);
    solver.add_theory(|tok| StnTheory::new(tok, StnConfig::default()));

    assert!(solver.propagate_and_backtrack_to_consistent());
    assert_eq!(solver.model.domain_of(i), (-10, 10));
    assert_eq!(solver.model.domain_of(ia), (0, 1));
    assert_eq!(solver.model.boolean_value_of(a), None);

    solver.enforce(leq(i, ia));
    assert!(solver.propagate_and_backtrack_to_consistent());
    assert_eq!(solver.model.domain_of(i), (-10, 1));
    assert_eq!(solver.model.domain_of(ia), (0, 1));
    assert_eq!(solver.model.boolean_value_of(a), None);

    solver.enforce(gt(ia, i));
    assert!(solver.propagate_and_backtrack_to_consistent());
    assert_eq!(solver.model.domain_of(i), (-10, 0));
    assert_eq!(solver.model.domain_of(ia), (0, 1));
    assert_eq!(solver.model.boolean_value_of(a), None);

    solver.enforce(geq(i, 0));
    assert!(solver.propagate_and_backtrack_to_consistent());
    assert_eq!(solver.model.domain_of(i), (0, 0));
    assert_eq!(solver.model.domain_of(ia), (1, 1));
    assert_eq!(solver.model.boolean_value_of(a), Some(true));
}

#[test]
fn optional_hierarchy() {
    use OptDomain::{Absent, Present, Unknown};

    let mut model = Model::new();
    let p = model.new_bvar("a").true_lit();
    let i = model.new_optional_ivar(-10, 10, p, "i");

    let scopes: Vec<Lit> = (0..3)
        .map(|i| model.new_presence_variable(p, format!("p_{i}")).true_lit())
        .collect();
    let domains = [(0, 8), (-20, -5), (5, 20)];
    let vars: Vec<IVar> = domains
        .iter()
        .enumerate()
        .map(|(i, (lb, ub))| model.new_optional_ivar(*lb, *ub, scopes[i], format!("i_{i}")))
        .collect();

    // at least one must be present
    // constraints.push(model.or(&scopes.iter().map(|&lit| BAtom::from(lit)).collect::<Vec<_>>()));

    for &sub_var in &vars {
        model.enforce(eq(i, sub_var));
    }

    let mut solver = Solver::new(model);
    solver.add_theory(|tok| StnTheory::new(tok, StnConfig::default()));

    // solver.model.print_state();

    assert!(solver.propagate_and_backtrack_to_consistent());

    // solver.model.print_state();

    assert_eq!(solver.model.opt_domain_of(i), Unknown(-10, 10));
    assert_eq!(solver.model.opt_domain_of(vars[0]), Unknown(0, 8));
    assert_eq!(solver.model.opt_domain_of(vars[1]), Unknown(-10, -5));
    assert_eq!(solver.model.opt_domain_of(vars[2]), Unknown(5, 10));

    solver.decide(Lit::leq(i, 9));
    assert!(solver.propagate_and_backtrack_to_consistent());

    assert_eq!(solver.model.opt_domain_of(i), Unknown(-10, 9));
    assert_eq!(solver.model.opt_domain_of(vars[0]), Unknown(0, 8));
    assert_eq!(solver.model.opt_domain_of(vars[1]), Unknown(-10, -5));
    assert_eq!(solver.model.opt_domain_of(vars[2]), Unknown(5, 9));

    // println!();
    // solver.model.state.print();

    solver.decide(Lit::leq(i, 4));
    assert!(solver.propagate_and_backtrack_to_consistent());

    // println!();
    // solver.model.state.print();

    assert_eq!(solver.model.opt_domain_of(i), Unknown(-10, 4));
    assert_eq!(solver.model.opt_domain_of(vars[0]), Unknown(0, 4));
    assert_eq!(solver.model.opt_domain_of(vars[1]), Unknown(-10, -5));
    assert_eq!(solver.model.opt_domain_of(vars[2]), Absent);
    // solver.model.discrete.print();

    solver.save_state();
    solver.decide(p);
    assert!(solver.propagate_and_backtrack_to_consistent());
    assert_eq!(solver.model.opt_domain_of(i), Present(-10, 4));
    assert_eq!(solver.model.opt_domain_of(vars[0]), Unknown(0, 4));
    assert_eq!(solver.model.opt_domain_of(vars[1]), Unknown(-10, -5));
    assert_eq!(solver.model.opt_domain_of(vars[2]), Absent);

    println!("======================");

    solver.decide(Lit::leq(i, -1));
    assert!(solver.propagate_and_backtrack_to_consistent());
    assert_eq!(solver.model.opt_domain_of(i), Present(-10, -1));
    assert_eq!(solver.model.opt_domain_of(vars[0]), Absent);
    assert_eq!(solver.model.opt_domain_of(vars[1]), Unknown(-10, -5));
    assert_eq!(solver.model.opt_domain_of(vars[2]), Absent);

    solver.decide(scopes[1]);
    assert!(solver.propagate_and_backtrack_to_consistent());
    assert_eq!(solver.model.opt_domain_of(i), Present(-10, -5));
    assert_eq!(solver.model.opt_domain_of(vars[0]), Absent);
    assert_eq!(solver.model.opt_domain_of(vars[1]), Present(-10, -5));
    assert_eq!(solver.model.opt_domain_of(vars[2]), Absent);

    // solver.decide(!p);
    // assert!(matches!(solver.propagate_and_backtrack_to_consistent(), Ok(true));
    // solver.model.discrete.print();

    // assert_eq!(solver.model.opt_domain_of(i), Absent);
    // assert_eq!(solver.model.opt_domain_of(vars[0]), Absent);
    // assert_eq!(solver.model.opt_domain_of(vars[1]), Absent);
    // assert_eq!(solver.model.opt_domain_of(vars[2]), Absent);
}

#[test]
fn test_reification_optionals() {
    let mut model = Model::new();
    let pa = model.new_presence_variable(Lit::TRUE, "pa").true_lit();
    let a = model.new_optional_ivar(0, 10, pa, "a");
    let pb = model.new_presence_variable(Lit::TRUE, "pb").true_lit();
    let b = model.new_optional_ivar(0, 10, pb, "b");

    let a_leq_b = model.reify(leq(a, b));
    let pab = model.presence_literal(a_leq_b.variable());
    assert_ne!(pab, Lit::TRUE);
    assert!(model.state.implies(pab, pa));
    assert!(model.state.implies(pab, pb));

    // defined if (!pa | pab) <=> (!pa | (pa & pb)) <=> pb
    let or = model.reify(or([!pa, a_leq_b]));
    let p_or = model.presence_literal(or.variable());
    assert!(model.state.implies(p_or, pb));
    assert!(!model.state.implies(p_or, pa));
}

/// A test represent the fact when **all** `premices` become true (regardless of the order),
/// then **all** `expected` literal should be inferred as true.
#[derive(Debug)]
struct Test {
    premices: Vec<Lit>,
    expected: Vec<Lit>,
}

impl Test {
    pub fn new(premices: &[Lit], expected: &[Lit]) -> Test {
        Test {
            premices: premices.into(),
            expected: expected.into(),
        }
    }
}

/// Run all tests on the given solver.
///
/// For all possible order of achievement of the premices in a given test, this checks that
///  - no expected literal becomes true before all premices are entailed.
///  - all expected literals become true once all premices have been enforced.
fn run_tests(solver: &mut Solver, tests: &[Test]) {
    for (test_id, test) in tests.iter().enumerate() {
        // we have a test, we will test the inferences under all possible realisation order of the premices
        for premices_order in test.premices.iter().copied().permutations(test.premices.len()) {
            // new test case, with a unique order of premices
            let test = Test::new(&premices_order, &test.expected);

            println!(
                "=============================\n\
                     Running test {} with premices order: {:?}",
                test_id,
                test.premices
                    .iter()
                    .map(|l| solver.model.fmt(*l).to_string())
                    .collect::<Vec<_>>()
            );

            solver.reset();
            solver.propagate().unwrap();

            for (decision_id, &decision) in test.premices.iter().enumerate() {
                solver.save_state();
                solver.model.print_state();

                // some premices have not been established yet, check that no inference was made
                for &expected in &test.expected {
                    assert!(
                        !solver.model.entails(expected),
                        "Inferred after only {} established premices: {}",
                        decision_id,
                        solver.model.fmt(expected)
                    );
                }

                println!("\ndecision: {}\n", solver.model.fmt(decision));
                solver.decide(decision);
                assert!(
                    solver.propagate().is_ok(),
                    "Failed to propagate the {}th premice: {} ",
                    decision_id + 1,
                    solver.model.fmt(decision),
                );
            }
            solver.model.print_state();

            // all premices established, check that all inferences were made
            for &expected in &test.expected {
                assert!(
                    solver.model.entails(expected),
                    "Not inferred: {}",
                    solver.model.fmt(expected),
                );
            }
        }
    }
}

#[test]
fn test_leq_propagation() {
    let mut model = Model::new();

    let a = model.new_ivar(0, 100, "a");
    let b = model.new_ivar(0, 100, "b");
    let l = model.reify(leq(a, b));

    let tests = vec![
        Test::new(&[l, a.geq(4)], &[b.geq(4)]),
        Test::new(&[l, b.leq(3)], &[a.leq(3)]),
        Test::new(&[!l, b.geq(4)], &[a.geq(5)]),
        Test::new(&[!l, a.leq(4)], &[b.leq(3)]),
        // the two test below should only work in the presence of bounds theory propagation (on by default)
        Test::new(&[b.lt(5), a.geq(5)], &[!l]),
        Test::new(&[a.leq(5), b.geq(5)], &[l]),
    ];

    let mut solver = Solver::new(model);
    solver.add_theory(|tok| StnTheory::new(tok, StnConfig::default()));
    run_tests(&mut solver, &tests);
}

#[test]
fn test_opt_leq_propagation() {
    let mut model = Model::new();
    let pa = model.new_presence_variable(Lit::TRUE, "pa").true_lit();
    let a = model.new_optional_ivar(0, 100, pa, "a");
    let pb = model.new_presence_variable(Lit::TRUE, "pb").true_lit();
    let b = model.new_optional_ivar(0, 100, pb, "b");
    // the two test below should only work in the presence of bounds theory propagation (on by default)
    let l = model.reify(leq(a, b));
    model.shape.labels.insert(l.variable(), "l".to_string());
    let v = model.presence_literal(l.variable());
    model.shape.labels.insert(v.variable(), "v".to_string());

    let tests = vec![
        Test::new(&[v, l, a.geq(4)], &[b.geq(4)]),
        Test::new(&[pa, l, a.geq(4)], &[b.geq(4)]),
        Test::new(&[v, l, b.leq(3)], &[a.leq(3)]),
        Test::new(&[pb, l, b.leq(3)], &[a.leq(3)]),
        Test::new(&[v, !l, b.geq(4)], &[a.geq(5)]),
        Test::new(&[pb, !l, b.geq(4)], &[a.geq(5)]),
        Test::new(&[v, !l, a.leq(4)], &[b.leq(3)]),
        Test::new(&[pa, !l, a.leq(4)], &[b.leq(3)]),
        Test::new(&[b.lt(5), a.geq(5)], &[!l]),
        Test::new(&[a.leq(5), b.geq(5)], &[l]),
    ];

    let mut solver = Solver::new(model);
    solver.add_theory(|tok| StnTheory::new(tok, StnConfig::default()));
    run_tests(&mut solver, &tests);
}

#[test]
fn test_opt_leq_eager_propagation() {
    let mut model = Model::new();
    let pa = model.new_presence_variable(Lit::TRUE, "pa").true_lit();
    let a = model.new_optional_ivar(0, 100, pa, "a");
    let pb = model.new_presence_variable(pa, "pb").true_lit();
    let b = model.new_optional_ivar(0, 100, pb, "b");
    // the two test below should only work in the presence of bounds theory propagation (on by default)
    let l = model.reify(leq(a, b));
    model.shape.labels.insert(l.variable(), "l".to_string());
    let v = model.presence_literal(l.variable());
    debug_assert_eq!(v, pb);
    // model.shape.labels.insert(v.variable(), "v".to_string());

    let tests = vec![
        // Test::new(&[l, a.geq(4)], &[b.geq(4)]),
        Test::new(&[v, l, b.leq(3)], &[a.leq(3)]),
        Test::new(&[v, !l, b.geq(4)], &[a.geq(5)]),
        Test::new(&[!l, a.leq(4)], &[b.leq(3)]),
        Test::new(&[b.lt(5), a.geq(5)], &[!l]),
        Test::new(&[a.leq(5), b.geq(5)], &[l]),
    ];

    let mut solver = Solver::new(model);
    solver.add_theory(|tok| StnTheory::new(tok, StnConfig::default()));
    run_tests(&mut solver, &tests);
}
