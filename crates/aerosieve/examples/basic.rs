use aerosieve::aerosieve_acoustic::SieveConfig;
use aerosieve::aerosieve_sink::SinkConfig;
use aerosieve::{Pipeline, PipelineConfig, SourceKind};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = PipelineConfig {
        ring_capacity: 4096,
        sieve_config: SieveConfig::default(),
        rules_path: PathBuf::from("crates/aerosieve-lexical/rules/default.yaml"),
        sink_config: SinkConfig::file("data/staging", "data/clean"),
    };

    let mut pipeline = Pipeline::new(config)?;

    // Push a 20ms, 16kHz mono frame with a Hinglish transcript.
    pipeline.push_chunk(
        SourceKind::Synthetic,
        vec![0.1; 320],
        "yeh ₹500 hai".to_string(),
    )?;

    if let Some(result) = pipeline.process_one() {
        println!("Passed: {}", result.passed);
        if let Some(ref norm) = result.normalized_text {
            println!("Normalized: {}", norm.normalized);
        }
    }

    Ok(())
}
