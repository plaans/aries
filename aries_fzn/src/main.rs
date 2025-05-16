use std::process::exit;

use aries_fzn::cli::parse_args;
use aries_fzn::cli::run;

fn main() {
    let args = parse_args();
    if let Err(e) = run(&args) {
        eprintln!("{e:#}");
        exit(1);
    }
}
