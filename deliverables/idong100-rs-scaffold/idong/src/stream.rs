//! Reader thread + bounded channel + statistics.
//!
//! One owning reader thread per device (the xEyeClass analog): bulk reads
//! (4096 B chunks / 16 KiB accumulation), resync decode, keep-alive
//! answered inline (0xFFF -> 0xFFE within 1 s), MarkerFrame + Event into
//! a bounded crossbeam channel with drop-oldest backpressure; stats via
//! atomics (drops, resyncs, quality window of the last 200 frames).

/// Facade over a live (or mocked) device.
pub struct Idong { /* transport + reader handle + identity */ }

/// Snapshot of stream statistics (all atomics, lock-free read).
#[derive(Debug, Default)]
pub struct StreamStats {
    pub frames: u64,
    pub drops: u64,
    pub resyncs: u64,
    pub last_quality: u8, // 0 green / 1 yellow / 2 red
}

// TODO(P1.5): Idong::open_or_boot(fw) -> Self; frames()/events()/stats().
