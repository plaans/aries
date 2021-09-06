use aries_backtrack::Backtrack;
use aries_model::bounds::Lit;
use aries_model::extensions::{AssignmentExt, ExpressionFactoryExt};
use aries_model::lang::{BAtom, IVar};
use aries_model::state::OptDomain;
use aries_model::Model;
use aries_solver::solver::Solver;
use aries_tnet::theory::{StnConfig, StnTheory};

#[test]
fn sat() {
    let mut model = Model::new();
    let a = model.new_bvar("a");
    let b = model.new_bvar("b");

    let mut solver = Solver::new(model);
    solver.enforce(a);
    assert!(solver.solve().unwrap().is_some());
    assert_eq!(solver.model.boolean_value_of(a), Some(true));
    let c = solver.model.implies(a, b);
    solver.enforce(c);
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

    let constraints = vec![model.lt(a, b), model.lt(b, c), model.lt(c, a)];

    let theory = StnTheory::new(model.new_write_token(), StnConfig::default());
    let mut solver = Solver::new(model);

    solver.add_theory(Box::new(theory));
    solver.enforce_all(&constraints);
    assert!(solver.solve().unwrap().is_none());
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
    let theory = StnTheory::new(model.new_write_token(), StnConfig::default());
    let mut solver = Solver::new(model);

    solver.add_theory(Box::new(theory));
    solver.enforce_all(&constraints);
    assert!(solver.solve().unwrap().is_some());
    match solver.minimize(c).unwrap() {
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

    let theory = StnTheory::new(model.new_write_token(), StnConfig::default());
    let mut solver = Solver::new(model);

    solver.add_theory(Box::new(theory));
    solver.enforce_all(&constraints);
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

    let mut solver = Solver::new(model);
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

    let theory = StnTheory::new(model.new_write_token(), StnConfig::default());
    let mut solver = Solver::new(model);
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

    let theory = StnTheory::new(model.new_write_token(), StnConfig::default());
    let mut solver = Solver::new(model);
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

#[test]
fn optional_hierarchy() {
    use OptDomain::{Absent, Present, Unknown};

    let mut model = Model::new();
    let p = model.new_bvar("a").true_lit();
    let i = model.new_optional_ivar(-10, 10, p, "i");

    let scopes: Vec<Lit> = (0..3)
        .map(|i| model.new_presence_variable(p, format!("p_{}", i)).true_lit())
        .collect();
    let domains = [(0, 8), (-20, -5), (5, 20)];
    let vars: Vec<IVar> = domains
        .iter()
        .enumerate()
        .map(|(i, (lb, ub))| model.new_optional_ivar(*lb, *ub, scopes[i], format!("i_{}", i)))
        .collect();

    let mut constraints = Vec::with_capacity(32);

    // at least one must be present
    // constraints.push(model.or(&scopes.iter().map(|&lit| BAtom::from(lit)).collect::<Vec<_>>()));

    for &sub_var in &vars {
        constraints.push(model.opt_eq(i, sub_var));
    }

    let theory = StnTheory::new(model.new_write_token(), StnConfig::default());
    let mut solver = Solver::new(model);
    solver.add_theory(Box::new(theory));

    // solver.model.state.print();

    solver.enforce_all(&constraints);
    assert!(solver.propagate_and_backtrack_to_consistent());

    // solver.model.state.print();

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
