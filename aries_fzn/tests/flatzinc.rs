//! Test solver against flatzinc files.
//!
//! Each (fzn,dzn) pair with the same name in flatzinc folder gives a test.

use aries_fzn::aries::Solver;
use aries_fzn::fzn::parser::parse_model;
use aries_fzn::fzn::solution::make_output_flow;
use test_each_file::test_each_file;

test_each_file! { for ["fzn", "dzn"] in "./aries_fzn/tests/flatzinc" => test }

/// Test the solver ont the given flatzinc input.
fn test([input, output]: [&str; 2]) {
    let model = parse_model(input).expect("parsing error");
    dbg!(&model);

    let solver = Solver::new(model);

    let mut actual_output = String::new();
    let store = |s: String| actual_output += s.as_str();
    make_output_flow(&solver, store).expect("solving error");

    assert_eq!(actual_output, output);
}
