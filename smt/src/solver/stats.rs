use std::fmt::{Display, Error, Formatter};

use crate::cpu_time::*;

/// Statistics of the solver. All times are in seconds.
pub struct Stats {
    /// Time spent in building hte constraints and initializing the theories
    pub init_time: Duration,
    pub solve_time: Duration,
    pub num_decisions: u64,
    pub num_conflicts: u64,
    pub num_restarts: u64,
    pub propagation_time: Duration,
    // First module is sat solver, other are the theories
    pub per_module_propagation_time: Vec<Duration>,
    pub per_module_conflicts: Vec<u64>,
    pub per_module_propagation_loops: Vec<u64>,
}

impl Stats {
    pub fn new() -> Stats {
        Stats {
            init_time: Duration::zero(),
            solve_time: Duration::zero(),
            num_decisions: 0,
            num_conflicts: 0,
            num_restarts: 0,
            propagation_time: Duration::zero(),
            per_module_propagation_time: vec![Duration::zero()],
            per_module_conflicts: vec![0],
            per_module_propagation_loops: vec![0],
        }
    }
}

impl Default for Stats {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for Stats {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        fn label(f: &mut Formatter<'_>, label: &str) -> Result<(), Error> {
            write!(f, "{:<20}: ", label)
        }
        fn val_throughput(f: &mut Formatter<'_>, value: u64, time: &Duration) -> Result<(), Error> {
            write!(
                f,
                "{:<12} ({:.0} /sec)",
                value,
                time.as_secs().map(|time| (value as f64) / time).unwrap_or(f64::NAN)
            )
        }
        fn new_line(f: &mut Formatter<'_>) -> Result<(), Error> {
            f.write_str("\n")
        }

        label(f, "restarts")?;
        writeln!(f, "{:<12}", self.num_restarts)?;

        label(f, "decisions")?;
        val_throughput(f, self.num_decisions, &self.solve_time)?;
        new_line(f)?;

        label(f, "conflicts")?;
        val_throughput(f, self.num_conflicts, &self.solve_time)?;
        new_line(f)?;

        writeln!(f, "================= ")?;
        label(f, "Solvers")?;
        write!(f, "{:>15}", "SAT")?;
        for i in 1..self.per_module_propagation_time.len() {
            write!(f, "{:>15}", format!("Theory({})", i))?;
        }
        if let Some(total_propagation_time) = self.propagation_time.as_secs() {
            new_line(f)?;
            label(f, "Propagation time (s)")?;
            for prop_time in &self.per_module_propagation_time {
                write!(f, "{:>15}", format!("{:.6}", prop_time))?;
            }
            new_line(f)?;
            label(f, "% propagation time")?;
            for prop_time in &self.per_module_propagation_time {
                let portion = if let Some(prop_time) = prop_time.as_secs() {
                    format!("{:.1}", prop_time / total_propagation_time)
                } else {
                    "???".to_string()
                };
                write!(f, "{:>13} %", portion)?;
            }
        }
        new_line(f)?;
        label(f, "# propagation loops")?;
        for loops in &self.per_module_propagation_loops {
            write!(f, "{:>15}", loops)?;
        }
        new_line(f)?;
        label(f, "# conflicts")?;
        for loops in &self.per_module_conflicts {
            write!(f, "{:>15}", loops)?;
        }

        if SUPPORT_CPU_TIMING {
            writeln!(f, "\n================= ")?;

            label(f, "Init time")?;
            writeln!(f, "{:.6} s", self.init_time)?;

            label(f, "Propagation time")?;
            writeln!(f, "{:.6} s", self.propagation_time)?;

            label(f, "Solve time")?;
            writeln!(f, "{:.6} s", self.solve_time)?;
        }
        Ok(())
    }
}
