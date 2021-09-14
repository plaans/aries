use std::fmt::{Display, Error, Formatter};

use crate::cpu_time::*;
use std::time::Duration;

/// Statistics of the solver. All times are in seconds.
#[derive(Clone)]
pub struct Stats {
    /// Time spent in building hte constraints and initializing the theories
    pub init_time: Duration,
    pub init_cycles: CycleCount,
    pub solve_time: Duration,
    pub solve_cycles: CycleCount,
    pub num_decisions: u64,
    pub num_conflicts: u64,
    pub num_restarts: u64,
    pub num_solutions: u64,
    pub propagation_time: CycleCount,
    // First module is sat solver, other are the theories
    pub per_module_propagation_time: Vec<CycleCount>,
    pub per_module_conflicts: Vec<u64>,
    pub per_module_propagation_loops: Vec<u64>,
}

impl Stats {
    pub fn new() -> Stats {
        Stats {
            init_time: Duration::from_micros(0),
            init_cycles: CycleCount::zero(),
            solve_time: Duration::from_micros(0),
            solve_cycles: CycleCount::zero(),
            num_decisions: 0,
            num_conflicts: 0,
            num_restarts: 0,
            num_solutions: 0,
            propagation_time: CycleCount::zero(),
            per_module_propagation_time: vec![CycleCount::zero()],
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
            write!(f, "{:<12} ({:.0} /sec)", value, (value as f64) / time.as_secs_f64())
        }
        fn new_line(f: &mut Formatter<'_>) -> Result<(), Error> {
            f.write_str("\n")
        }

        label(f, "solutions")?;
        writeln!(f, "{:<12}", self.num_solutions)?;

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

        if SUPPORT_CPU_TIMING {
            new_line(f)?;
            label(f, "% propagation cycles")?;
            for &prop_time in &self.per_module_propagation_time {
                let portion = format!("{}", prop_time / self.propagation_time);
                write!(f, "{:>15}", portion)?;
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

        writeln!(f, "\n================= ")?;

        label(f, "Init time")?;
        writeln!(f, "{:.6} s", self.init_time.as_secs_f64())?;

        label(f, "Solve time")?;
        writeln!(f, "{:.6} s", self.solve_time.as_secs_f64())?;

        Ok(())
    }
}
