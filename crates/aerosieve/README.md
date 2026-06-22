# aerosieve

Integration pipeline that orchestrates the four AeroSieve stages:

1. **Ring** — ingest frames into a lock-free SPSC buffer
2. **Acoustic** — reject silence, low-SNR, and clipped audio
3. **Lexical** — normalize Hinglish text through compiled rules
4. **Sink** — commit clean pairs to storage with atomic filesystem semantics

Re-exports all sub-crate public APIs under `aerosieve::*`.
