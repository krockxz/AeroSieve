pub const DEFAULT_SILENCE_THRESHOLD_DB: f32 = -50.0;
pub const DEFAULT_MIN_SNR_DB: f32 = 15.0;
pub const DEFAULT_CLIP_RATIO: f32 = 0.001;

#[derive(Debug, Clone)]
pub struct SieveConfig {
    pub silence_threshold_db: f32,
    pub min_snr_db: f32,
    pub clip_ratio_threshold: f32,
    pub noise_window_samples: usize,
}

impl Default for SieveConfig {
    fn default() -> Self {
        Self {
            silence_threshold_db: DEFAULT_SILENCE_THRESHOLD_DB,
            min_snr_db: DEFAULT_MIN_SNR_DB,
            clip_ratio_threshold: DEFAULT_CLIP_RATIO,
            noise_window_samples: 1600,
        }
    }
}

/// Result returned by `AcousticSieve::analyze`.
#[derive(Debug, Clone)]
pub struct SieveResult {
    pub pass: bool,
    pub rms_db: f32,
    pub snr_db: f32,
    pub noise_floor_db: f32,
    pub clip_ratio: f32,
    pub reject_reason: Option<RejectReason>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RejectReason {
    Silence,
    LowSnr,
    Clipping,
}

pub struct AcousticSieve {
    config: SieveConfig,
}

impl AcousticSieve {
    pub fn new(config: SieveConfig) -> Self {
        Self { config }
    }

    pub fn analyze(&self, samples: &[f32]) -> SieveResult {
        if samples.is_empty() {
            return SieveResult {
                pass: false,
                rms_db: f32::NEG_INFINITY,
                snr_db: f32::NEG_INFINITY,
                noise_floor_db: f32::NEG_INFINITY,
                clip_ratio: 0.0,
                reject_reason: Some(RejectReason::Silence),
            };
        }

        let squared_sum: f32 = samples.iter().map(|x| x * x).sum();
        let rms = (squared_sum / samples.len() as f32).sqrt();
        let rms_db = Self::amplitude_to_db(rms);

        let noise_window = &samples[..self.config.noise_window_samples.min(samples.len())];
        let noise_squared_sum: f32 = noise_window.iter().map(|x| x * x).sum();
        let noise_rms = if noise_window.is_empty() {
            0.0
        } else {
            (noise_squared_sum / noise_window.len() as f32).sqrt()
        };
        let noise_floor_db = Self::amplitude_to_db(noise_rms);

        let snr_db = if noise_rms > 0.0 {
            rms_db - noise_floor_db
        } else {
            f32::NEG_INFINITY
        };

        let clip_count = samples
            .iter()
            .filter(|&&x| x.abs() >= 0.999)
            .count();
        let clip_ratio = clip_count as f32 / samples.len() as f32;

        let check_snr = self.config.noise_window_samples > 0;
        let reject_reason = if rms_db < self.config.silence_threshold_db {
            Some(RejectReason::Silence)
        } else if check_snr && snr_db < self.config.min_snr_db {
            Some(RejectReason::LowSnr)
        } else if clip_ratio > self.config.clip_ratio_threshold {
            Some(RejectReason::Clipping)
        } else {
            None
        };

        SieveResult {
            pass: reject_reason.is_none(),
            rms_db,
            snr_db,
            noise_floor_db,
            clip_ratio,
            reject_reason,
        }
    }

    pub fn rms_db(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return f32::NEG_INFINITY;
        }
        let squared_sum: f32 = samples.iter().map(|x| x * x).sum();
        let rms = (squared_sum / samples.len() as f32).sqrt();
        Self::amplitude_to_db(rms)
    }

    fn amplitude_to_db(amplitude: f32) -> f32 {
        if amplitude <= 0.0 {
            f32::NEG_INFINITY
        } else {
            20.0 * amplitude.log10()
        }
    }
}

impl Default for AcousticSieve {
    fn default() -> Self {
        Self::new(SieveConfig::default())
    }
}


