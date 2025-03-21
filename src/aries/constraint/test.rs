use aries::core::IntCst;
use aries::core::VarRef;
use aries::model::lang::linear::NFLinearSumItem;
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
    let mut solutions: Vec<(i32, i32)> = solver
        .enumerate(&[x.into(), y.into()])
        .unwrap()
        .iter()
        .map(|v| (v[0], v[1]))
        .collect();
    solutions.sort();
    solutions
}

/// Generate the solutions respecting the given predicate check.
/// 
/// The solutions are generated in lexicographic order.
fn gen_solutions_2(
    x: VarRef,
    y: VarRef,
    model: &Model<String>,
    check: impl Fn(IntCst, IntCst) -> bool,
) -> Vec<(IntCst, IntCst)> {
    let (lb_x, ub_x) = model.state.bounds(x);
    let (lb_y, ub_y) = model.state.bounds(y);

    let mut solutions = Vec::new();

    for val_x in lb_x..=ub_x {
        for val_y in lb_y..=ub_y {
            if check(val_x, val_y) {
                let solution = (val_x, val_y);
                solutions.push(solution);
            }
        }
    }
    solutions
}

/// Verify all the (x,y) solutions of the model.
/// 
/// (x,y) should be a solution iff `check(x,y) == true`.
pub(super) fn verify_all_2(
    x: impl Into<VarRef>,
    y: impl Into<VarRef>,
    model: Model<String>,
    check: impl Fn(IntCst, IntCst) -> bool,
) {
    let x = x.into();
    let y = y.into();
    let expected = gen_solutions_2(x, y, &model, check);
    let solutions = get_solutions_2(x, y, model);
    assert_eq!(solutions, expected);
}

/// Prepare a basic model for the tests.
/// Use it as follows.
/// ```
/// let (model, x, y) = basic_model();
/// ```
///
/// It has two variables:
///  - x in \[-1,7\]
///  - y in \[-4,6\]
pub(super) fn basic_model() -> (
    Model<String>,
    IVar,
    IVar,
) {
    let mut model = Model::new();

    let x = model.new_ivar(-1, 7, "x".to_string());
    let y = model.new_ivar(-4, 6, "y".to_string());

    (model, x, y)
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
    let (model, x, y) = basic_model();

    let c_x = 3;
    let c_y = 2;
    let b = 13;

    let items = vec![
        NFLinearSumItem {
            var: x.into(),
            factor: c_x,
        },
        NFLinearSumItem {
            var: y.into(),
            factor: c_y,
        },
    ];

    (model, items, x, y, c_x, c_y, b)
}
