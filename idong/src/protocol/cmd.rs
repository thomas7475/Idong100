//! Runtime command engine - the "7F 55" framing (xeyezHal.dll v2.6.0.6).
//!
//! H2D: [0]=0x7F [1]=0x55 [2]=cmd [3..5]=b2-b4 [6..7]=u16 arg (BYTE
//! -SWAPPED on the wire) [8..]=payload (<= 56 B).  D2H: [4]=status.
//! Commands: 3 register write (8 B chunks), 5 blob read, 6 GET_IMG_START
//! (status 1), 15 classifier write.  Timeouts 1 s TX / 5 s RX; data
//! pipe 3 for GetImageData.
//!
//! Safety: only vendor-decoded commands are representable (error.rs
//! rejects everything else - rule "no speculative writes"). The builder
//! is total: any payload over [`RUNTIME_PAYLOAD_MAX`] fails closed with
//! a `ProtocolError`, never truncates silently.

use super::{RUNTIME_MAGIC, RUNTIME_PAYLOAD_MAX, RUNTIME_RX_TIMEOUT, RUNTIME_TX_TIMEOUT};
use crate::error::{ProtocolError, Result};
use crate::transport::Transport;

/// Vendor-decoded runtime commands (xeyezHal call sites).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    /// Register write, 8-byte chunks (SetCmosTable loop).
    RegWrite = 3,
    /// Blob data read (GetBlobData).
    BlobRead = 5,
    /// GET_IMG_START, expects status 1 (GetImageBegin).
    ImgStart = 6,
    /// Classifier / register write, chunk loop @0x10003a50.
    Classifier = 15,
}

impl Cmd {
    fn from_u8(v: u8) -> Result<Cmd> {
        match v {
            3 => Ok(Cmd::RegWrite),
            5 => Ok(Cmd::BlobRead),
            6 => Ok(Cmd::ImgStart),
            15 => Ok(Cmd::Classifier),
            other => Err(ProtocolError::UnknownCommand(other).into()),
        }
    }
}

/// A runtime command frame under construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmdPacket {
    pub cmd: Cmd,
    /// Bytes b2-b4 (frame offsets 3..5); vendor usage varies per command.
    pub b: [u8; 2],
    /// u16 argument, byte-swapped on the wire by the builder @0x10003010.
    pub arg: u16,
    /// Payload, at most [`RUNTIME_PAYLOAD_MAX`] bytes.
    pub payload: Vec<u8>,
}

impl CmdPacket {
    pub fn new(cmd: Cmd, arg: u16, payload: &[u8]) -> Result<Self> {
        if payload.len() > RUNTIME_PAYLOAD_MAX {
            return Err(
                ProtocolError::Short { need: payload.len(), have: RUNTIME_PAYLOAD_MAX }.into()
            );
        }
        Ok(CmdPacket { cmd, b: [0, 0], arg, payload: payload.to_vec() })
    }

    /// Encode to wire bytes: `7F 55 cmd b2 b3 b4 arglo arghi payload`
    /// with the u16 arg **byte-swapped** (big-endian on the wire, LE host
    /// value - exactly the builder @0x10003010 sequence).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(8 + self.payload.len());
        v.extend_from_slice(&RUNTIME_MAGIC);
        v.push(self.cmd as u8);
        // b2-b4: the three parameter bytes at frame offsets 3..5
        v.extend_from_slice(&self.b);
        v.push(0); // b4 pad byte (offset 5)
                   // byte-swapped u16 (arg_hi arg_lo on the wire at offsets 6..7)
        v.extend_from_slice(&self.arg.to_be_bytes());
        v.extend_from_slice(&self.payload);
        v
    }

    /// Decode from wire bytes (the inverse of [`to_bytes`]); used by the
    /// mock transport to verify what the state machines sent.
    pub fn parse(buf: &[u8]) -> Result<Self> {
        if buf.len() < 8 || buf[..2] != RUNTIME_MAGIC {
            return Err(ProtocolError::Short { need: 8, have: buf.len() }.into());
        }
        let cmd = Cmd::from_u8(buf[2])?;
        let b = [buf[3], buf[4]];
        // frame [5] is a pad byte in the vendor layout
        let arg = u16::from_be_bytes([buf[6], buf[7]]);
        Ok(CmdPacket { cmd, b, arg, payload: buf[8..].to_vec() })
    }
}

/// Runtime client: keep-alive and link-status handling (the xEyeClass
/// reader-thread duties), operating over any [`Transport`].
pub struct RuntimeClient<'a> {
    transport: &'a mut dyn Transport,
}

/// Runtime link status decoded from a type-4 frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    /// Subcode 1: nominal.
    Nominal,
    /// Subcode 0xF: disturbance reported.
    Disturbed,
}

impl<'a> RuntimeClient<'a> {
    pub fn new(transport: &'a mut dyn Transport) -> Self {
        RuntimeClient { transport }
    }

    /// Answer an RX 0xFFF keep-alive with TX 0xFFE (1 s budget).
    /// Wire form: boot framing (PU magic) with kind 0xFFE.
    pub fn answer_keepalive(&mut self) -> Result<()> {
        let f = super::frames::Frame::h2d(super::KEEPALIVE_TX, &[]);
        self.transport.write(&f.to_bytes())
    }

    /// Parse a link-status (type 4) frame and produce the mandated reply
    /// (type 4 with word 0x11). Returns the decoded status.
    pub fn handle_link_status(&mut self, f: &super::frames::Frame) -> Result<LinkStatus> {
        use super::frames::Frame;
        if f.kind != super::TYPE_LINK_STATUS {
            return Err(
                ProtocolError::BadType { kind: f.kind, expected: super::TYPE_LINK_STATUS }.into()
            );
        }
        // payload starts at frame +6; the disassembly reads word @ +8
        // and dwords @ +0xC/+0x10.
        if f.payload.len() < 4 {
            return Err(ProtocolError::Short { need: 4, have: f.payload.len() }.into());
        }
        let sub = u16::from_le_bytes([f.payload[2], f.payload[3]]);
        let status = match sub {
            1 => LinkStatus::Nominal,
            0xF => LinkStatus::Disturbed,
            other => {
                return Err(ProtocolError::BadType {
                    kind: other,
                    expected: super::TYPE_LINK_STATUS,
                }
                .into())
            }
        };
        // Reply: type 4, word 0x11 (tx_builder @0x1001028e/0x10010327).
        let mut p = Vec::with_capacity(4);
        p.extend_from_slice(&0u16.to_le_bytes());
        p.extend_from_slice(&super::LINK_STATUS_REPLY_WORD.to_le_bytes());
        let reply = Frame::h2d(super::TYPE_LINK_STATUS, &p);
        self.transport.write(&reply.to_bytes())?;
        Ok(status)
    }

    /// Is `f` a keep-alive poll from the device?
    pub fn is_keepalive(f: &super::frames::Frame) -> bool {
        f.kind == super::KEEPALIVE_RX && f.magic == super::MAGIC_D2H
    }

    /// Send a runtime command and await the reply frame.
    /// RX timeout is 5 s; the reply status byte sits at D2H +4.
    pub fn execute(&mut self, pkt: &CmdPacket) -> Result<super::frames::Frame> {
        let _ = RUNTIME_TX_TIMEOUT;
        self.transport.write(&pkt.to_bytes())?;
        // read until a frame arrives (decoder assembled in the stream
        // layer; here a single bulk read is the common case)
        let chunk = self.transport.read(super::RUNTIME_CHUNK).map_err(|e| e)?;
        let mut d = super::frames::FrameDecoder::new();
        let frames = d.push(&chunk);
        frames.into_iter().next().ok_or(ProtocolError::ReadTimeout(RUNTIME_RX_TIMEOUT).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::IdongError;

    #[test]
    fn wire_layout_with_byte_swapped_arg() {
        // arg 0x1234 appears as 12 34 (BE) at wire offsets 6..8
        let p = CmdPacket::new(Cmd::RegWrite, 0x1234, &[0xAA; 4]).unwrap();
        let b = p.to_bytes();
        assert_eq!(&b[..2], &[0x7f, 0x55]);
        assert_eq!(b[2], 3);
        assert_eq!(&b[6..8], &[0x12, 0x34], "u16 arg byte-swapped");
        assert_eq!(&b[8..], &[0xAA; 4]);
        // roundtrip
        assert_eq!(CmdPacket::parse(&b).unwrap(), p);
    }

    #[test]
    fn only_vendor_commands_representable() {
        use crate::error::IdongError;
        assert!(Cmd::from_u8(2).is_err());
        assert!(Cmd::from_u8(6).is_ok());
        // unknown opcode parsed off the wire -> UnknownCommand
        let b = vec![0x7f, 0x55, 0x99, 0, 0, 0, 0, 0];
        match CmdPacket::parse(&b) {
            Err(IdongError::Protocol(ProtocolError::UnknownCommand(0x99))) => {}
            other => panic!("expected UnknownCommand, got {other:?}"),
        }
    }

    #[test]
    fn payload_over_max_fails_closed() {
        let big = [0u8; RUNTIME_PAYLOAD_MAX + 1];
        match CmdPacket::new(Cmd::Classifier, 0, &big) {
            Err(IdongError::Protocol(ProtocolError::Short { .. })) => {}
            other => panic!("expected payload-too-large failure, got {other:?}"),
        }
    }
}
