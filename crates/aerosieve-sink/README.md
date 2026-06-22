# aerosieve-sink

Zero-copy file sink with atomic commit semantics.

- Writes audio and text to a **staging directory**, then **hard-links** (or renames) into a clean directory
- `Null` mode for benchmarking (no disk I/O)
- UUID-based naming for collision-free storage
- Staging cleanup on demand
