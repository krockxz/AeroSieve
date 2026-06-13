use criterion::{black_box, criterion_group, criterion_main, Criterion};
use aerosieve_acoustic::AcousticSieve;

fn bench_analyze_320_samples(c: &mut Criterion) {
    let sieve = AcousticSieve::default();
    let signal: Vec<f32> = (0..320).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();

    c.bench_function("acoustic_analyze_320", |b| {
        b.iter(|| {
            sieve.analyze(black_box(&signal));
        });
    });
}

criterion_group!(benches, bench_analyze_320_samples);
criterion_main!(benches);
