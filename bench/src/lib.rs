use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::{collections::BTreeMap, time::Duration};

use crate::time_series::TimeSerie;

pub mod comp;
//#[cfg(feature = "plot")]
pub mod plot;
pub mod results;
pub mod time_series;

/// Identifier of solver (typically a string derive from the location of its results)
pub type SolverID = String;

/// Characterization of a benchmark problem
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct Problem {
    /// Name of the problem (often a path in the folder containing the benchmarks)
    pub name: String,
    /// Maximum time given to the solver.
    pub timeout: Duration,
    pub flags: BTreeMap<String, String>,
}

impl Problem {
    /// A unique identifier of the problem.
    pub fn id(&self) -> String {
        let mut id = self.name.clone();
        write!(id, "__to:{}s", self.timeout.as_secs_f32()).unwrap();
        for (k, v) in &self.flags {
            write!(id, "_{k}:{v}").unwrap()
        }
        id
    }

    /// Generates a filesystem-safe filename from the problem ID
    pub fn filename(&self) -> String {
        // Replace problematic characters with underscores
        let normalized = self
            .id()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();

        // Ensure the filename is not empty and ends with .json
        if normalized.is_empty() {
            "problem.json".to_string()
        } else {
            format!("{}.json", normalized)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, PartialOrd, Clone, Copy)]
pub enum SolveStatus {
    Solved,
    Timeout,
}

pub type MetricValue = i64;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct IntermediateResult {
    pub timestamp: Duration,
    pub objective: MetricValue,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SolveResult {
    pub problem: Problem,
    pub status: SolveStatus,
    pub runtime: Duration,
    pub objective_value: Option<i64>,
    pub metrics: BTreeMap<Metric, f64>,
    pub objective_history: Vec<IntermediateResult>,
}

impl SolveResult {
    /// Serializes the result to a new file in the indicated directory.
    /// The name of the file is derived from the problem ([`Problem::filename`]).
    /// If the directory does not exist, it will be created.
    /// If a file with the same name already exists, it will be overwritten.
    pub fn save_to_dir(&self, dir: &str) -> Result<()> {
        // Create the directory if it doesn't exist
        std::fs::create_dir_all(dir).context("Failed to create directory")?;

        // Generate a filesystem-safe filename from the problem
        let filename = self.problem.filename();
        let file_path = std::path::Path::new(dir).join(filename);
        let file_path_str = file_path.to_str().context("Failed to convert file path to string")?;

        // Save to file
        self.save_to_file(file_path_str)
            .context("Failed to save result to file")?;

        Ok(())
    }

    pub fn serialize(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn save_to_file(&self, file: &str) -> Result<()> {
        let serialized = self.serialize().context("Failed to serialize result")?;
        std::fs::write(file, serialized).context("Failed to write to file")?;
        Ok(())
    }

    pub fn deserialize(data: &str) -> Result<Self> {
        serde_json::from_str(data).context("Failed to deserialize result")
    }

    pub fn load_from_file(file: &str) -> Result<Self> {
        let content = std::fs::read_to_string(file).context("Failed to read file")?;
        SolveResult::deserialize(&content).context("Failed to deserialize file content")
    }

    pub fn with_metric(mut self, metric: Metric, value: impl Into<f64>) -> Self {
        self.metrics.insert(metric, value.into());
        self
    }

    pub fn objective_hist(&self) -> TimeSerie {
        let mut hist = vec![];
        for measure in &self.objective_history {
            hist.push((measure.timestamp, measure.objective as f64));
        }
        if let Some(final_obj) = self.objective_value {
            hist.push((self.runtime, final_obj as f64));
        }
        TimeSerie::from_constant_per_part(hist, self.problem.timeout)
    }

    pub fn ipc_history(&self, best: MetricValue) -> TimeSerie {
        let best = best as f64;
        let mut hist = vec![(Duration::ZERO, 0.0)];
        for measure in &self.objective_history {
            let x = best / measure.objective as f64;
            assert!(x.is_finite(), "{best} / {}", measure.objective);
            hist.push((measure.timestamp, best / measure.objective as f64));
        }
        if let Some(final_obj) = self.objective_value {
            hist.push((self.runtime, best / final_obj as f64));
        }
        TimeSerie::from_constant_per_part(hist, self.problem.timeout)
    }

    pub fn solved_hist(&self) -> TimeSerie {
        let mut hist = vec![(Duration::ZERO, 0.0)];
        if self.status == SolveStatus::Solved {
            hist.push((self.runtime, 1.0));
        }
        TimeSerie::from_constant_per_part(hist, self.problem.timeout)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub enum Metric {
    NumConflicts,
    NumDecisions,
    NumDomUpdates,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::tempdir;

    fn create_test_solve_result() -> SolveResult {
        let mut metrics = BTreeMap::new();
        metrics.insert(Metric::NumConflicts, 10.0);
        metrics.insert(Metric::NumDecisions, 5.0);
        metrics.insert(Metric::NumDomUpdates, 3.0);

        SolveResult {
            problem: Problem {
                name: "test_problem".to_string(),
                timeout: Duration::from_secs(60),
                flags: Default::default(),
            },
            status: SolveStatus::Solved,
            runtime: Duration::from_secs(5),
            objective_value: Some(42),
            metrics,
            objective_history: vec![],
        }
    }

    #[test]
    fn test_serialize() -> Result<()> {
        let result = create_test_solve_result();
        let serialized = result.serialize()?;
        assert!(serialized.contains("test_problem"));
        assert!(serialized.contains("Solved"));
        Ok(())
    }

    #[test]
    fn test_deserialize() -> Result<()> {
        let result = create_test_solve_result();
        let serialized = result.serialize()?;
        let deserialized = SolveResult::deserialize(&serialized)?;
        assert_eq!(deserialized.problem.name, "test_problem");
        assert_eq!(deserialized.objective_value, Some(42));
        Ok(())
    }

    #[test]
    fn test_save_and_load_to_file() -> Result<()> {
        let result = create_test_solve_result();
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_result.json");
        let file_path_str = file_path.to_str().unwrap();

        // Test save_to_file
        result.save_to_file(file_path_str)?;

        // Test load_from_file
        let loaded_result = SolveResult::load_from_file(file_path_str)?;

        // Verify the loaded data matches the original
        assert_eq!(loaded_result.problem.name, result.problem.name);
        assert_eq!(loaded_result.status, result.status);
        assert_eq!(loaded_result.objective_value, result.objective_value);
        assert_eq!(loaded_result.metrics.len(), result.metrics.len());
        Ok(())
    }

    #[test]
    fn test_roundtrip() -> Result<()> {
        let result = create_test_solve_result();
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("roundtrip_test.json");
        let file_path_str = file_path.to_str().unwrap();

        // Save, load, and compare
        result.save_to_file(file_path_str)?;
        let loaded = SolveResult::load_from_file(file_path_str)?;

        // Check that all fields are preserved
        assert_eq!(result.problem.name, loaded.problem.name);
        assert_eq!(result.problem.timeout, loaded.problem.timeout);
        assert_eq!(result.problem.flags, loaded.problem.flags);
        assert_eq!(result.status, loaded.status);
        assert_eq!(result.runtime, loaded.runtime);
        assert_eq!(result.objective_value, loaded.objective_value);
        assert_eq!(result.metrics, loaded.metrics);
        Ok(())
    }

    #[test]
    fn test_save_to_dir() -> Result<()> {
        let result = create_test_solve_result();
        let temp_dir = tempdir().unwrap();
        let dir_path = temp_dir.path();
        let dir_path_str = dir_path.to_str().unwrap();

        // Test save_to_dir
        result.save_to_dir(dir_path_str)?;

        // Check that the file was created with the expected name
        let expected_filename = result.problem.filename();
        let expected_file_path = dir_path.join(expected_filename);
        assert!(expected_file_path.exists());

        // Load the file and verify its contents
        let loaded = SolveResult::load_from_file(expected_file_path.to_str().unwrap())?;

        // Verify the loaded data matches the original
        assert_eq!(loaded.problem.name, result.problem.name);
        assert_eq!(loaded.objective_value, result.objective_value);
        Ok(())
    }

    #[test]
    fn test_save_to_dir_creates_directory() -> Result<()> {
        let result = create_test_solve_result();
        let temp_dir = tempdir().unwrap();
        let subdir_path = temp_dir.path().join("subdir1").join("subdir2");
        let subdir_path_str = subdir_path.to_str().unwrap();

        // Ensure the directory doesn't exist initially
        assert!(!subdir_path.exists());

        // Test save_to_dir with non-existent directory
        result.save_to_dir(subdir_path_str)?;

        // Check that the directory was created
        assert!(subdir_path.exists());

        // Check that the file was created
        let expected_filename = result.problem.filename();
        let expected_file_path = subdir_path.join(expected_filename);
        assert!(expected_file_path.exists());
        Ok(())
    }

    #[test]
    fn test_filename_normalization() {
        // Test with a problem name that contains special characters
        let mut metrics = BTreeMap::new();
        metrics.insert(Metric::NumConflicts, 10.0);

        let problem = Problem {
            name: "test/problem:with*special?chars".to_string(),
            timeout: Duration::from_secs(60),
            flags: Default::default(),
        };

        let filename = problem.filename();

        // Verify that special characters are replaced with underscores
        assert!(filename.contains("test_problem_with_special_chars"));
        assert!(filename.ends_with(".json"));

        // Verify that only safe characters remain
        for c in filename.chars() {
            assert!(c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-');
        }
    }

    #[test]
    fn test_filename_with_unicode() {
        // Test with a problem name that contains unicode characters
        let problem = Problem {
            name: "test_problem_ñáéíóú".to_string(),
            timeout: Duration::from_secs(30),
            flags: Default::default(),
        };

        let filename = problem.filename();

        // Verify that unicode characters are replaced with underscores
        assert!(filename.contains("test_problem_"));
        assert!(filename.ends_with(".json"));

        // Verify that only safe characters remain
        for c in filename.chars() {
            assert!(c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-');
        }
    }
}
