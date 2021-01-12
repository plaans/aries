use std::fmt::{Display, Error, Formatter};

#[derive(Debug)]
pub struct Stats {
    pub solves: u64,
    pub restarts: u64,
    pub decisions: u64,
    pub rnd_decisions: u64,
    pub conflicts: u64,
    pub propagations: u64,
    pub tot_literals: u64,
    pub del_literals: u64,
    pub init_time: std::time::Instant,
    pub end_time: std::time::Instant,
}
impl Default for Stats {
    fn default() -> Self {
        let now = std::time::Instant::now();
        Stats {
            solves: 0,
            restarts: 0,
            decisions: 0,
            rnd_decisions: 0,
            conflicts: 0,
            propagations: 0,
            tot_literals: 0,
            del_literals: 0,
            init_time: now,
            end_time: now,
        }
    }
}

impl Display for Stats {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let cpu_time = self.end_time - self.init_time;
        let cpu_time = cpu_time.as_secs_f64();

        writeln!(f, "restarts              : {:<12}", self.restarts)?;
        writeln!(
            f,
            "conflicts             : {:<12}   ({:.0} /sec)",
            self.conflicts,
            (self.conflicts as f64) / cpu_time
        )?;

        writeln!(
            f,
            "decisions             : {:<12}   ({:4.2} % random) ({:.0} /sec)",
            self.decisions,
            (self.rnd_decisions as f64) * 100.0 / (self.decisions as f64),
            (self.decisions as f64) / cpu_time
        )?;

        writeln!(
            f,
            "propagations          : {:<12}   ({:.0} /sec)",
            self.propagations,
            (self.propagations as f64) / cpu_time
        )?;

        writeln!(
            f,
            "conflict literals     : {:<12}   ({:4.2} % deleted)",
            self.tot_literals,
            (self.del_literals as f64) * 100.0 / ((self.del_literals + self.tot_literals) as f64)
        )?;

        writeln!(f, "Memory used           : {:.2} MB", 0.0)?;
        writeln!(f, "CPU time              : {} s", cpu_time)
    }
}
