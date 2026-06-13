use criterion::{criterion_group, criterion_main, Criterion};

use aerosieve::*;
use aerosieve_ring::SourceKind;
use std::path::PathBuf;

fn bench_pipeline_end_to_end(c: &mut Criterion) {
    let config = aerosieve::PipelineConfig {
        ring_capacity: 64,
        sieve_config: aerosieve_acoustic::SieveConfig::default(),
        rules_path: PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../aerosieve-lexical/rules/default.yaml")),
        sink_config: aerosieve_sink::SinkConfig {
            staging_dir: std::env::temp_dir().join("aerosieve-bench-staging"),
            clean_dir: std::env::temp_dir().join("aerosieve-bench-clean"),
        },
    };

    c.bench_function("pipeline_end_to_end_320_samples", |b| {
        b.iter(|| {
            let mut pipeline = aerosieve::Pipeline::new(config.clone()).unwrap();
            let audio: Vec<f32> = (0..320).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
            pipeline.push_chunk(SourceKind::Synthetic, audio.clone(), "hello world".into()).unwrap();
            pipeline.process_all();
        });
    });
}

criterion_group!(benches, bench_pipeline_end_to_end);
criterion_main!(benches);
