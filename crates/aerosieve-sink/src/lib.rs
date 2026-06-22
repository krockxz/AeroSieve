use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SinkMode {
    #[default]
    File,
    Null,
}

#[derive(Debug, Clone)]
pub struct SinkConfig {
    pub staging_dir: PathBuf,
    pub clean_dir: PathBuf,
    pub mode: SinkMode,
}

impl SinkConfig {
    pub fn file(staging_dir: impl Into<PathBuf>, clean_dir: impl Into<PathBuf>) -> Self {
        Self {
            staging_dir: staging_dir.into(),
            clean_dir: clean_dir.into(),
            mode: SinkMode::File,
        }
    }

    pub fn null() -> Self {
        Self {
            staging_dir: PathBuf::new(),
            clean_dir: PathBuf::new(),
            mode: SinkMode::Null,
        }
    }
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
        if config.mode == SinkMode::File {
            std::fs::create_dir_all(&config.staging_dir)?;
            std::fs::create_dir_all(&config.clean_dir)?;
        }
        Ok(Self { config })
    }

    pub fn write_audio(&self, _audio: &[u8], uuid: &str) -> Result<PathBuf, std::io::Error> {
        if self.config.mode == SinkMode::Null {
            return Ok(PathBuf::from(format!("{uuid}.raw")));
        }
        let path = self.config.staging_dir.join(format!("{uuid}.raw"));
        std::fs::write(&path, _audio)?;
        Ok(path)
    }

    pub fn write_text(&self, _text: &str, uuid: &str) -> Result<PathBuf, std::io::Error> {
        if self.config.mode == SinkMode::Null {
            return Ok(PathBuf::from(format!("{uuid}.txt")));
        }
        let path = self.config.staging_dir.join(format!("{uuid}.txt"));
        std::fs::write(&path, _text)?;
        Ok(path)
    }

    pub fn commit(&self, uuid: &str) -> Result<WriteResult, std::io::Error> {
        if self.config.mode == SinkMode::Null {
            return Ok(WriteResult {
                uuid: uuid.to_string(),
                audio_path: PathBuf::from(format!("{uuid}.raw")),
                text_path: PathBuf::from(format!("{uuid}.txt")),
                committed: true,
            });
        }

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
        if self.config.mode == SinkMode::Null {
            return Ok(());
        }
        for entry in std::fs::read_dir(&self.config.staging_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("aerosieve-sink-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_write_and_commit() {
        let base = temp_dir();
        let staging = base.join("staging");
        let clean = base.join("clean");
        let sink = Sink::new(SinkConfig {
            staging_dir: staging.clone(),
            clean_dir: clean.clone(),
            mode: SinkMode::File,
        })
        .unwrap();
        let uuid = sink.generate_uuid();

        sink.write_audio(&[0x01, 0x02, 0x03], &uuid).unwrap();
        sink.write_text("hello world", &uuid).unwrap();
        let result = sink.commit(&uuid).unwrap();

        assert!(result.committed);
        assert!(result.audio_path.exists());
        assert!(result.text_path.exists());
        assert!(!staging.join(format!("{uuid}.raw")).exists());
        assert!(!staging.join(format!("{uuid}.txt")).exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_cleanup_staging() {
        let base = temp_dir();
        let staging = base.join("staging");
        let clean = base.join("clean");
        let sink = Sink::new(SinkConfig {
            staging_dir: staging.clone(),
            clean_dir: clean.clone(),
            mode: SinkMode::File,
        })
        .unwrap();

        let uuid = sink.generate_uuid();
        sink.write_audio(&[1, 2, 3], &uuid).unwrap();
        assert!(staging.join(format!("{uuid}.raw")).exists());

        sink.cleanup_staging().unwrap();
        assert!(!staging.join(format!("{uuid}.raw")).exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_write_audio_and_text_contents() {
        let base = temp_dir();
        let staging = base.join("staging");
        let clean = base.join("clean");
        let sink = Sink::new(SinkConfig {
            staging_dir: staging.clone(),
            clean_dir: clean.clone(),
            mode: SinkMode::File,
        })
        .unwrap();

        let uuid = sink.generate_uuid();
        let audio_data = vec![0xAAu8; 1024];
        sink.write_audio(&audio_data, &uuid).unwrap();
        sink.write_text("\u{0928}\u{092E}\u{0938}\u{094D}\u{0924}\u{0947}, hello", &uuid).unwrap();

        let staging_audio = fs::read(staging.join(format!("{uuid}.raw"))).unwrap();
        assert_eq!(staging_audio, audio_data);

        let staging_text =
            fs::read_to_string(staging.join(format!("{uuid}.txt"))).unwrap();
        assert_eq!(staging_text, "\u{0928}\u{092E}\u{0938}\u{094D}\u{0924}\u{0947}, hello");

        let _ = fs::remove_dir_all(&base);
    }
}


