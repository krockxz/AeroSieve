# aerosieve-ring

Lock-free SPSC ring buffer for transferring `AudioChunk` frames between producer and consumer without copies.

Backed by [`ringbuf`](https://crates.io/crates/ringbuf) (HeapRb). Each slot carries audio samples, transcript text, source metadata, and bitflag state.
