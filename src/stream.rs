//! Reader thread + bounded channel + statistics.
//!
//! One owning reader thread per device (the xEyeClass analog): bulk reads
//! (4096 B chunks), resync decode, keep-alive answered inline
//! (0xFFF -> 0xFFE within 1 s), marker frames into a **bounded**
//! crossbeam channel with drop-oldest backpressure - exactly the vendor
//! `PipeBufferSize` window behavior: if the consumer is slower than the
//! device (50 fps), the oldest frame is evicted and the overflow
//! counter increments. Stats live behind atomics, lock-free to read.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver as StopRx, Sender as StopTx, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};

use crate::error::{IdongError, ProtocolError, Result};
use crate::marker::MarkerFrame;
use crate::protocol::cmd::RuntimeClient;
use crate::protocol::frames::FrameDecoder;
use crate::protocol::{self, records};
use crate::transport::Transport;

/// Stream statistics (all monotonic atomics, lock-free read).
#[derive(Debug, Default)]
pub struct StreamStats {
    /// Bulk chunks consumed.
    pub chunks: AtomicU64,
    /// Complete frames decoded.
    pub frames: AtomicU64,
    /// Marker frames produced (before backpressure).
    pub marker_frames: AtomicU64,
    /// Frames evicted by drop-oldest backpressure (slow consumer).
    pub overflow: AtomicU64,
    /// Frames whose counter jumped (vendor drop check).
    pub drops: AtomicU64,
    /// Keep-alive polls answered.
    pub keepalives: AtomicU64,
    /// Link-status frames handled.
    pub link_status: AtomicU64,
    /// Bytes skipped while hunting for a magic (resync accounting).
    pub resyncs: AtomicU64,
}

impl StreamStats {
    fn bump(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Snapshot all counters (for diagnostics topics).
    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            chunks: self.chunks.load(Ordering::Relaxed),
            frames: self.frames.load(Ordering::Relaxed),
            marker_frames: self.marker_frames.load(Ordering::Relaxed),
            overflow: self.overflow.load(Ordering::Relaxed),
            drops: self.drops.load(Ordering::Relaxed),
            keepalives: self.keepalives.load(Ordering::Relaxed),
            link_status: self.link_status.load(Ordering::Relaxed),
            resyncs: self.resyncs.load(Ordering::Relaxed),
        }
    }
}

/// Plain-copy snapshot of [`StreamStats`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatsSnapshot {
    pub chunks: u64,
    pub frames: u64,
    pub marker_frames: u64,
    pub overflow: u64,
    pub drops: u64,
    pub keepalives: u64,
    pub link_status: u64,
    pub resyncs: u64,
}

/// Handle to the running reader thread: receive frames, read stats,
/// stop the thread by dropping (or [`Streamer::stop`]).
pub struct Streamer {
    stats: Arc<StreamStats>,
    rx: Receiver<MarkerFrame>,
    stop: StopTx<()>,
}

impl Streamer {
    /// Start the reader thread on an owned transport with a bounded
    /// channel of `depth` frames (the QoS depth / buffer_depth analog).
    pub fn start(transport: Box<dyn Transport + Send>, depth: usize) -> Self {
        let (tx, rx) = bounded::<MarkerFrame>(depth);
        // The reader thread keeps a receiver clone to evict the OLDEST
        // queued frame on overflow (drop-oldest, not drop-newest).
        let evict = rx.clone();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let stats = Arc::new(StreamStats::default());
        let thread_stats = Arc::clone(&stats);
        std::thread::Builder::new()
            .name("idong-stream".into())
            .spawn(move || reader_loop(transport, tx, evict, thread_stats, stop_rx))
            .expect("spawn idong-stream thread");
        Streamer { stats, rx, stop: stop_tx }
    }

    /// Blocking read of the next marker frame with timeout.
    pub fn read_frame(&self, timeout: Duration) -> Result<MarkerFrame> {
        match self.rx.recv_timeout(timeout) {
            Ok(f) => Ok(f),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                Err(IdongError::Protocol(ProtocolError::ReadTimeout(timeout)))
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => Err(IdongError::NotFound),
        }
    }

    /// Snapshot the stream statistics.
    pub fn stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }

    /// Ask the reader thread to stop (also happens by dropping self).
    pub fn stop(&self) {
        let _ = self.stop.send(());
    }
}

impl Drop for Streamer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Bounded send with drop-oldest backpressure (vendor PipeBufferSize
/// semantics): when the channel is full the oldest queued frame is
/// evicted and the overflow counter increments. Returns false only when
/// no receiver is left (both the consumer and the eviction clone gone).
fn bounded_send(
    tx: &Sender<MarkerFrame>,
    evict: &Receiver<MarkerFrame>,
    frame: MarkerFrame,
    stats: &StreamStats,
) -> bool {
    let mut frame = frame;
    loop {
        match tx.try_send(frame.clone()) {
            Ok(()) => return true,
            Err(TrySendError::Full(f)) => {
                frame = f;
                // evict the oldest queued frame to make room
                match evict.try_recv() {
                    Ok(_evicted) => StreamStats::bump(&stats.overflow),
                    Err(_) => return false, // drained concurrently or gone
                }
            }
            Err(TrySendError::Disconnected(f)) => {
                frame = f;
                // consumer side gone: try one eviction; if the queue is
                // unreachable, stop the thread
                match evict.try_recv() {
                    Ok(_evicted) => StreamStats::bump(&stats.overflow),
                    Err(_) => return false,
                }
            }
        }
    }
}

fn reader_loop(
    mut transport: Box<dyn Transport + Send>,
    tx: Sender<MarkerFrame>,
    evict: Receiver<MarkerFrame>,
    stats: Arc<StreamStats>,
    stop: StopRx<()>,
) {
    let mut decoder = FrameDecoder::new();
    let mut prev_counter: Option<u32> = None;
    loop {
        match stop.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => return,
            Err(TryRecvError::Empty) => {}
        }
        let chunk = match transport.read(protocol::RUNTIME_CHUNK) {
            Ok(c) => c,
            Err(e) => {
                log::debug!("stream read error: {e}; retrying");
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        if chunk.is_empty() {
            // No data within the read budget; check stop again.
            std::thread::sleep(Duration::from_millis(1));
            continue;
        }
        StreamStats::bump(&stats.chunks);
        for frame in decoder.push(&chunk) {
            StreamStats::bump(&stats.frames);
            // Control plane: keep-alive answered inline (1 s budget).
            if RuntimeClient::is_keepalive(&frame) {
                let mut client = RuntimeClient::new(&mut *transport);
                if let Err(e) = client.answer_keepalive() {
                    log::warn!("keep-alive answer failed: {e}");
                } else {
                    StreamStats::bump(&stats.keepalives);
                }
                continue;
            }
            if frame.kind == protocol::TYPE_LINK_STATUS {
                let mut client = RuntimeClient::new(&mut *transport);
                match client.handle_link_status(&frame) {
                    Ok(_) => StreamStats::bump(&stats.link_status),
                    Err(e) => log::warn!("link-status handling failed: {e}"),
                }
                continue;
            }
            // Data plane: marker record.
            if let Some(mf) = records::parse_record(&frame.payload, prev_counter) {
                if mf.dropped {
                    StreamStats::bump(&stats.drops);
                }
                prev_counter = Some(mf.frame_counter);
                StreamStats::bump(&stats.marker_frames);
                if !bounded_send(&tx, &evict, mf, &stats) {
                    return; // no receiver anywhere - stop the thread
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{reply, MockTransport};
    use crate::protocol::frames::Frame;

    fn marker_record(counter: u32) -> Vec<u8> {
        let mut v = vec![0u8; records::RECORD_MIN];
        v[0..4].copy_from_slice(&counter.to_le_bytes());
        v[records::MODE_OFFSET..records::MODE_OFFSET + 4].copy_from_slice(&0u32.to_le_bytes());
        v[0x24..0x28].copy_from_slice(&80i32.to_le_bytes()); // slot 0 quality
        v
    }

    /// Synthetic 50 fps stream over the mock: keep-alive answered inline,
    /// three marker records decoded, garbage resynced past.
    #[test]
    fn mock_replay_drives_the_stream() {
        let mut reads = vec![reply(Frame::d2h(protocol::KEEPALIVE_RX, &[]).to_bytes())];
        for c in 1..=3u32 {
            reads.push(reply(Frame::d2h(0, &marker_record(c)).to_bytes()));
        }
        reads.push(reply(vec![0xDE, 0xAD, 0xBE, 0xEF])); // garbage -> resync
                                                         // A link-status frame (subcode 1) is answered with word 0x11.
        let mut ls = Frame::d2h(protocol::TYPE_LINK_STATUS, &[0u8; 4]);
        ls.payload[2..4].copy_from_slice(&1u16.to_le_bytes());
        reads.push(reply(ls.to_bytes()));

        let s = Streamer::start(Box::new(MockTransport::new(reads)), 8);
        let mut got = Vec::new();
        for _ in 0..3 {
            let f = s.read_frame(Duration::from_secs(2)).expect("frame");
            got.push(f.frame_counter);
        }
        assert_eq!(got, vec![1, 2, 3]);
        // let the reader consume the remaining scripted reads (garbage
        // + link-status) before asserting control-plane counters
        std::thread::sleep(Duration::from_millis(150));
        let st = s.stats();
        assert_eq!(st.keepalives, 1, "one keep-alive answered inline");
        assert_eq!(st.link_status, 1, "one link-status handled");
        assert_eq!(st.marker_frames, 3);
        assert_eq!(st.drops, 0, "counters continuous 1-2-3");
        s.stop();
    }

    /// Backpressure: consumer absent while six frames arrive on a
    /// depth-2 channel -> four evictions, last two retained
    /// (drop-oldest, exact accounting).
    #[test]
    fn drop_oldest_accounting() {
        let mut reads = Vec::new();
        for c in 0..6u32 {
            reads.push(reply(Frame::d2h(0, &marker_record(c)).to_bytes()));
        }
        let s = Streamer::start(Box::new(MockTransport::new(reads)), 2);
        // let the reader thread produce all six frames (mock is instant)
        std::thread::sleep(Duration::from_millis(200));
        let f1 = s.read_frame(Duration::from_secs(1)).unwrap();
        let f2 = s.read_frame(Duration::from_secs(1)).unwrap();
        // the oldest two were evicted: the retained pair is 4, 5
        assert_eq!((f1.frame_counter, f2.frame_counter), (4, 5));
        let st = s.stats();
        assert_eq!(st.marker_frames, 6, "all six produced");
        assert_eq!(st.overflow, 4, "four evicted (drop-oldest)");
        s.stop();
    }

    /// Counter jump marks the dropped frames (vendor continuity check).
    #[test]
    fn counter_jump_counts_as_drop() {
        let reads = vec![
            reply(Frame::d2h(0, &marker_record(10)).to_bytes()),
            reply(Frame::d2h(0, &marker_record(14)).to_bytes()), // jump
        ];
        let s = Streamer::start(Box::new(MockTransport::new(reads)), 8);
        let _ = s.read_frame(Duration::from_secs(2)).unwrap();
        let second = s.read_frame(Duration::from_secs(2)).unwrap();
        assert!(second.dropped);
        assert_eq!(s.stats().drops, 1);
        s.stop();
    }
}
