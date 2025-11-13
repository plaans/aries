//! This module provides utility function to find domain/problem files based on naming conventions.

use std::path::{Path, PathBuf};

use regex::Regex;

use crate::{Message, Res, errors::Ctx};

/// Attempts to find the corresponding domain file for the given PDDL/HDDL problem.
/// This method will look for a file named `domain.pddl` (resp. `domain.hddl`) in the
/// current and parent folders.
pub fn find_domain_of(problem_file: &std::path::Path) -> Res<PathBuf> {
    // these are the domain file names that we will look for in the current and parent directory
    let mut candidate_domain_files = Vec::with_capacity(2);

    // add domain.pddl or domain.hddl
    candidate_domain_files.push(match problem_file.extension() {
        Some(ext) => Path::new("domain").with_extension(ext),
        None => Path::new("domain.pddl").to_path_buf(),
    });

    let problem_filename = problem_file
        .file_name()
        .title("Invalid file")?
        .to_str()
        .title("Could not convert file name to utf8")?;

    // if the problem file is of the form XXXXX.YY.pb.Zddl
    // then add XXXXX.dom.Zddl to the candidate filenames
    let re = Regex::new("^(.+)(\\.[^\\.]+)\\.pb\\.([hp]ddl)$").unwrap();
    for m in re.captures_iter(problem_filename) {
        let name = format!("{}.dom.{}", &m[1], &m[3]);
        candidate_domain_files.push(name.into());
    }
    // if the problem file is of the form XXXXX.pb.Zddl,
    // then add XXXXX.dom.Zddl to the candidate filenames
    let re = Regex::new("^(.+)\\.pb\\.([hp]ddl)$").unwrap();
    for m in re.captures_iter(problem_filename) {
        let name = format!("{}.dom.{}", &m[1], &m[2]);
        candidate_domain_files.push(name.into());
    }
    // if the problem file is of the form XXXXX.Zddl
    // then add XXXXX-domain.Zddl to the candidate filenames
    let re = Regex::new("^(.+)\\.([hp]ddl)$").unwrap();
    for m in re.captures_iter(problem_filename) {
        let name = format!("{}-domain.{}", &m[1], &m[2]);
        candidate_domain_files.push(name.into());
        let name = format!("domain-{}.{}", &m[1], &m[2]);
        candidate_domain_files.push(name.into());
    }

    // if the problem if of the form instance-NN.Zddl
    // the add domain-NN.Zddl to the candidates filenames
    let re = Regex::new("^instance-([1-9]+)\\.([hp]ddl)$").unwrap();
    for m in re.captures_iter(problem_filename) {
        let name = format!("domain-{}.{}", &m[1], &m[2]);
        candidate_domain_files.push(name.into());
    }

    // directories where to look for the domain
    let mut candidate_directories = Vec::with_capacity(2);
    if let Some(curr) = problem_file.parent() {
        candidate_directories.push(curr.to_owned());
        if let Some(parent) = curr.parent() {
            candidate_directories.push(parent.to_owned());
            candidate_directories.push(parent.join("domains"));
        }
    }

    for f in &candidate_domain_files {
        for dir in &candidate_directories {
            let candidate = dir.join(f);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    Err(Message::error(format!(
        "Could not find find a corresponding file in same or parent directory as the problem file. Candidates: {:?}",
        candidate_domain_files
    )))
}

pub fn find_problem_of(plan_file: &std::path::Path) -> Res<PathBuf> {
    let mut candidate_problem_files = Vec::with_capacity(2);

    let plan_filename = plan_file
        .file_name()
        .title("Invalid file")?
        .to_str()
        .title("Could not convert file name to utf8")?;

    // if the plan file is of the form XXXXX.planYYYYYYYYY
    // then add XXXXX(.pb).Zddl to the candidate filenames
    let re = Regex::new("^(.+)\\.plan[^\\.]*$").unwrap();
    for m in re.captures_iter(plan_filename) {
        let base_name = &m[1];
        candidate_problem_files.push(format!("{base_name}.pb.pddl").into());
        candidate_problem_files.push(format!("{base_name}.pddl").into());
    }
    candidate_problem_files.push(Path::new("problem.pddl").to_path_buf());
    candidate_problem_files.push(Path::new("problem.hddl").to_path_buf());

    let mut candidate_directories = Vec::with_capacity(2);
    if let Some(curr) = plan_file.parent() {
        candidate_directories.push(curr.to_owned());
        if let Some(parent) = curr.parent() {
            candidate_directories.push(parent.to_owned());
        }
    }
    for f in &candidate_problem_files {
        for dir in &candidate_directories {
            let candidate = dir.join(f);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    Err(Message::error(format!(
        "Could not find find a corresponding file in same or parent directory as the problem file. Candidates: {:?}",
        candidate_problem_files
    )))
}
