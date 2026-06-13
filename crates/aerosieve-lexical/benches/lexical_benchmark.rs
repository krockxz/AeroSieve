use criterion::{black_box, criterion_group, criterion_main, Criterion};
use aerosieve_lexical::RuleEngine;
use std::path::PathBuf;

fn bench_normalize(c: &mut Criterion) {
    let rules_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules/default.yaml");
    let engine = RuleEngine::from_yaml_file(&rules_path).unwrap_or_else(|_| RuleEngine::empty());

    let text = "yeh ₹500 hai aur 5 laakh rupaye ka hisaab hai";

    c.bench_function("lexical_normalize", |b| {
        b.iter(|| {
            engine.normalize(black_box(text));
        });
    });
}

criterion_group!(benches, bench_normalize);
criterion_main!(benches);
