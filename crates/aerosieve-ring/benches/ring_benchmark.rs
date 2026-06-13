use criterion::{black_box, Criterion};
use aerosieve_ring::{create_ring, AudioChunk};

pub fn bench_push_pop(c: &mut Criterion) {
    c.bench_function("push_pop_4096", |b| {
        let (mut prod, mut cons) = create_ring(4096);
        let chunk = AudioChunk::with_capacity(1024, 256);
        b.iter(|| {
            prod.push(chunk.clone()).unwrap();
            black_box(cons.pop().unwrap());
        });
    });
}

criterion::criterion_group!(benches, bench_push_pop);
criterion::criterion_main!(benches);
