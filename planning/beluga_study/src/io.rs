use std::fs::File;
use std::io::{BufWriter, Write};
use serde_json::{self, json};
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
    pub results_file_path: String,
}

pub fn write_mus_mcs_enumeration_result_to_file(
    results_file_path: String,
    complete: Option<bool>,
    muses: Vec<Vec<String>>,
    mcses: Vec<Vec<String>>,
) -> std::io::Result<()> {

    // let _ = create_dir_all(&results_file_path)?;
    let file = File::create_new(&results_file_path).or_else(|_| File::create(&results_file_path))?;
    let mut writer = BufWriter::new(file);

    serde_json::to_writer(
        &mut writer,
        &json!({
            "complete": complete.as_ref(),
            "muses": muses,
            "mcses": mcses,
        }),
    )?;

    writer.flush()?;
    Ok(())
}
}
