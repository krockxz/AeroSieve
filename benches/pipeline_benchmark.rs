use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, RefreshKind, System};

use aerosieve::*;
use aerosieve_acoustic::{AcousticSieve, SieveConfig};
use aerosieve_lexical::RuleEngine;
use aerosieve_ring::{create_ring, AudioChunk, SourceKind};
use aerosieve_sink::SinkConfig;

const FRAME_SAMPLES: usize = 320; // 20ms @ 16kHz mono
const WARMUP_FRAMES: usize = 5_000;
const LATENCY_FRAMES: usize = 100_000;
const THROUGHPUT_FRAMES: usize = 1_000_000;
const MEMORY_FRAMES: usize = 10_000_000;

fn make_speech() -> Vec<f32> {
    (0..FRAME_SAMPLES).map(|i| (i as f32 * 0.1).sin() * 0.5).collect()
}

fn rules_path() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../aerosieve-lexical/rules/default.yaml"
    ))
}

fn compute_config() -> PipelineConfig {
    PipelineConfig {
        ring_capacity: 65536,
        sieve_config: SieveConfig {
            noise_window_samples: 0,
            ..SieveConfig::default()
        },
        rules_path: rules_path(),
        sink_config: SinkConfig::null(),
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64) * p / 100.0).ceil() as usize;
    sorted[idx.saturating_sub(1)]
}

fn print_latency_table(title: &str, latencies_ns: &mut [f64]) {
    latencies_ns.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = percentile(latencies_ns, 50.0);
    let p90 = percentile(latencies_ns, 90.0);
    let p95 = percentile(latencies_ns, 95.0);
    let p99 = percentile(latencies_ns, 99.0);
    let p999 = percentile(latencies_ns, 99.9);
    let max = latencies_ns.last().copied().unwrap_or(0.0);

    println!("\n  {}", title);
    println!("  ──────────────────────────────────────────────────────────────");
    println!("  {:>6} {:>10} {:>10} {:>10} {:>10} {:>10}", "P50", "P90", "P95", "P99", "P99.9", "MAX");
    println!("  {:>6.0} {:>10.0} {:>10.0} {:>10.0} {:>10.0} {:>10.0}", p50, p90, p95, p99, p999, max);
    println!("  ──────────────────────────────────────────────────────────────");
    println!("  target P99 < 1000µs | measured P99 = {:.1}µs", p99 / 1000.0);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 1. Tail Latency Distribution
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
fn bench_tail_latency(c: &mut Criterion) {
    let config = compute_config();
    let mut pipeline = Pipeline::new(config).unwrap();
    let speech = make_speech();
    let text = "yeh \u{20B9}500 hai aur 5 laakh rupaye";

    // Warmup
    for _ in 0..WARMUP_FRAMES {
        pipeline
            .push_chunk(SourceKind::Synthetic, speech.clone(), text.into())
            .unwrap();
    }
    pipeline.process_all();

    // Measurement: push one frame, then time how long it takes to process it.
    // This is end-to-end per-frame latency through the whole pipeline.
    let mut latencies: Vec<f64> = Vec::with_capacity(LATENCY_FRAMES);
    for _ in 0..LATENCY_FRAMES {
        pipeline
            .push_chunk(SourceKind::Synthetic, speech.clone(), text.into())
            .unwrap();
        let t0 = Instant::now();
        pipeline.process_one();
        latencies.push(t0.elapsed().as_secs_f64() * 1e9);
    }

    print_latency_table("1. Tail Latency — full pipeline (null sink)", &mut latencies);

    c.bench_function("tail_latency_100k", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let mut p = Pipeline::new(compute_config()).unwrap();
                let audio = speech.clone();
                for _ in 0..1000 {
                    p.push_chunk(SourceKind::Synthetic, audio.clone(), text.into())
                        .unwrap();
                }
                let t = Instant::now();
                p.process_all();
                total += t.elapsed();
            }
            total
        });
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 2. Flatline Memory Footprint
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
fn current_rss_mb() -> f64 {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing().with_memory()),
    );
    let Ok(pid) = sysinfo::get_current_pid() else {
        return 0.0;
    };
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_memory(),
    );
    sys.process(pid)
        .map(|p| p.memory() as f64 / (1024.0 * 1024.0))
        .unwrap_or(0.0)
}

fn bench_memory_flatline(c: &mut Criterion) {
    let config = compute_config();
    let mut pipeline = Pipeline::new(config).unwrap();
    let speech = make_speech();
    let text = "hello world";

    // Warmup
    for _ in 0..WARMUP_FRAMES {
        pipeline
            .push_chunk(SourceKind::Synthetic, speech.clone(), text.into())
            .unwrap();
    }
    pipeline.process_all();

    let samples = 20;
    let mut rss_samples: Vec<(usize, f64)> = Vec::with_capacity(samples);
    let step = MEMORY_FRAMES / samples;

    for i in 0..MEMORY_FRAMES {
        pipeline
            .push_chunk(SourceKind::Synthetic, speech.clone(), text.into())
            .unwrap();
        pipeline.process_one();

        if i % step == 0 {
            rss_samples.push((i, current_rss_mb()));
        }
    }

    println!("\n  2. Memory Footprint — {} frames (null sink)", MEMORY_FRAMES);
    println!("  ──────────────────────────────────────────────────────────────");
    println!("  {:>10} {:>12}", "FRAME", "RSS (MB)");
    for (frame, rss) in rss_samples {
        println!("  {:>10} {:>12.1}", frame, rss);
    }
    println!("  ──────────────────────────────────────────────────────────────");

    c.bench_function("memory_flatline_10m", |b| {
        b.iter(|| {
            let mut p = Pipeline::new(compute_config()).unwrap();
            for _ in 0..100_000 {
                p.push_chunk(SourceKind::Synthetic, speech.clone(), text.into())
                    .unwrap();
                p.process_one();
            }
        });
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 3. Throughput Saturation (1 / 4 / 8 cores)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
fn bench_throughput_saturation(c: &mut Criterion) {
    let speech = make_speech();
    let text = "hello world";
    let per_worker = THROUGHPUT_FRAMES / 8;

    println!("\n  3. Throughput Saturation ({} frames total)", THROUGHPUT_FRAMES);
    println!("  ──────────────────────────────────────────────────────────────");

    let mut g = c.benchmark_group("throughput");

    for workers in [1, 2, 4, 8] {
        let total_frames = per_worker * workers;
        g.throughput(Throughput::Elements(total_frames as u64));

        let id = format!("{workers}_workers");
        g.bench_function(&id, |b| {
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    let mut handles = Vec::with_capacity(workers);
                    for _ in 0..workers {
                        let speech = speech.clone();
                        let text = text.to_string();
                        handles.push(thread::spawn(move || {
                            run_pipeline_batch_with_data(per_worker, speech, text)
                        }));
                    }
                    let t0 = Instant::now();
                    for h in handles {
                        let _ = h.join();
                    }
                    total += t0.elapsed();
                }
                total
            });
        });

        let handles: Vec<_> = (0..workers)
            .map(|_| {
                let speech = speech.clone();
                let text = text.to_string();
                thread::spawn(move || run_pipeline_batch_with_data(per_worker, speech, text))
            })
            .collect();
        let t0 = Instant::now();
        for h in handles {
            let _ = h.join();
        }
        let elapsed = t0.elapsed();
        let fps = total_frames as f64 / elapsed.as_secs_f64();
        println!(
            "  {:>2} worker(s) | {:>10.0} fps | {:.2?} total",
            workers, fps, elapsed
        );
    }
    g.finish();
    println!("  ──────────────────────────────────────────────────────────────");
}

fn run_pipeline_batch_with_data(frames: usize, speech: Vec<f32>, text: String) -> Duration {
    let mut p = Pipeline::new(compute_config()).unwrap();
    let batch = 10_000;
    let t0 = Instant::now();
    let mut pushed = 0;
    while pushed < frames {
        let end = (pushed + batch).min(frames);
        for _ in pushed..end {
            p.push_chunk(SourceKind::Synthetic, speech.clone(), text.clone())
                .unwrap();
        }
        while p.process_one().is_some() {}
        pushed = end;
    }
    t0.elapsed()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 4. Component microbenchmarks (published alongside the headline numbers)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
fn bench_component_micros(c: &mut Criterion) {
    let mut g = c.benchmark_group("ring");
    g.throughput(Throughput::Elements(1));
    g.bench_function("push_pop", |b| {
        let (mut prod, mut cons) = create_ring(4096);
        let mut chunk = AudioChunk::with_capacity(FRAME_SAMPLES, 256);
        chunk.audio_samples = make_speech();
        prod.push(chunk).unwrap();
        b.iter(|| {
            let popped = cons.pop().unwrap();
            prod.push(popped).unwrap();
        });
    });
    g.finish();

    let sieve = AcousticSieve::default();
    let speech = make_speech();
    let mut g = c.benchmark_group("acoustic");
    g.throughput(Throughput::Elements(FRAME_SAMPLES as u64));
    g.bench_function("analyze_speech", |b| {
        b.iter(|| sieve.analyze(black_box(&speech)));
    });
    g.finish();

    let engine = RuleEngine::from_yaml_file(&rules_path())
        .unwrap_or_else(|_| RuleEngine::empty());
    let text = "yeh \u{20B9}500 hai aur 5 laakh rupaye";
    let mut g = c.benchmark_group("lexical");
    g.bench_function("normalize", |b| {
        b.iter(|| engine.normalize(black_box(text)));
    });
    g.finish();

    let mut g = c.benchmark_group("file_sink");
    let base = std::env::temp_dir().join("aerosieve-bench-file");
    let file_config = PipelineConfig {
        ring_capacity: 1024,
        sieve_config: SieveConfig {
            noise_window_samples: 0,
            ..SieveConfig::default()
        },
        rules_path: rules_path(),
        sink_config: SinkConfig::file(base.join("staging"), base.join("clean")),
    };
    g.bench_function("single_frame", |b| {
        b.iter(|| {
            let mut p = Pipeline::new(file_config.clone()).unwrap();
            p.push_chunk(SourceKind::Synthetic, speech.clone(), "hello".into())
                .unwrap();
            p.process_all();
        });
    });
    g.finish();
    let _ = std::fs::remove_dir_all(&base);
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(30);
    targets =
        bench_tail_latency,
        bench_memory_flatline,
        bench_throughput_saturation,
        bench_component_micros,
);
criterion_main!(benches);
