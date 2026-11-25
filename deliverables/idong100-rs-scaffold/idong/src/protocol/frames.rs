//! Frame codec + resynchronizing stream decoder.
//!
//! Boot framing: `[u32 magic LE][u16 type LE][payload]`; no length field,
//! so the decoder scans for magic on resync (draft section 6: decode must
//! be total - arbitrary bytes never panic; fuzz target in tests/).

use super::{MAGIC_D2H, MAGIC_H2D};

#[derive(Debug, Clone)]
pub struct Frame {
    pub magic: u32,
    pub kind: u16,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn h2d(kind: u16, payload: &[u8]) -> Self {
        Frame { magic: MAGIC_H2D, kind, payload: payload.to_vec() }
    }
}

/// Push bytes, get complete frames; skips garbage between magics.
// TODO(P1.2): implement + fuzz; golden vectors under tests/data/.
pub struct FrameDecoder { buf: Vec<u8> }
impl FrameDecoder {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Frame> { let _ = chunk; Vec::new() }
}
