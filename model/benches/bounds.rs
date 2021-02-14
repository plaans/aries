use aries_model::bounds::Bound;
use aries_model::Model;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::prelude::SliceRandom;

#[inline]
fn entailment(xs: &[Bound], ys: &[Bound]) -> u64 {
    let mut count = 0;
    for &x in xs {
        for &y in ys {
            if x.entails(y) {
                count += 1
            }
        }
    }
    count
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
    let mut rng = rand::thread_rng();
    bounds.shuffle(&mut rng);

    c.bench_function("bounds-entail-many-vars", |b| {
        b.iter(|| entailment(black_box(&bounds), black_box(&bounds)))
    });

    let mut bounds = Vec::new();
    for _ in 0..5 {
        let var = model.new_ivar(0, 100, "");
        for v in -20..20 {
            bounds.push(Bound::leq(var, v));
            bounds.push(Bound::geq(var, v));
        }
    }
    let mut rng = rand::thread_rng();
    bounds.shuffle(&mut rng);

    c.bench_function("bounds-entail-few-vars", |b| {
        b.iter(|| entailment(black_box(&bounds), black_box(&bounds)))
    });

    let mut bounds = Vec::new();

    let var = model.new_ivar(0, 100, "");
    for v in -40..40 {
        bounds.push(Bound::leq(var, v));
        bounds.push(Bound::geq(var, v));
    }

    let mut rng = rand::thread_rng();
    bounds.shuffle(&mut rng);

    c.bench_function("bounds-entail-one-var", |b| {
        b.iter(|| entailment(black_box(&bounds), black_box(&bounds)))
    });
}

criterion_group!(benches, criterion_benchmark);

criterion_main!(benches);
