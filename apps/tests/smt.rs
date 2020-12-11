use aries_model::assignments::Assignment;
use aries_model::Model;
use aries_smt::solver::SMTSolver;
use aries_tnet::stn::DiffLogicTheory;

#[test]
fn sat() {
    let mut model = Model::new();
    let a = model.new_bvar("a");
    let b = model.new_bvar("b");

    let mut solver = SMTSolver::new(model);
    solver.enforce(&[a.into()]);
    assert!(solver.solve());
    assert_eq!(solver.model.boolean_value_of(a), Some(true));
    assert_eq!(solver.model.boolean_value_of(b), None);
    let c = solver.model.implies(a, b);
    solver.enforce(&[c]);
    assert!(solver.solve());
    assert_eq!(solver.model.boolean_value_of(a), Some(true));
    assert_eq!(solver.model.boolean_value_of(b), Some(true));

    solver.enforce(&[!b]);

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
    let theory = DiffLogicTheory::new();
    solver.add_theory(Box::new(theory));
    solver.enforce(&constraints);
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
    let theory = DiffLogicTheory::new();
    solver.add_theory(Box::new(theory));
    solver.enforce(&constraints);
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
    let theory = DiffLogicTheory::new();
    solver.add_theory(Box::new(theory));
    solver.enforce(&constraints);
    assert!(solver.solve());
    match solver.minimize(a) {
        None => panic!(),
        Some((val, _)) => assert_eq!(val, 6),
    }
    solver.print_stats()
}
