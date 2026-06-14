use aerosieve_acoustic::{AcousticSieve, SieveConfig, SieveResult};
use aerosieve_lexical::{NormalizedText, RuleEngine};
use aerosieve_ring::{create_ring, AudioChunk, RingConsumer, RingProducer, SlotFlags, SourceKind};
use aerosieve_sink::{Sink, SinkConfig};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub use aerosieve_acoustic;
pub use aerosieve_lexical;
pub use aerosieve_ring;
pub use aerosieve_sink;

#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub ring_capacity: usize,
    pub sieve_config: SieveConfig,
    pub rules_path: PathBuf,
    pub sink_config: SinkConfig,
}

#[derive(Debug, Default)]
pub struct PipelineStats {
    pub frames_processed: u64,
    pub frames_passed: u64,
    pub frames_rejected: u64,
    pub norm_rules_applied: u64,
    pub commits_succeeded: u64,
    pub commits_failed: u64,
}

pub struct Pipeline {
    producer: RingProducer,
    consumer: RingConsumer,
    sieve: AcousticSieve,
    engine: RuleEngine,
    sink: Sink,
    stats: PipelineStats,
}

#[derive(Debug)]
pub struct PipelineResult {
    pub chunk_id: u64,
    pub passed: bool,
    pub sieve_result: SieveResult,
    pub normalized_text: Option<NormalizedText>,
    pub write_result: Option<aerosieve_sink::WriteResult>,
}

impl Pipeline {
    pub fn new(config: PipelineConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let (producer, consumer) = create_ring(config.ring_capacity);
        let sieve = AcousticSieve::new(config.sieve_config);
        let engine = RuleEngine::from_yaml_file(&config.rules_path)
            .unwrap_or_else(|_| {
                eprintln!("Warning: could not load rules from {:?}, using empty engine", config.rules_path);
                RuleEngine::empty()
            });
        let sink = Sink::new(config.sink_config)?;

        Ok(Self {
            producer,
            consumer,
            sieve,
            engine,
            sink,
            stats: PipelineStats::default(),
        })
    }

    pub fn producer_mut(&mut self) -> &mut RingProducer {
        &mut self.producer
    }

    pub fn push_chunk(
        &mut self,
        source_kind: SourceKind,
        audio: Vec<f32>,
        transcript: String,
    ) -> Result<(), aerosieve_ring::RingError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let chunk = AudioChunk {
            timestamp_ns: now,
            source_kind,
            sample_rate: 16000,
            audio_samples: audio,
            transcript,
            flags: SlotFlags::VALID,
        };

        self.producer.push(chunk)
    }

    pub fn process_one(&mut self) -> Option<PipelineResult> {
        let chunk = self.consumer.pop()?;
        self.stats.frames_processed += 1;

        let sieve_result = self.sieve.analyze(&chunk.audio_samples);

        if !sieve_result.pass {
            self.stats.frames_rejected += 1;
            return Some(PipelineResult {
                chunk_id: self.stats.frames_processed,
                passed: false,
                sieve_result,
                normalized_text: None,
                write_result: None,
            });
        }

        self.stats.frames_passed += 1;

        let normalized = self.engine.normalize(&chunk.transcript);
        self.stats.norm_rules_applied += normalized.rules_applied.len() as u64;

        let uuid = self.sink.generate_uuid();
        let audio_bytes = chunk.audio_as_bytes();

        let write_result = (|| -> Result<aerosieve_sink::WriteResult, std::io::Error> {
            self.sink.write_audio(audio_bytes, &uuid)?;
            self.sink.write_text(&normalized.normalized, &uuid)?;
            self.sink.commit(&uuid)
        })();

        match write_result {
            Ok(wr) => {
                self.stats.commits_succeeded += 1;
                Some(PipelineResult {
                    chunk_id: self.stats.frames_processed,
                    passed: true,
                    sieve_result,
                    normalized_text: Some(normalized),
                    write_result: Some(wr),
                })
            }
            Err(e) => {
                self.stats.commits_failed += 1;
                eprintln!("Sink error: {e}");
                Some(PipelineResult {
                    chunk_id: self.stats.frames_processed,
                    passed: true,
                    sieve_result,
                    normalized_text: Some(normalized),
                    write_result: None,
                })
            }
        }
    }

    pub fn process_all(&mut self) -> Vec<PipelineResult> {
        let mut results = Vec::new();
        while let Some(result) = self.process_one() {
            results.push(result);
        }
        results
    }

    pub fn stats(&self) -> &PipelineStats {
        &self.stats
    }
}


