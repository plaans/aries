use crate::backtrack::DecLvl;
use crate::core::{IntCst, Lit};
use crate::reasoners::ReasonerId;
use crate::reasoners::REASONERS;
use crate::utils::cpu_time::*;
use env_param::EnvParam;
use std::collections::BTreeMap;
use std::fmt::{Display, Error, Formatter};
use std::ops::{Index, IndexMut};
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
    pub num_decisions: u64,
    pub num_conflicts: u64,
    pub num_restarts: u64,
    pub num_solutions: u64,
    pub propagation_time: CycleCount,
    pub per_module_stat: BTreeMap<ReasonerId, ModuleStat>,
    running: RunningStats,
    best_cost: Option<IntCst>,
}

#[derive(Clone, Default)]
pub struct ModuleStat {
    pub propagation_time: CycleCount,
    pub conflicts: u64,
    pub propagation_loops: u64,
}

impl Stats {
    pub fn new() -> Stats {
        let mut per_mod = BTreeMap::new();
        for id in &REASONERS {
            per_mod.insert(*id, ModuleStat::default());
        }

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
            per_module_stat: per_mod,
            running: Default::default(),
            best_cost: None,
        }
    }

    pub fn add_decision(&mut self, _decision: Lit) {
        self.num_decisions += 1;
        self.running.add_decision();
    }

    pub fn add_conflict(&mut self, depth: DecLvl, size: usize, lbd: u32) {
        self.num_conflicts += 1;
        self.running.add_conflict(size, depth, lbd);
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
                self.running.avg(self.running.lbd),
                format!("{:.1}", (1f64 / self.running.search_space_left).log2()),
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
        for i in self.per_module_stat.keys() {
            write!(f, "{:>15}", format!("{i}"))?;
        }

        if SUPPORT_CPU_TIMING {
            new_line(f)?;
            label(f, "% propagation cycles")?;
            for ms in self.per_module_stat.values() {
                let portion = format!("{}", ms.propagation_time / self.propagation_time);
                write!(f, "{portion:>15}")?;
            }
        }
        new_line(f)?;
        label(f, "# propagation loops")?;
        for ms in self.per_module_stat.values() {
            write!(f, "{:>15}", ms.propagation_loops)?;
        }
        new_line(f)?;
        label(f, "# conflicts")?;
        for ms in self.per_module_stat.values() {
            write!(f, "{:>15}", ms.conflicts)?;
        }

        writeln!(f, "\n================= ")?;

        label(f, "Init time")?;
        writeln!(f, "{:.6} s", self.init_time.as_secs_f64())?;

        label(f, "Solve time")?;
        writeln!(f, "{:.6} s", self.solve_time.as_secs_f64())?;

        Ok(())
    }
}

#[derive(Copy, Clone)]
struct RunningStats {
    count: u64,
    lbd: u64,
    search_space_left: f64,
    size: u64,
    depth: u64,
    decisions: u64,
}

impl Default for RunningStats {
    fn default() -> Self {
        Self {
            count: 0,
            lbd: 0,
            search_space_left: 1f64,
            size: 0,
            depth: 0,
            decisions: 0,
        }
    }
}

impl RunningStats {
    fn add_decision(&mut self) {
        self.decisions += 1;
    }
    fn add_conflict(&mut self, size: usize, depth: DecLvl, lbd: u32) {
        self.count += 1;
        self.size += size as u64;
        self.lbd += lbd as u64;
        self.search_space_left *= 1f64 - 1f64 / (2f64.powf(lbd as f64));
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

impl Index<ReasonerId> for Stats {
    type Output = ModuleStat;

    fn index(&self, index: ReasonerId) -> &Self::Output {
        &self.per_module_stat[&index]
    }
}

impl IndexMut<ReasonerId> for Stats {
    fn index_mut(&mut self, index: ReasonerId) -> &mut Self::Output {
        self.per_module_stat.get_mut(&index).unwrap()
    }
}
