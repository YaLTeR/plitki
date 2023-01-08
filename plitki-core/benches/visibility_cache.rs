use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use plitki_core::visibility_cache::VisibilityCache;

pub fn criterion_benchmark(c: &mut Criterion) {
    // Simulate a lane with 5000 non-overlapping objects.
    let objects = (0..5000).map(|x| (x, x));

    c.bench_function("create cache", |b| {
        b.iter(|| {
            let cache = VisibilityCache::new(black_box(&objects).clone());
            black_box(cache);
        })
    });

    let cache = VisibilityCache::new(objects);

    c.bench_function("compute visibility", |b| {
        b.iter(|| {
            let values = black_box(&cache).visible_objects(1000..2000);
            let _ = black_box(values);
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
