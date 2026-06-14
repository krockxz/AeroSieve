use ringbuf::{HeapRb, HeapProd, HeapCons};
use ringbuf::traits::{Observer, Producer, Consumer, Split};
use std::mem::size_of;
use std::fmt;

pub const DEFAULT_CAPACITY: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SourceKind {
    File = 0,
    Microphone = 1,
    TcpSocket = 2,
    Synthetic = 3,
}

impl SourceKind {
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::File),
            1 => Some(Self::Microphone),
            2 => Some(Self::TcpSocket),
            3 => Some(Self::Synthetic),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotFlags(u32);

impl SlotFlags {
    pub const VALID: Self = Self(1 << 0);
    pub const REJECTED: Self = Self(1 << 1);
    pub const FINAL: Self = Self(1 << 2);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub fn set(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub timestamp_ns: u64,
    pub source_kind: SourceKind,
    pub sample_rate: u32,
    pub audio_samples: Vec<f32>,
    pub transcript: String,
    pub flags: SlotFlags,
}

impl AudioChunk {
    pub fn with_capacity(audio_samples: usize, text_bytes: usize) -> Self {
        Self {
            timestamp_ns: 0,
            source_kind: SourceKind::Synthetic,
            sample_rate: 16000,
            audio_samples: Vec::with_capacity(audio_samples),
            transcript: String::with_capacity(text_bytes),
            flags: SlotFlags::empty(),
        }
    }

    pub fn clear(&mut self) {
        self.timestamp_ns = 0;
        self.sample_rate = 16000;
        self.audio_samples.clear();
        self.transcript.clear();
        self.flags = SlotFlags::empty();
    }

    pub fn audio_as_bytes(&self) -> &[u8] {
        let len = self.audio_samples.len().checked_mul(size_of::<f32>())
            .expect("audio_samples length overflow");
        let ptr = self.audio_samples.as_ptr() as *const u8;
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }

    pub fn audio_as_f32_mut(&mut self) -> &mut [f32] {
        &mut self.audio_samples
    }
}

#[derive(Debug)]
pub struct RingError;

pub struct RingProducer {
    inner: HeapProd<AudioChunk>,
}

pub struct RingConsumer {
    inner: HeapCons<AudioChunk>,
}

impl fmt::Debug for RingProducer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RingProducer").finish()
    }
}

impl fmt::Debug for RingConsumer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RingConsumer").finish()
    }
}

pub fn create_ring(capacity: usize) -> (RingProducer, RingConsumer) {
    let rb = HeapRb::<AudioChunk>::new(capacity);
    let (prod, cons) = rb.split();
    (RingProducer { inner: prod }, RingConsumer { inner: cons })
}

impl RingProducer {
    pub fn push(&mut self, chunk: AudioChunk) -> Result<(), RingError> {
        self.inner.try_push(chunk).map_err(|_| RingError)
    }

    pub fn is_full(&self) -> bool {
        self.inner.is_full()
    }

    pub fn vacant_len(&self) -> usize {
        self.inner.vacant_len()
    }
}

impl RingConsumer {
    pub fn pop(&mut self) -> Option<AudioChunk> {
        self.inner.try_pop()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn occupied_len(&self) -> usize {
        self.inner.occupied_len()
    }
}

impl fmt::Display for RingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ring buffer error")
    }
}

impl std::error::Error for RingError {}


