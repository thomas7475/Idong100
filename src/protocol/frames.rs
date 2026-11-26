//! Frame codec + resynchronizing stream decoder.
//!
//! Boot framing: `[u32 magic LE][u16 type LE][payload]`; no length field,
//! so the decoder scans for magic on resync (draft section 6: decode must
//! be total - arbitrary bytes never panic; fuzz target in tests/).
//! Frames are bulk-packet delimited: in the boot phase a full 64-byte
//! bulk packet IS a frame; in the runtime stream the record length is
//! derived by the record parser, so the decoder consumes a whole bulk
//! chunk per frame and the caller re-slices.

use super::{MAGIC_D2H, MAGIC_H2D};

/// A single boot-protocol frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub magic: u32,
    pub kind: u16,
    pub payload: Vec<u8>,
}

impl Frame {
    /// New host-to-device frame.
    pub fn h2d(kind: u16, payload: &[u8]) -> Self {
        Frame { magic: MAGIC_H2D, kind, payload: payload.to_vec() }
    }

    /// New device-to-host frame (for tests / replay).
    pub fn d2h(kind: u16, payload: &[u8]) -> Self {
        Frame { magic: MAGIC_D2H, kind, payload: payload.to_vec() }
    }

    /// Header size (magic + type).
    pub const HEADER: usize = 6;

    /// Encode into `out`.
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.reserve(Self::HEADER + self.payload.len());
        out.extend_from_slice(&self.magic.to_le_bytes());
        out.extend_from_slice(&self.kind.to_le_bytes());
        out.extend_from_slice(&self.payload);
    }

    /// Encode into a fresh buffer.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(Self::HEADER + self.payload.len());
        self.encode(&mut v);
        v
    }

    /// Parse one frame from the front of `buf`. Returns the frame and the
    /// number of bytes consumed; `None` when more data is needed or the
    /// buffer does not start with a known magic.
    pub fn parse(buf: &[u8]) -> Option<(Frame, usize)> {
        if buf.len() < Self::HEADER {
            return None;
        }
        let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        if magic != MAGIC_H2D && magic != MAGIC_D2H {
            return None;
        }
        let kind = u16::from_le_bytes([buf[4], buf[5]]);
        // No length field: a frame is one bulk packet. Consume everything
        // available; the runtime record parser re-slices by record size.
        let payload = buf[Self::HEADER..].to_vec();
        let n = buf.len();
        Some((Frame { magic, kind, payload }, n))
    }
}

/// Incremental decoder for a byte stream: scans for either magic, then
/// emits frames at bulk-packet boundaries. Used by the read loops;
/// unit-tested against golden byte streams, including resync over
/// garbage (decode must be total - never a panic).
#[derive(Debug, Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
    /// Bytes skipped while hunting for a magic (resync accounting).
    pub resyncs: u64,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a chunk; returns all complete frames found. Leading garbage
    /// before a magic is dropped and counted in `resyncs`.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Frame> {
        self.buf.extend_from_slice(chunk);
        let mut frames = Vec::new();
        loop {
            // find a magic prefix
            let idx = match find_magic(&self.buf) {
                Some(i) => i,
                None => {
                    // keep the last 3 bytes (partial magic candidate)
                    let keep = self.buf.len().saturating_sub(3);
                    if keep > 0 {
                        self.resyncs += keep as u64;
                        self.buf.drain(..keep);
                    }
                    break;
                }
            };
            if idx > 0 {
                self.resyncs += idx as u64;
                self.buf.drain(..idx);
            }
            if self.buf.len() < Frame::HEADER {
                break;
            }
            if let Some((f, n)) = Frame::parse(&self.buf) {
                self.buf.drain(..n);
                frames.push(f);
            } else {
                // magic parse refused (mixed magic) - skip 1 byte
                self.buf.drain(..1);
                self.resyncs += 1;
            }
        }
        frames
    }
}

fn find_magic(buf: &[u8]) -> Option<usize> {
    if buf.len() < 4 {
        return None;
    }
    for i in 0..=buf.len() - 4 {
        let m = u32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]);
        if m == MAGIC_H2D || m == MAGIC_D2H {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_h2d() {
        let f = Frame::h2d(3, &[0xAA; 64]);
        let b = f.to_bytes();
        assert_eq!(&b[..4], &[0x50, 0x55, 0x7f, 0x7f]);
        let (g, n) = Frame::parse(&b).unwrap();
        assert_eq!(n, b.len());
        assert_eq!(g, f);
    }

    #[test]
    fn roundtrip_d2h() {
        let f = Frame::d2h(0xFFF, &[0x01]);
        let b = f.to_bytes();
        assert_eq!(&b[..4], &[0x4f, 0x44, 0x7f, 0x7f]);
        let (g, _) = Frame::parse(&b).unwrap();
        assert_eq!(g.kind, 0xFFF);
    }

    #[test]
    fn scanner_resync_over_garbage() {
        let mut d = FrameDecoder::new();
        let mut stream = vec![0u8; 37];
        let f = Frame::d2h(2, b"Ready to load program!");
        stream.extend_from_slice(&f.to_bytes());
        let frames = d.push(&stream);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].kind, 2);
        assert_eq!(&frames[0].payload, b"Ready to load program!");
        assert_eq!(d.resyncs, 37, "garbage bytes counted as resyncs");
    }

    #[test]
    fn split_chunks_reassemble() {
        let mut d = FrameDecoder::new();
        let b = Frame::d2h(4, &[0x11, 0x00, 0x01]).to_bytes();
        assert!(d.push(&b[..3]).is_empty());
        let frames = d.push(&b[3..]);
        assert_eq!(frames.len(), 1);
    }

    /// Decode must be total: no input, however malformed, may panic.
    #[test]
    fn garbage_never_panics() {
        let mut d = FrameDecoder::new();
        // pseudo-random garbage incl. truncated magics at the tail
        let mut seed = 0x1234_5678u64;
        for _ in 0..2000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let chunk = seed.to_le_bytes();
            let _ = d.push(&chunk);
        }
        let _ = d.push(&[0x50, 0x55]);
    }

    #[test]
    fn keepalive_frame_shape() {
        // 0xFFF keep-alive: D2H magic + kind 0xFFF (wire order verified
        // in the boot-protocol disassembly: word @ frame+4 == 0xfff).
        let f = Frame::d2h(0x0FFF, &[]);
        let b = f.to_bytes();
        assert_eq!(&b[..6], &[0x4f, 0x44, 0x7f, 0x7f, 0xff, 0x0f]);
    }
}
