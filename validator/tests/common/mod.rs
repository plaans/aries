use anyhow::Result;
use prost::Message;
use std::fs::read;
use std::path::PathBuf;

use unified_planning::{Plan, Problem};

fn get_bin(name: &str, folder: &str) -> Result<Vec<u8>> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../planning/ext/up/bins");
    path.push(folder);
    path.push(name);
    path.set_extension("bin");
    let bin = read(path)?;
    Ok(bin)
}

pub fn get_problem(name: &str) -> Result<Problem> {
    let bin = get_bin(name, "problems")?;
    let problem = Problem::decode(bin.as_slice())?;
    Ok(problem)
}

pub fn get_plan(name: &str) -> Result<Plan> {
    let bin = get_bin(name, "plans")?;
    let plan = Plan::decode(bin.as_slice())?;
    Ok(plan)
}
