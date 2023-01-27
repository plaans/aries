use std::fmt::{Display, Error, Formatter};

use crate::cpu_time::*;
use aries_backtrack::DecLvl;
use aries_core::{IntCst, Lit};
use env_param::EnvParam;
use std::time::Duration;

static PRINT_RUNNING_STATS: EnvParam<bool> = EnvParam::new("ARIES_PRINT_RUNNING_STATS", "false");

/// Statistics of the solver. All times are in seconds.
#[derive(Clone)]
pub struct Stats {
    /// Time spent in building hte constraints and initializing the theories
    pub init_time: Duration,
    pub init_cycles: CycleCount,
    pub solve_time: Duration,
    pub solve_cycles: CycleCount,
    num_decisions: u64,
    num_conflicts: u64,
    num_restarts: u64,
    num_solutions: u64,
    pub propagation_time: CycleCount,
    // First module is sat solver, other are the theories
    pub per_module_propagation_time: Vec<CycleCount>,
    pub per_module_conflicts: Vec<u64>,
    pub per_module_propagation_loops: Vec<u64>,
    running: RunningStats,
    best_cost: Option<IntCst>,
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
            running: Default::default(),
            best_cost: None,
        }
    }

    pub fn add_decision(&mut self, _decision: Lit) {
        self.num_decisions += 1;
        self.running.add_decision();
    }

    pub fn add_conflict(&mut self, depth: DecLvl, size: usize) {
        self.num_conflicts += 1;
        self.running.add_conflict(size, depth);
        if self.running.count == 1000 {
            self.print_running(" ");
        }
    }

    pub fn add_solution(&mut self, cost: IntCst) {
        self.num_solutions += 1;
        self.best_cost = Some(cost);
        self.print_running("*");
    }

    pub fn add_restart(&mut self) {
        self.num_restarts += 1;
        self.print_running("<");
    }

    pub fn print_running(&mut self, first: &str) {
        if PRINT_RUNNING_STATS.get() {
            let line = [
                self.best_cost.map_or("-".to_string(), |c| c.to_string()),
                self.running.avg(self.running.depth),
                self.running.avg(self.running.size),
                self.running.avg(self.running.decisions),
            ];
            print!("{first}");
            for cell in line {
                print!(" {cell:>8}");
            }
            println!();
        }
        self.running.clear();
    }

    pub fn num_conflicts(&self) -> u64 {
        self.num_conflicts
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
            write!(f, "{label:<20}: ")
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
            write!(f, "{:>15}", format!("Theory({i})"))?;
        }

        if SUPPORT_CPU_TIMING {
            new_line(f)?;
            label(f, "% propagation cycles")?;
            for &prop_time in &self.per_module_propagation_time {
                let portion = format!("{}", prop_time / self.propagation_time);
                write!(f, "{portion:>15}")?;
            }
        }
        new_line(f)?;
        label(f, "# propagation loops")?;
        for loops in &self.per_module_propagation_loops {
            write!(f, "{loops:>15}")?;
        }
        new_line(f)?;
        label(f, "# conflicts")?;
        for loops in &self.per_module_conflicts {
            write!(f, "{loops:>15}")?;
        }

        writeln!(f, "\n================= ")?;

        label(f, "Init time")?;
        writeln!(f, "{:.6} s", self.init_time.as_secs_f64())?;

        label(f, "Solve time")?;
        writeln!(f, "{:.6} s", self.solve_time.as_secs_f64())?;

        Ok(())
    }
}

#[derive(Default, Copy, Clone)]
struct RunningStats {
    count: u64,
    // lbd: u64,
    size: u64,
    depth: u64,
    decisions: u64,
}
impl RunningStats {
    fn add_decision(&mut self) {
        self.decisions += 1;
    }
    fn add_conflict(&mut self, size: usize, depth: DecLvl) {
        self.count += 1;
        self.size += size as u64;
        // self.lbd += lbd as u64;
        self.depth += depth.to_int() as u64;
    }
    pub fn avg(&self, measure: u64) -> String {
        let avg = (measure as f32) / (self.count as f32);
        format!("{avg:.2}")
    }
    pub fn clear(&mut self) {
        *self = Default::default()
    }
}
