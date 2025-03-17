use aries_fzn::output::make_output;
use aries_fzn::parser::parse_model;
use aries_fzn::solver::Solver;
use test_each_file::test_each_file;

test_each_file! { for ["fzn", "dzn"] in "./tests/data" => test }

fn test([input, output]: [&str; 2]) {
    let model = parse_model(input).unwrap();

    let solver = Solver::new(model);

    let result = solver.solve().unwrap();
    let actual_output = make_output(result);

    assert_eq!(actual_output.as_str(), output);
}
