use aries_backtrack::Backtrack;
use aries_model::bounds::Lit;
use aries_model::extensions::assignments::AssignmentExt;
use aries_model::lang::IVar;
use aries_model::state::domains::OptDomains;
use aries_model::{Model, WriterId};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::prelude::{SliceRandom, StdRng};
use rand::{Rng, SeedableRng};

fn count_entailed(xs: &[Lit], model: &Model) -> usize {
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

pub fn read_benchmark(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(2398248538438434234);

    let mut model = Model::new();

    let mut variables = Vec::new();
    let mut literals = Vec::new();

    for _ in 0..100 {
        let dom_start = rng.gen_range(-50..50);
        let dom_size = rng.gen_range(1..30);
        let var = model.new_ivar(dom_start, dom_start + dom_size, "");
        for _ in 0..5 {
            variables.push(var);
            literals.push(Lit::leq(var, rng.gen_range(-50..50)));
            literals.push(Lit::geq(var, rng.gen_range(-50..50)));
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

fn enforce_all(model: &mut OptDomains, lits: &[(Lit, u32)]) {
    let writer = WriterId::new(1u8);

    for &(lit, cause) in lits {
        model.set(lit, writer.cause(cause)).unwrap();
    }
}

fn backtrack_full(model: &mut OptDomains) {
    model.reset()
}

pub fn write_benchmark(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(2398248538438434234);

    let mut model = Model::new();

    let mut variables = Vec::new();
    let mut literals = Vec::new();

    for _ in 0..1000 {
        let var = model.new_ivar(0, 1000, "");
        variables.push(var);
    }

    for i in 10..400 {
        variables.shuffle(&mut rng);
        for v in &variables {
            let min_update = Lit::geq(*v, 10 + rng.gen_range((i - 2)..(i + 2)));
            let max_update = Lit::leq(*v, 990 - rng.gen_range((i - 2)..(i + 2)));
            let i = i as u32;
            if rng.gen_bool(0.5) {
                literals.push((min_update, i));
                literals.push((max_update, i));
            } else {
                literals.push((max_update, i));
                literals.push((min_update, i));
            }
        }
    }

    literals.shuffle(&mut rng);
    variables.shuffle(&mut rng);

    c.bench_function("model-writes-no-error", |b| {
        b.iter(|| {
            let dom = &mut model.state.domains.clone();
            enforce_all(black_box(dom), black_box(&literals))
        });
    });

    enforce_all(&mut model.state.domains, &literals);
    let base = model.state.domains;
    c.bench_function("model-backtrack", |b| {
        b.iter(|| backtrack_full(black_box(&mut base.clone())));
    });
}

criterion_group!(benches, read_benchmark, write_benchmark,);

criterion_main!(benches);
