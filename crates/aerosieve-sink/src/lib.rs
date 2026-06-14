use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SinkConfig {
    pub staging_dir: PathBuf,
    pub clean_dir: PathBuf,
}

#[derive(Debug)]
pub struct WriteResult {
    pub uuid: String,
    pub audio_path: PathBuf,
    pub text_path: PathBuf,
    pub committed: bool,
}

#[derive(Debug)]
pub struct Sink {
    config: SinkConfig,
}

impl Sink {
    pub fn new(config: SinkConfig) -> Result<Self, std::io::Error> {
        std::fs::create_dir_all(&config.staging_dir)?;
        std::fs::create_dir_all(&config.clean_dir)?;
        Ok(Self { config })
    }

    pub fn write_audio(&self, audio: &[u8], uuid: &str) -> Result<PathBuf, std::io::Error> {
        let path = self.config.staging_dir.join(format!("{uuid}.raw"));
        std::fs::write(&path, audio)?;
        Ok(path)
    }

    pub fn write_text(&self, text: &str, uuid: &str) -> Result<PathBuf, std::io::Error> {
        let path = self.config.staging_dir.join(format!("{uuid}.txt"));
        std::fs::write(&path, text)?;
        Ok(path)
    }

    pub fn commit(&self, uuid: &str) -> Result<WriteResult, std::io::Error> {
        let audio_staging = self.config.staging_dir.join(format!("{uuid}.raw"));
        let audio_clean = self.config.clean_dir.join(format!("{uuid}.raw"));
        let text_staging = self.config.staging_dir.join(format!("{uuid}.txt"));
        let text_clean = self.config.clean_dir.join(format!("{uuid}.txt"));

        match std::fs::hard_link(&audio_staging, &audio_clean) {
            Ok(()) => {
                match std::fs::hard_link(&text_staging, &text_clean) {
                    Ok(()) => {
                        let _ = std::fs::remove_file(&audio_staging);
                        let _ = std::fs::remove_file(&text_staging);
                        Ok(WriteResult {
                            uuid: uuid.to_string(),
                            audio_path: audio_clean,
                            text_path: text_clean,
                            committed: true,
                        })
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&audio_clean);
                        Err(e)
                    }
                }
            }
            Err(_) => {
                std::fs::rename(&audio_staging, &audio_clean)?;
                std::fs::rename(&text_staging, &text_clean)?;
                Ok(WriteResult {
                    uuid: uuid.to_string(),
                    audio_path: audio_clean,
                    text_path: text_clean,
                    committed: false,
                })
            }
        }
    }

    pub fn generate_uuid(&self) -> String {
        Uuid::new_v4().to_string()
    }

    pub fn cleanup_staging(&self) -> Result<(), std::io::Error> {
        for entry in std::fs::read_dir(&self.config.staging_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}


