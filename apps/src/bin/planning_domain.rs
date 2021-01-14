use std::path::PathBuf;
use structopt::StructOpt;

/// Attempts to find the corresponding domain file of a given PDDL/HDDL problem file.
#[derive(Debug, StructOpt)]
#[structopt(name = "planning_domain", rename_all = "kebab-case")]
struct Opt {
    problem_file: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    match aries::find_domain_of(&opt.problem_file) {
        Ok(path) => {
            print!("{}", path.display());
            Ok(())
        }
        Err(e) => Err(e),
    }
}
