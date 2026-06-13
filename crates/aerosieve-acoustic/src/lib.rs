/// Root-mean-square level in decibels below which audio is considered silence.
pub const DEFAULT_SILENCE_THRESHOLD_DB: f32 = -50.0;

/// Minimum signal-to-noise ratio (dB) required for audio to pass.
pub const DEFAULT_MIN_SNR_DB: f32 = 15.0;

/// Fraction of samples that must hit +/-1.0 before audio is rejected as clipped.
pub const DEFAULT_CLIP_RATIO: f32 = 0.001;

/// Duration of the leading noise window in milliseconds.
pub const DEFAULT_NOISE_WINDOW_MS: f32 = 100.0;

/// Sample rate in Hz used when computing `noise_window_samples` from `DEFAULT_NOISE_WINDOW_MS`.
pub const DEFAULT_SAMPLE_RATE: u32 = 16000;

/// Configuration for the acoustic sieve.
#[derive(Debug, Clone)]
pub struct SieveConfig {
    /// RMS level (dB) below which audio is rejected as silence.
    /// Defaults to -50.0 dB.
    pub silence_threshold_db: f32,

    /// Minimum SNR (dB) required to pass. Compared against signal RMS minus noise-floor RMS.
    /// Defaults to 15.0 dB.
    pub min_snr_db: f32,

    /// Fraction of samples at or above ±0.999 that triggers clipping rejection.
    /// Defaults to 0.001 (0.1%).
    pub clip_ratio_threshold: f32,

    /// Number of leading samples used to estimate the noise floor for SNR.
    /// Set to 0 to disable the SNR check entirely (any SNR will be accepted).
    /// Default derived from `DEFAULT_NOISE_WINDOW_MS` at `DEFAULT_SAMPLE_RATE` (1600 samples).
    pub noise_window_samples: usize,
}

impl Default for SieveConfig {
    fn default() -> Self {
        Self {
            silence_threshold_db: DEFAULT_SILENCE_THRESHOLD_DB,
            min_snr_db: DEFAULT_MIN_SNR_DB,
            clip_ratio_threshold: DEFAULT_CLIP_RATIO,
            noise_window_samples: (DEFAULT_NOISE_WINDOW_MS / 1000.0 * DEFAULT_SAMPLE_RATE as f32)
                as usize,
        }
    }
}

/// Result returned by [`AcousticSieve::analyze`].
#[derive(Debug, Clone)]
pub struct SieveResult {
    /// Whether the audio passed all checks.
    pub pass: bool,

    /// RMS level of the full buffer in decibels.
    pub rms_db: f32,

    /// Signal-to-noise ratio in dB (signal RMS minus noise-floor RMS).
    pub snr_db: f32,

    /// RMS level of the noise window in decibels.
    pub noise_floor_db: f32,

    /// Fraction of samples at or above ±0.999.
    pub clip_ratio: f32,

    /// The reason the audio was rejected, if any.
    pub reject_reason: Option<RejectReason>,
}

/// Why the acoustic sieve rejected a buffer.
#[derive(Debug, Clone, PartialEq)]
pub enum RejectReason {
    /// The overall RMS level fell below the silence threshold.
    Silence,
    /// The signal-to-noise ratio fell below the minimum.
    LowSnr,
    /// Too many samples were at or near full scale (±1.0).
    Clipping,
}

/// Acoustic quality sieve.
///
/// Applies three sequential checks:
/// 1. **Silence** — overall RMS below a configurable threshold.
/// 2. **SNR** — signal-to-noise ratio against a leading noise window.
/// 3. **Clipping** — fraction of samples near ±1.0.
pub struct AcousticSieve {
    config: SieveConfig,
}

impl AcousticSieve {
    /// Create a new sieve with the given configuration.
    pub fn new(config: SieveConfig) -> Self {
        Self { config }
    }

    /// Analyze a buffer of audio samples.
    ///
    /// # Noise window contract
    ///
    /// The first `config.noise_window_samples` samples are used as the noise-floor
    /// baseline for SNR calculation. **Callers must ensure the leading portion of
    /// the buffer contains only noise (or silence), not speech or other signal.**
    ///
    /// If `noise_window_samples` is 0, the SNR check is skipped and any SNR is
    /// accepted. This is useful when a noise reference is unavailable.
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

    /// Compute the root-mean-square level of a buffer in decibels (dB).
    ///
    /// Returns `f32::NEG_INFINITY` for an empty buffer.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silence_is_rejected() {
        let sieve = AcousticSieve::default();
        let silence = vec![0.0f32; 320];
        let result = sieve.analyze(&silence);
        assert!(!result.pass);
        assert_eq!(result.reject_reason, Some(RejectReason::Silence));
    }

    #[test]
    fn test_normal_speech_passes() {
        let mut config = SieveConfig::default();
        config.noise_window_samples = 32;
        let sieve = AcousticSieve::new(config);
        let mut signal = vec![0.001f32; 320];
        for i in 32..320 {
            signal[i] = ((i as f32 - 32.0) * 0.1).sin() * 0.5;
        }
        let result = sieve.analyze(&signal);
        assert!(result.pass);
        assert!(result.rms_db > -20.0);
        assert!(result.rms_db < 0.0);
    }

    #[test]
    fn test_clipping_is_rejected() {
        let mut config = SieveConfig::default();
        config.silence_threshold_db = -60.0;
        config.clip_ratio_threshold = 0.001;
        config.noise_window_samples = 0; // disable SNR check for this test
        let sieve = AcousticSieve::new(config);
        let mut clipped = vec![0.5f32; 320];
        clipped[0] = 1.0;
        clipped[1] = -1.0;
        clipped[2] = 0.999;
        let result = sieve.analyze(&clipped);
        assert!(!result.pass);
        assert_eq!(result.reject_reason, Some(RejectReason::Clipping));
    }

    #[test]
    fn test_empty_buffer() {
        let sieve = AcousticSieve::default();
        let result = sieve.analyze(&[]);
        assert!(!result.pass);
        assert_eq!(result.reject_reason, Some(RejectReason::Silence));
    }

    #[test]
    fn test_rms_db_helper() {
        let signal = vec![0.5f32; 100];
        let db = AcousticSieve::rms_db(&signal);
        assert!((db - 20.0 * 0.5f32.log10()).abs() < 0.01);
    }

    #[test]
    fn test_low_snr_is_rejected() {
        let mut config = SieveConfig::default();
        config.silence_threshold_db = -60.0;
        config.min_snr_db = 40.0;
        config.noise_window_samples = 20;
        let sieve = AcousticSieve::new(config);
        let mut signal = vec![0.0f32; 320];
        signal[..20].copy_from_slice(&vec![0.1; 20]);
        signal[20..].copy_from_slice(&vec![0.15; 300]);
        let result = sieve.analyze(&signal);
        assert!(!result.pass);
        assert!(result.snr_db < 40.0);
        assert!(!result.snr_db.is_infinite());
    }
}
