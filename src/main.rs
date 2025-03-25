use aries_fzn::cli::parse_args;
use aries_fzn::cli::run;

fn main() {
    let args = parse_args();
    run(&args);
}
