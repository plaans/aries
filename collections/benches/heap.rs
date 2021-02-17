use aries_collections::heap::IdxHeap;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const N: usize = 10000;

fn insert_remove(heap: &mut IdxHeap<usize, f64>, n: usize) {
    for i in 0..n {
        heap.enqueue(i);
    }
    for _ in 0..n {
        heap.pop().unwrap();
    }
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(79837224973);
    let mut heap = IdxHeap::new();
    for i in 0..N {
        heap.declare_element(i, rng.gen_range(-100..100) as f64);
    }

    for &n in &[20, 100, 1000, 10000] {
        let name = format!("heap-insert-remove-{}", n);
        c.bench_function(&name, |b| b.iter(|| insert_remove(&mut heap, black_box(n as usize))));
    }
}

criterion_group!(benches, criterion_benchmark);

criterion_main!(benches);
