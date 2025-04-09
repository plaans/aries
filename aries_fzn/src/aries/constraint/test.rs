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
use itertools::Itertools;

/// Return all possible values for the given variables.
///
/// The solutions are generated in lexicographic order.
fn get_solutions<const N: usize>(
    vars: [VarRef; N],
    model: Model<String>,
) -> Vec<[IntCst; N]> {
    let mut solver = Solver::new(model);
    let mut solutions: Vec<[IntCst; N]> = solver
        .enumerate(&vars)
        .unwrap()
        .into_iter()
        .map(|v| v.try_into().expect("wrong number of elements"))
        .collect();
    solutions.sort();
    solutions
}

/// Generate the solutions respecting the given predicate verify.
///
/// The solutions are generated in lexicographic order.
fn gen_solutions<const N: usize>(
    vars: [VarRef; N],
    model: &Model<String>,
    verify: impl Fn([IntCst; N]) -> bool,
) -> Vec<[IntCst; N]> {
    let candidates = vars
        .iter()
        .map(|var| model.state.lb(*var)..=model.state.ub(*var))
        .multi_cartesian_product()
        .map(|vec| vec.try_into().expect("wrong number of elements"));

    let mut solutions = Vec::new();

    for candidate in candidates {
        if verify(candidate) {
            solutions.push(candidate);
        }
    }
    solutions
}

/// Verify all the solutions of the model.
///
/// `[var_1, var_2, ...]` should be a solution iff
/// `verify([var_1, var_2, ...]) == true`.
/// 
/// ```
/// # use aries::core::IntCst;
/// # use aries::model::Model;
/// # use aries::model::lang::IVar;
/// # use crate::aries::constraint::test::verify_all;
/// let model: Model<String>;
/// let x: IVar;
/// let y: IVar;
/// # let mut model = Model::new();
/// # x = model.new_ivar(1, 1, "x".to_string());
/// # y = model.new_ivar(1, 1, "y".to_string());
/// let verify = |[x, y]: [IntCst; 2]| x == y;
/// verify_all([x, y], model, verify);
/// ```
/// 
pub(super) fn verify_all<const N: usize>(
    vars: [impl Into<VarRef>; N],
    model: Model<String>,
    verify: impl Fn([IntCst; N]) -> bool,
) {
    let vars: [VarRef; N] = vars
        .into_iter()
        .map(|var| var.into())
        .collect::<Vec<VarRef>>()
        .try_into()
        .unwrap();
    let expected = gen_solutions(vars, &model, verify);
    let solutions = get_solutions(vars, model);
    assert_eq!(solutions, expected);
}

/// Prepare a basic model for the tests.
/// Use it as follows.
/// ```
/// let (model, x) = basic_int_model_1();
/// ```
///
/// It has one variables:
///  - x in \[-1,7\]
pub(super) fn basic_int_model_1() -> (Model<String>, IVar) {
    let mut model = Model::new();

    let x = model.new_ivar(-1, 7, "x".to_string());

    (model, x)
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
    let (mut model, x) = basic_int_model_1();

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
