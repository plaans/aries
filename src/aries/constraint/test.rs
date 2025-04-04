//! Module to easily test aries constraints.
//!
//! It provides two kinds of function:
//!  - basic model generators
//!  - solution checking by enumerating all possibilities

use aries::core::IntCst;
use aries::core::VarRef;
use aries::model::lang::linear::NFLinearSumItem;
use aries::model::lang::BVar;
use aries::model::lang::IVar;
use aries::model::Model;
use aries::solver::Solver;

/// Return all possible values for the given variables.
///
/// The solutions are generated in lexicographic order.
fn get_solutions_2(
    x: VarRef,
    y: VarRef,
    model: Model<String>,
) -> Vec<(IntCst, IntCst)> {
    let mut solver = Solver::new(model);
    let mut solutions: Vec<(IntCst, IntCst)> = solver
        .enumerate(&[x.into(), y.into()])
        .unwrap()
        .iter()
        .map(|v| (v[0], v[1]))
        .collect();
    solutions.sort();
    solutions
}

/// Generate the solutions respecting the given predicate verify.
///
/// The solutions are generated in lexicographic order.
fn gen_solutions_2(
    x: VarRef,
    y: VarRef,
    model: &Model<String>,
    verify: impl Fn(IntCst, IntCst) -> bool,
) -> Vec<(IntCst, IntCst)> {
    let (lb_x, ub_x) = model.state.bounds(x);
    let (lb_y, ub_y) = model.state.bounds(y);

    let mut solutions = Vec::new();

    for val_x in lb_x..=ub_x {
        for val_y in lb_y..=ub_y {
            if verify(val_x, val_y) {
                let solution = (val_x, val_y);
                solutions.push(solution);
            }
        }
    }
    solutions
}

/// Verify all the (x,y) solutions of the model.
///
/// (x,y) should be a solution iff `verify(x,y) == true`.
pub(super) fn verify_all_2(
    x: impl Into<VarRef>,
    y: impl Into<VarRef>,
    model: Model<String>,
    verify: impl Fn(IntCst, IntCst) -> bool,
) {
    let x = x.into();
    let y = y.into();
    let expected = gen_solutions_2(x, y, &model, verify);
    let solutions = get_solutions_2(x, y, model);
    assert_eq!(solutions, expected);
}

/// Return all possible values for the given variables.
///
/// The solutions are generated in lexicographic order.
fn get_solutions_3(
    x: VarRef,
    y: VarRef,
    z: VarRef,
    model: Model<String>,
) -> Vec<(IntCst, IntCst, IntCst)> {
    let mut solver = Solver::new(model);
    let mut solutions: Vec<(IntCst, IntCst, IntCst)> = solver
        .enumerate(&[x.into(), y.into(), z.into()])
        .unwrap()
        .iter()
        .map(|v| (v[0], v[1], v[2]))
        .collect();
    solutions.sort();
    solutions
}

/// Generate the solutions respecting the given predicate verify.
///
/// The solutions are generated in lexicographic order.
fn gen_solutions_3(
    x: VarRef,
    y: VarRef,
    z: VarRef,
    model: &Model<String>,
    verify: impl Fn(IntCst, IntCst, IntCst) -> bool,
) -> Vec<(IntCst, IntCst, IntCst)> {
    let (lb_x, ub_x) = model.state.bounds(x);
    let (lb_y, ub_y) = model.state.bounds(y);
    let (lb_z, ub_z) = model.state.bounds(z);

    let mut solutions = Vec::new();

    for val_x in lb_x..=ub_x {
        for val_y in lb_y..=ub_y {
            for val_z in lb_z..=ub_z {
                if verify(val_x, val_y, val_z) {
                    let solution = (val_x, val_y, val_z);
                    solutions.push(solution);
                }
            }
        }
    }
    solutions
}

/// Verify all the (x,y,z) solutions of the model.
///
/// (x,y,z) should be a solution iff `verify(x,y,z) == true`.
pub(super) fn verify_all_3(
    x: impl Into<VarRef>,
    y: impl Into<VarRef>,
    z: impl Into<VarRef>,
    model: Model<String>,
    verify: impl Fn(IntCst, IntCst, IntCst) -> bool,
) {
    let x = x.into();
    let y = y.into();
    let z = z.into();
    let expected = gen_solutions_3(x, y, z, &model, verify);
    let solutions = get_solutions_3(x, y, z, model);
    assert_eq!(solutions, expected);
}

/// Prepare a basic model for the tests.
/// Use it as follows.
/// ```
/// let (model, x, y) = basic_int_model_2();
/// ```
///
/// It has two variables:
///  - x in \[-1,7\]
///  - y in \[-4,6\]
pub(super) fn basic_int_model_2() -> (Model<String>, IVar, IVar) {
    let mut model = Model::new();

    let x = model.new_ivar(-1, 7, "x".to_string());
    let y = model.new_ivar(-4, 6, "y".to_string());

    (model, x, y)
}

/// Prepare a basic integer model for the tests.
/// Use it as follows.
/// ```
/// let (model, x, y, z) = basic_int_model_3();
/// ```
///
/// It has three variables:
///  - x in \[-1,7\]
///  - y in \[-4,6\]
///  - z in \[-2,5\]
pub(super) fn basic_int_model_3() -> (Model<String>, IVar, IVar, IVar) {
    let (mut model, x, y) = basic_int_model_2();
    let z = model.new_ivar(-2, 5, "z".to_string());

    (model, x, y, z)
}

/// Prepare a basic linear model for the tests.
/// Use it as follows.
/// ```
/// let (model, sum, x, y, c_x, c_y, b) = basic_lin_model();
/// ```
///
/// It has two variables:
///  - x in \[-1,7\]
///  - y in \[-4,6\]
///
/// The linear sum is 3\*x + 2\*y with bound 13.
pub(super) fn basic_lin_model() -> (
    Model<String>,
    Vec<NFLinearSumItem>,
    IVar,
    IVar,
    IntCst,
    IntCst,
    IntCst,
) {
    let (model, x, y) = basic_int_model_2();

    let c_x = 3;
    let c_y = 2;
    let b = 13;

    let sum = vec![
        NFLinearSumItem {
            var: x.into(),
            factor: c_x,
        },
        NFLinearSumItem {
            var: y.into(),
            factor: c_y,
        },
    ];

    (model, sum, x, y, c_x, c_y, b)
}

/// Prepare a basic boolean model for the tests.
/// Use it as follows.
/// ```
/// let (model, x, y) = basic_bool_model_2();
/// ```
///
/// It has two boolean variables: x, y.
pub(super) fn basic_bool_model_2() -> (Model<String>, BVar, BVar) {
    let mut model = Model::new();
    let x = model.new_bvar("x".to_string());
    let y = model.new_bvar("y".to_string());

    (model, x, y)
}

/// Prepare a basic boolean model for the tests.
/// Use it as follows.
/// ```
/// let (model, x, y, z) = basic_bool_model_3();
/// ```
///
/// It has three boolean variables: x, y, z.
pub(super) fn basic_bool_model_3() -> (Model<String>, BVar, BVar, BVar) {
    let (mut model, x, y) = basic_bool_model_2();
    let z = model.new_bvar("z".to_string());

    (model, x, y, z)
}

/// Prepare a basic model for the tests.
/// Use it as follows.
/// ```
/// let (model, x, y, r) = basic_reif_model();
/// ```
///
/// It has two variables:
///  - x in \[-1,7\]
///  - y in \[-4,6\]
///  - r bool
pub(super) fn basic_reif_model() -> (Model<String>, IVar, IVar, BVar) {
    let (mut model, x, y) = basic_int_model_2();

    let r = model.new_bvar("r".to_string());

    (model, x, y, r)
}
