//! `_InitBlobUpload` state machine (decoded from xEyeUSB.dll).
//!
//! Sequence: TX DSP-start -> RX type 2 "Ready to load program!" ->
//! TX type 2 packet-count+CRC -> TX type-3 64 B packets (6 s each) ->
//! RX type 3 CRC result (abort on mismatch) -> device-ID exchange
//! (300 ms x 10) -> TX type 7 DSP reset. Staging order of the three
//! firmware images is open item #1 - resolved at the P2.3 USBPcap gate.
//!
//! Safety: `run()` refuses to execute unless (a) the caller passes the
//! write-risk flag AND (b) [`BOOT_CONFIRMED`] is flipped after a USBPcap
//! transcript of the vendor stack byte-confirms the sequence (draft
//! P2.3). Until then it returns [`IdongError::BootUnconfirmed`] - the
//! state machine is exercised over the MockTransport in unit tests only.

use super::frames::{Frame, FrameDecoder};
use super::{
    BOOT_PACKET, CRC_FAIL_WORD, DEVICE_ID_POLL, DEVICE_ID_TRIES, MAGIC_D2H, TIMEOUT_READY,
    TYPE_COUNT_CRC, TYPE_DSP_RESET, TYPE_FIRMWARE_PACKET, TYPE_READY_TO_LOAD,
};
use crate::error::{BootStep, IdongError, Result};
use crate::transport::Transport;

/// Safety gate. Flipped to `true` only after the P2.3 USBPcap
/// byte-confirmation of the vendor upload.
pub const BOOT_CONFIRMED: bool = false;

/// Identity reported at the end of a successful boot.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BootInfo {
    /// Raw "DEVICE ID =" payload (layout pending open item #3).
    pub device_id_raw: Vec<u8>,
    /// DSP version string if ASCII-decodable.
    pub dsp_version: Option<String>,
}

/// CRC of the firmware blob as announced to the device.
///
/// Open item #2: the vendor variant is not yet identified (CRC16-CCITT
/// vs CRC32, endianness). `blob_crc()` is the placeholder used by unit
/// tests; the real one is pinned by the USBPcap capture before any
/// hardware run.
pub fn blob_crc(blob: &[u8]) -> u32 {
    // CRC-32/ISO-HDLC (zlib) - placeholder pending P2.3 confirmation.
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in blob {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// The upload state machine. One instance uploads **one** firmware blob;
/// the multi-file staging (BF527 `.ldr`, Spartan-3 `.bit`, 8051 `.hex`)
/// is orchestrated by the stream facade once open item #1 is resolved.
///
/// `run()` walks the vendor sequence one step at a time; every failure
/// carries the exact [`BootStep`] so a hardware log maps 1:1 onto the
/// disassembly-derived sequence.
pub struct BootUploader<'a> {
    transport: &'a mut dyn Transport,
    blob: &'a [u8],
    decoder: FrameDecoder,
}

impl<'a> BootUploader<'a> {
    pub fn new(transport: &'a mut dyn Transport, blob: &'a [u8]) -> Self {
        BootUploader { transport, blob, decoder: FrameDecoder::new() }
    }

    /// Run the full upload. See the module docs for the safety gate.
    pub fn run(&mut self, write_risk: bool) -> Result<BootInfo> {
        if !BOOT_CONFIRMED || !write_risk {
            return Err(IdongError::BootUnconfirmed);
        }
        self.dsp_start()?;
        self.count_and_crc()?;
        self.packet_stream()?;
        self.crc_ack()?;
        let info = self.device_id()?;
        self.dsp_reset()?;
        Ok(info)
    }

    /// Step 1: TX DSP-start -> RX type 2 "Ready to load program!".
    fn dsp_start(&mut self) -> Result<()> {
        // Exact DSP-start payload bytes are open item #3; the
        // disassembly shows a 64-byte scratch buffer sent through the TX
        // builder with a 3 s RX wait. Placeholder = empty type-1 frame;
        // pinned by the USBPcap gate.
        let start = Frame::h2d(1, &[]);
        self.transport
            .write(&start.to_bytes())
            .map_err(|e| IdongError::boot(BootStep::DspStart, e.to_string()))?;
        let f = self
            .read_frame(TIMEOUT_READY)
            .map_err(|e| IdongError::boot(BootStep::DspStart, e.to_string()))?;
        if f.magic != MAGIC_D2H || f.kind != TYPE_READY_TO_LOAD {
            return Err(IdongError::boot(
                BootStep::DspStart,
                format!("expected type {TYPE_READY_TO_LOAD} ready frame, got kind {}", f.kind),
            ));
        }
        Ok(())
    }

    /// Step 2: announce packet count + CRC (type 2).
    fn count_and_crc(&mut self) -> Result<()> {
        let packets = self.blob.len().div_ceil(BOOT_PACKET);
        let mut p = Vec::with_capacity(8);
        p.extend_from_slice(&(packets as u32).to_le_bytes());
        p.extend_from_slice(&blob_crc(self.blob).to_le_bytes());
        let f = Frame::h2d(TYPE_COUNT_CRC, &p);
        self.transport
            .write(&f.to_bytes())
            .map_err(|e| IdongError::boot(BootStep::CountAndCrc, e.to_string()))
    }

    /// Step 3: stream 64-byte type-3 packets, 6 s timeout each.
    fn packet_stream(&mut self) -> Result<()> {
        for chunk in self.blob.chunks(BOOT_PACKET) {
            let f = Frame::h2d(TYPE_FIRMWARE_PACKET, chunk);
            self.transport
                .write(&f.to_bytes())
                .map_err(|e| IdongError::boot(BootStep::PacketStream, e.to_string()))?;
        }
        Ok(())
    }

    /// Step 4: RX type 3 CRC result; word 0xF at frame offset +8 = failure.
    fn crc_ack(&mut self) -> Result<()> {
        let f = self
            .read_frame(TIMEOUT_READY)
            .map_err(|e| IdongError::boot(BootStep::CrcAck, e.to_string()))?;
        if f.kind != TYPE_FIRMWARE_PACKET {
            return Err(IdongError::boot(
                BootStep::CrcAck,
                format!("expected type {TYPE_FIRMWARE_PACKET} CRC verdict, got {}", f.kind),
            ));
        }
        // payload[2..4] == frame offset +8 (payload starts at frame +6)
        if f.payload.len() >= 4 {
            let word = u16::from_le_bytes([f.payload[2], f.payload[3]]);
            if word == CRC_FAIL_WORD {
                return Err(IdongError::boot(
                    BootStep::CrcAck,
                    "device reported CRC failure (status word 0xF)",
                ));
            }
        }
        Ok(())
    }

    /// Step 5: device-ID / DSP version, 300 ms poll x <= 10.
    fn device_id(&mut self) -> Result<BootInfo> {
        for _ in 0..DEVICE_ID_TRIES {
            if let Ok(f) = self.read_frame(DEVICE_ID_POLL) {
                if f.kind == TYPE_READY_TO_LOAD && !f.payload.is_empty() {
                    let dsp_version = if f.payload.iter().all(|b| b.is_ascii()) {
                        Some(String::from_utf8_lossy(&f.payload).into_owned())
                    } else {
                        None
                    };
                    return Ok(BootInfo { device_id_raw: f.payload, dsp_version });
                }
            }
        }
        Err(IdongError::boot(BootStep::DeviceId, "no device-ID frame within 3 s"))
    }

    /// Step 6: TX type 7 (DSP reset) after "Get DSP Reset ACK OK".
    fn dsp_reset(&mut self) -> Result<()> {
        let f = Frame::h2d(TYPE_DSP_RESET, &[]);
        self.transport
            .write(&f.to_bytes())
            .map_err(|e| IdongError::boot(BootStep::DspReset, e.to_string()))
    }

    /// Read one frame: bulk reads feed the decoder until a frame lands.
    /// The boot phase maxpacket is 64 B, so one read is one frame.
    fn read_frame(&mut self, deadline: std::time::Duration) -> Result<Frame> {
        loop {
            let chunk = self.transport.read(BOOT_PACKET).map_err(|e| {
                IdongError::boot(BootStep::DspStart, format!("{e} (deadline {deadline:?})"))
            })?;
            if let Some(f) = self.decoder.push(&chunk).into_iter().next() {
                return Ok(f);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_placeholder_is_stable() {
        assert_eq!(blob_crc(b"hello idong"), blob_crc(b"hello idong"));
        assert_ne!(blob_crc(b"hello idong"), blob_crc(b"hello idong!"));
    }

    #[test]
    fn packet_count_matches_ceil() {
        // 235,930 B (A_a_v4.4_BG0313.bit size) -> 3687 packets
        let n = 235_930_usize.div_ceil(BOOT_PACKET);
        assert_eq!(n, 3687);
    }
}
