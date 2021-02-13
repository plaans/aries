use aries_model::assignments::Assignment;
use aries_model::lang::{BAtom, IVar};
use aries_model::Model;
use aries_smt::solver::SMTSolver;
use aries_tnet::stn::IncSTN;

#[test]
fn sat() {
    let mut model = Model::new();
    let a = model.new_bvar("a");
    let b = model.new_bvar("b");

    let mut solver = SMTSolver::new(model);
    solver.enforce(a);
    assert!(solver.solve());
    assert_eq!(solver.model.boolean_value_of(a), Some(true));
    assert_eq!(solver.model.boolean_value_of(b), None);
    let c = solver.model.implies(a, b);
    solver.enforce(c);
    assert!(solver.solve());
    assert_eq!(solver.model.boolean_value_of(a), Some(true));
    assert_eq!(solver.model.boolean_value_of(b), Some(true));

    solver.enforce(!b);

    assert!(!solver.solve());
}

#[test]
fn diff_logic() {
    let mut model = Model::new();
    let a = model.new_ivar(0, 10, "a");
    let b = model.new_ivar(0, 10, "b");
    let c = model.new_ivar(0, 10, "c");

    let constraints = vec![model.lt(a, b), model.lt(b, c), model.lt(c, a)];

    let mut solver = SMTSolver::new(model);
    let theory = IncSTN::new();
    solver.add_theory(Box::new(theory));
    solver.enforce_all(&constraints);
    assert!(!solver.solve());
}

#[test]
fn minimize() {
    let mut model = Model::new();
    let a = model.new_ivar(0, 10, "a");
    let b = model.new_ivar(0, 10, "b");
    let c = model.new_ivar(0, 10, "c");

    let x = model.geq(b, 6);
    let y = model.geq(b, 8);

    let constraints = vec![model.lt(a, b), model.lt(b, c), model.lt(a, c), model.or2(x, y)];

    let mut solver = SMTSolver::new(model);
    let theory = IncSTN::new();
    solver.add_theory(Box::new(theory));
    solver.enforce_all(&constraints);
    assert!(solver.solve());
    match solver.minimize(c) {
        None => panic!(),
        Some((val, _)) => assert_eq!(val, 7),
    }
    solver.print_stats()
}

#[test]
fn minimize_small() {
    let mut model = Model::new();
    let a = model.new_ivar(0, 10, "a");

    let x = model.geq(a, 6);
    let y = model.geq(a, 8);

    let constraints = vec![model.or2(x, y)];

    let mut solver = SMTSolver::new(model);
    let theory = IncSTN::new();
    solver.add_theory(Box::new(theory));
    solver.enforce_all(&constraints);
    assert!(solver.solve());
    match solver.minimize(a) {
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
        model.leq(a, 8),
        model.leq(2, a),
        model.lt(1, b),
        model.lt(b, 9),
        model.geq(c, 2),
        model.geq(8, c),
        model.gt(d, 1),
        model.gt(9, d),
        !model.gt(e, 8),
        !model.gt(2, e),
        !model.geq(1, f),
        !model.geq(f, 9),
        !model.lt(g, 2),
        !model.lt(8, g),
        !model.leq(h, 1),
        !model.leq(9, h),
    ];

    let mut solver = SMTSolver::new(model);
    solver.enforce_all(&constraints);
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

    let mut solver = SMTSolver::new(model);
    let theory = IncSTN::new();
    solver.add_theory(Box::new(theory));

    assert!(solver.propagate_and_backtrack_to_consistent());
    assert_eq!(solver.model.boolean_value_of(a), None);
    assert_eq!(solver.model.domain_of(ia), (0, 1));
    assert_eq!(solver.model.boolean_value_of(a), None);
    assert_eq!(solver.model.domain_of(ia), (0, 1));
    assert_eq!(solver.model.boolean_value_of(a), None);
    assert_eq!(solver.model.domain_of(ia), (0, 1));
    assert_eq!(solver.model.boolean_value_of(a), None);
    assert_eq!(solver.model.domain_of(ia), (0, 1));

    let constraints: Vec<BAtom> = vec![a.into(), (!b).into(), solver.model.geq(ic, 1), solver.model.leq(id, 0)];
    solver.enforce_all(&constraints);

    assert!(solver.propagate_and_backtrack_to_consistent());
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

    let mut solver = SMTSolver::new(model);
    let theory = IncSTN::new();
    solver.add_theory(Box::new(theory));

    assert!(solver.propagate_and_backtrack_to_consistent());
    assert_eq!(solver.model.domain_of(i), (-10, 10));
    assert_eq!(solver.model.domain_of(ia), (0, 1));
    assert_eq!(solver.model.boolean_value_of(a), None);

    let constraint = solver.model.leq(i, ia);
    solver.enforce(constraint);
    assert!(solver.propagate_and_backtrack_to_consistent());
    assert_eq!(solver.model.domain_of(i), (-10, 1));
    assert_eq!(solver.model.domain_of(ia), (0, 1));
    assert_eq!(solver.model.boolean_value_of(a), None);

    let constraint = solver.model.gt(ia, i);
    solver.enforce(constraint);
    assert!(solver.propagate_and_backtrack_to_consistent());
    assert_eq!(solver.model.domain_of(i), (-10, 0));
    assert_eq!(solver.model.domain_of(ia), (0, 1));
    assert_eq!(solver.model.boolean_value_of(a), None);

    let constraint = solver.model.geq(i, 0);
    solver.enforce(constraint);
    assert!(solver.propagate_and_backtrack_to_consistent());
    assert_eq!(solver.model.domain_of(i), (0, 0));
    assert_eq!(solver.model.domain_of(ia), (1, 1));
    assert_eq!(solver.model.boolean_value_of(a), Some(true));
}
