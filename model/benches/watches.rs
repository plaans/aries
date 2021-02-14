use aries_model::bounds::{Bound, Watches};
use aries_model::Model;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::prelude::SliceRandom;

fn count_watches(xs: &[Bound], watches: &Watches<u32>) -> usize {
    let mut count = 0;
    for &x in xs {
        count += watches.watches_on(x).count();
    }
    count
}

fn insert_all_watches(bounds: &[Bound]) -> Watches<u32> {
    let mut watches = Watches::new();
    for (i, &b) in bounds.iter().enumerate() {
        watches.add_watch(i as u32, b);
    }
    watches
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut model = Model::new();
    let mut bounds = Vec::new();

    for _ in 0..50 {
        let var = model.new_ivar(0, 100, "");
        for v in -20..20 {
            bounds.push(Bound::leq(var, v));
            bounds.push(Bound::geq(var, v));
        }
    }

    c.bench_function("insertion-watches-deterministic-order", |b| {
        b.iter(|| insert_all_watches(black_box(&bounds)))
    });

    let watches = insert_all_watches(&bounds);

    c.bench_function("count-watches-deterministic-order", |b| {
        b.iter(|| count_watches(black_box(&bounds), black_box(&watches)))
    });

    // shuffle bounds

    let mut rng = rand::thread_rng();
    bounds.shuffle(&mut rng);

    c.bench_function("insertion-watches-random-order", |b| {
        b.iter(|| insert_all_watches(black_box(&bounds)))
    });

    let watches = insert_all_watches(&bounds);

    c.bench_function("watches-random-order", |b| {
        b.iter(|| count_watches(black_box(&bounds), black_box(&watches)))
    });
}

criterion_group!(benches, criterion_benchmark);

criterion_main!(benches);
