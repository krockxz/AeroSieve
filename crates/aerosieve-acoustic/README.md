# aerosieve-acoustic

Deterministic acoustic quality sieve. Applies three sequential checks to 20ms audio frames:

1. **Silence** — RMS below configurable dB threshold
2. **SNR** — signal-to-noise ratio against a leading noise window
3. **Clipping** — fraction of samples near ±1.0

Pure math — no ML models, no heap allocation in the hot path.
