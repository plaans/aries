use clap::{Args, Parser, Subcommand};

#[derive(Debug, Clone, Parser)]
#[clap(about = "Beluga CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    Solve(SolveArgs),
    Explain(ExplainArgs),
}

// #[derive(Debug, Clone, Args)]
// pub struct ProblemFile {
//     #[clap(long = "relative", short, action)]
//     pub is_path_relative: bool,
//     pub path: String,
// }

#[derive(Debug, Clone, Args)]
pub struct SolveArgs {
    // #[clap(flatten)]
    // pub problem_file: ProblemFile,
    pub problem_file_path: String,
}

#[derive(Debug, Clone, Args)]
pub struct ExplainArgs {
    // #[clap(flatten)]
    // pub problem_file: ProblemFile,
    pub problem_file_path: String,
    pub question_name: String,
    pub question_args: Vec<String>,
}
