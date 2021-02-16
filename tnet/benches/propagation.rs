use aries_model::bounds::{Bound, Relation};
use aries_model::lang::IntCst;
use aries_tnet::stn::STN;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::prelude::SliceRandom;
use rand::Rng;

const DOMAIN_MAX: IntCst = 100000;

fn propagate_bounds(mut stn: STN, updates: &[Bound]) {
    for &b in updates {
        stn.model.discrete.decide(b).unwrap();
        stn.propagate_all().unwrap();
    }
}

fn left_right_linear_graph() -> (GraphName, STN, Vec<Bound>) {
    let mut stn = STN::new();

    let mut timepoints = Vec::new();
    for _ in 0..100 {
        let tp = stn.add_timepoint(0, DOMAIN_MAX);
        if let Some(prev) = timepoints.last().copied() {
            stn.add_edge(tp, prev, -1);
        }

        timepoints.push(tp);
    }
    let first = *timepoints.first().unwrap();
    let last = *timepoints.last().unwrap();
    let mut updates = Vec::new();
    for i in 0..500 {
        updates.push(Bound::geq(first, i));
        updates.push(Bound::leq(last, DOMAIN_MAX - i));
    }
    ("LR-LIN", stn, updates)
}

type GraphName = &'static str;

fn left_right_random_graph() -> (GraphName, STN, Vec<Bound>) {
    let mut rng = rand::thread_rng();
    let mut stn = STN::new();

    let mut timepoints = Vec::new();
    for i in 0..100 {
        let tp = stn.add_timepoint(0, DOMAIN_MAX);

        if i > 0 {
            let num_edges = rng.gen_range(0..10);
            for _ in 0..num_edges {
                let before = timepoints[rng.gen_range(0..i)];
                let delay = rng.gen_range(0..10);
                stn.add_edge(tp, before, -delay);
            }
        }

        timepoints.push(tp);
    }
    let first = *timepoints.first().unwrap();
    let last = *timepoints.last().unwrap();
    let mut updates = Vec::new();
    for i in 0..500 {
        updates.push(Bound::geq(first, i));
        updates.push(Bound::leq(last, DOMAIN_MAX - i));
    }
    ("LR-RAND", stn, updates)
}

fn edge_activations_random_graph() -> (GraphName, STN, Vec<Bound>) {
    let mut rng = rand::thread_rng();
    let mut stn = STN::new();

    let mut timepoints = Vec::new();
    let mut activations = Vec::new();
    for i in 0..100 {
        let tp = stn.add_timepoint(0, DOMAIN_MAX);

        if i > 0 {
            let num_edges = rng.gen_range(0..10);
            for _ in 0..num_edges {
                let before = timepoints[rng.gen_range(0..i)];
                let delay = rng.gen_range(0..10);
                let trigger = stn.add_inactive_edge(tp, before, -delay);
                activations.push(trigger);
            }
        }

        timepoints.push(tp);
    }

    activations.shuffle(&mut rng);

    ("ACTIVATIONS-LR-RAND", stn, activations)
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let graphs = vec![
        left_right_linear_graph(),
        left_right_random_graph(),
        edge_activations_random_graph(),
    ];

    for (name, stn, bounds) in graphs {
        c.bench_function(&format!("stn-{}-lb-ub", name), |b| {
            b.iter(|| propagate_bounds(black_box(stn.clone()), black_box(&bounds)))
        });

        // lower bounds only
        let bounds_subset: Vec<_> = bounds
            .iter()
            .copied()
            .filter(|b| b.relation() == Relation::GT)
            .collect();

        if !bounds_subset.is_empty() && bounds_subset.len() != bounds.len() {
            c.bench_function(&format!("stn-{}-lb", name), |b| {
                b.iter(|| propagate_bounds(black_box(stn.clone()), black_box(&bounds_subset)))
            });
        }

        // upper bounds only
        let bounds_subset: Vec<_> = bounds
            .iter()
            .copied()
            .filter(|b| b.relation() == Relation::LEQ)
            .collect();
        if !bounds_subset.is_empty() && bounds_subset.len() != bounds.len() {
            c.bench_function(&format!("stn-{}-ub", name), |b| {
                b.iter(|| propagate_bounds(black_box(stn.clone()), black_box(&bounds_subset)))
            });
        }
    }
}

criterion_group!(benches, criterion_benchmark);

criterion_main!(benches);
