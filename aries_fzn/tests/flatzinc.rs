//! Test solver against flatzinc files.
//!
//! Each (fzn,dzn) pair with the same name in output directory gives a test.
//! The parser is also tested on the malformed flatzic files in error directory.

use aries::solver::search::beta::BetaBrancher;
use aries::solver::search::beta::restart::Never;
use aries::solver::search::beta::value_order::Min;
use aries::solver::search::beta::var_order::Lexical;
use aries_fzn::aries::Solver;
use aries_fzn::fzn::parser::parse_model;
use aries_fzn::fzn::solution::make_output_flow;
use test_each_file::test_each_file;

test_each_file! { for ["fzn", "dzn"] in "./aries_fzn/tests/output" as output => test_output }
test_each_file! { for ["fzn"] in "./aries_fzn/tests/error" as error => test_error }

/// Test the solver on the given flatzinc input.
fn test_output([input, output]: [&str; 2]) {
    let model = parse_model(input).expect("parsing error");
    dbg!(&model);

    let mut solver = Solver::new(model);

    // Hard set the configuration so the solution order is fixed
    // It should not be changed since all fzn-dzn pairs rely on this
    solver.set_brancher(Box::new(BetaBrancher::new(
        Lexical.into(),
        Min.into(),
        Never.into(),
    )));

    let mut actual_output = String::new();
    let store = |s: String| actual_output += s.as_str();
    make_output_flow(&solver, store).expect("solving error");

    assert_eq!(actual_output, output);
}

/// Test the parser raises an error.
fn test_error([input]: [&str; 1]) {
    if let Ok(model) = parse_model(input) {
        dbg!(&model);
        panic!("parsing should have failed")
    }
}
