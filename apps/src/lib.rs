use anyhow::*;
use std::path::{Path, PathBuf};

/// Attempts to find the corresponding domain file for the given PDDL/HDDL problem.
/// This method will look for a file named `domain.pddl` (resp. `domain.hddl`) in the
/// current and parent folders.
pub fn find_domain_of(problem_file: &std::path::Path) -> anyhow::Result<PathBuf> {
    let filename = match problem_file.extension() {
        Some(ext) => Path::new("domain").with_extension(ext),
        None => Path::new("domain.pddl").to_path_buf(),
    };

    let dir = problem_file.parent().unwrap();
    let candidate1 = dir.join(&filename);
    let candidate2 = dir.parent().unwrap().join(&filename);
    if candidate1.exists() {
        Ok(candidate1)
    } else if candidate2.exists() {
        Ok(candidate2)
    } else {
        bail!("Could not find find a corresponding 'domain.pddl' file in same or parent directory as the problem file.")
    }
}
