//! Test solver against flatzinc files.
//!
//! Each (fzn,dzn) pair with the same name in flatzinc folder
//! gives a test.

use aries_fzn::aries::Solver;
use aries_fzn::fzn::output::make_output;
use aries_fzn::fzn::parser::parse_model;
use test_each_file::test_each_file;

test_each_file! { for ["fzn", "dzn"] in "./aries_fzn/tests/flatzinc" => test }

/// Test the solver ont the given flatzinc input.
fn test([input, output]: [&str; 2]) {
    let model = parse_model(input).unwrap();
    dbg!(&model);

    let solver = Solver::new(model);

    let result = solver.solve().unwrap();
    let actual_output = make_output(result);

    assert_eq!(actual_output.as_str(), output);
}
