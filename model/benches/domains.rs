use aries_model::assignments::Assignment;
use aries_model::bounds::Bound;
use aries_model::lang::IVar;
use aries_model::Model;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::prelude::SliceRandom;
use rand::Rng;

fn count_entailed(xs: &[Bound], model: &Model) -> usize {
    let mut count = 0;
    for &x in xs {
        if model.entails(x) {
            count += 1;
        }
    }
    count
}

fn count_lower_bounds(xs: &[IVar], model: &Model) -> usize {
    let mut count = 0;
    for &x in xs {
        if model.lower_bound(x) >= -100 {
            count += 1;
        }
    }
    count
}

fn count_upper_bounds(xs: &[IVar], model: &Model) -> usize {
    let mut count = 0;
    for &x in xs {
        if model.upper_bound(x) >= -100 {
            count += 1;
        }
    }
    count
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut rng = rand::thread_rng();

    let mut model = Model::new();

    let mut variables = Vec::new();
    let mut literals = Vec::new();

    for _ in 0..100 {
        let dom_start = rng.gen_range(-50..50);
        let dom_size = rng.gen_range(1..30);
        let var = model.new_ivar(dom_start, dom_start + dom_size, "");
        for _ in 0..5 {
            variables.push(var);
            literals.push(Bound::leq(var, rng.gen_range(-50..50)));
            literals.push(Bound::geq(var, rng.gen_range(-50..50)));
        }
    }

    literals.shuffle(&mut rng);
    variables.shuffle(&mut rng);

    c.bench_function("model-count-entailed-bounds", |b| {
        b.iter(|| count_entailed(black_box(&literals), black_box(&model)))
    });

    c.bench_function("model-count-lower-bounds", |b| {
        b.iter(|| count_lower_bounds(black_box(&variables), black_box(&model)))
    });
    c.bench_function("model-count-upper-bounds", |b| {
        b.iter(|| count_upper_bounds(black_box(&variables), black_box(&model)))
    });
}

criterion_group!(benches, criterion_benchmark);

criterion_main!(benches);
