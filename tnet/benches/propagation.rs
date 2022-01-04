use aries_core::literals::{Lit, Relation};
use aries_model::lang::IntCst;
use aries_tnet::theory::Stn;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::prelude::SliceRandom;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const DOMAIN_MAX: IntCst = 100000;

fn propagate_bounds(mut stn: Stn, updates: &[Lit]) {
    for &b in updates {
        stn.model.discrete.decide(b).unwrap();
        stn.propagate_all().unwrap();
    }
}

fn left_right_linear_graph() -> (GraphName, Stn, Vec<Lit>) {
    let mut stn = Stn::new();

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
        updates.push(Lit::geq(first, i));
        updates.push(Lit::leq(last, DOMAIN_MAX - i));
    }
    ("LR-LIN", stn, updates)
}

type GraphName = &'static str;

fn left_right_random_graph() -> (GraphName, Stn, Vec<Lit>) {
    let mut rng = StdRng::seed_from_u64(9849879857498574);
    let mut stn = Stn::new();

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
        updates.push(Lit::geq(first, i));
        updates.push(Lit::leq(last, DOMAIN_MAX - i));
    }
    ("LR-RAND", stn, updates)
}

fn edge_activations_random_graph() -> (GraphName, Stn, Vec<Lit>) {
    let mut rng = StdRng::seed_from_u64(9820942423043434);
    let mut stn = Stn::new();

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

        // lower literals only
        let bounds_subset: Vec<_> = bounds
            .iter()
            .copied()
            .filter(|b| b.relation() == Relation::Gt)
            .collect();

        if !bounds_subset.is_empty() && bounds_subset.len() != bounds.len() {
            c.bench_function(&format!("stn-{}-lb", name), |b| {
                b.iter(|| propagate_bounds(black_box(stn.clone()), black_box(&bounds_subset)))
            });
        }

        // upper literals only
        let bounds_subset: Vec<_> = bounds
            .iter()
            .copied()
            .filter(|b| b.relation() == Relation::Leq)
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
