use log::info;

#[derive(Default, Debug)]
pub struct Stats {
    pub solves: u64,
    pub restarts: u64,
    pub decisions: u64,
    pub rnd_decisions: u64,
    pub conflicts: u64,
    pub propagations: u64,
    pub tot_literals: u64,
    pub del_literals: u64,
}

pub fn print_stats(stats: &Stats, cpu_time: f64) {
    info!("restarts              : {:<12}", stats.restarts);
    info!(
        "conflicts             : {:<12}   ({:.0} /sec)",
        stats.conflicts,
        (stats.conflicts as f64) / cpu_time
    );

    info!(
        "decisions             : {:<12}   ({:4.2} % random) ({:.0} /sec)",
        stats.decisions,
        (stats.rnd_decisions as f64) * 100.0 / (stats.decisions as f64),
        (stats.decisions as f64) / cpu_time
    );

    info!(
        "propagations          : {:<12}   ({:.0} /sec)",
        stats.propagations,
        (stats.propagations as f64) / cpu_time
    );

    info!(
        "conflict literals     : {:<12}   ({:4.2} % deleted)",
        stats.tot_literals,
        (stats.del_literals as f64) * 100.0 / ((stats.del_literals + stats.tot_literals) as f64)
    );

    info!("Memory used           : {:.2} MB", 0.0);
    info!("CPU time              : {} s", cpu_time);
    info!("");
}
