//! Runtime record plane - the PU/OD framing on EP 0x83/0x03 (turn 46).
//!
//! Source: xEyeUSB.dll v2.6.0.6 disassembly (the `usbCom` worker @RVA
//! 0x11770 and the `ens` subsystem @0xeb00-0x10900; artifacts in
//! `research/idong100/agents/20-turn46/`). This is the third protocol
//! layer: the 7F 55 session stays on EP 0x02/0x82, the record plane
//! runs on the second MI_04 bulk pair. EasyUSB pipe calibration: the
//! 7F 55 engine writes pipe 0 / reads pipe 1 (USBPcap-proven EP
//! 0x02/0x82), so usbCom's read pipe 2 / write pipe 3 are EP
//! 0x83/0x03.
//!
//! The vendor exchange, per `usbCom` loop:
//!
//! 1. read a 64-B OD frame of type [`TYPE_RECORD_READY`] (0xF0) from
//!    EP 0x83 - one frame announces one record;
//! 2. read the 640-B raw record from EP 0x83 (it ends with the OD
//!    magic dword at +0x27C);
//! 3. write a 64-B PU frame of type [`TYPE_RECORD_CMD`] (7) to EP
//!    0x03 - the ACK for the NEXT record, whose payload the vendor
//!    stack fills from the PREVIOUS record's classification
//!    (`classifying.dll!ClassifyAndTrack`); the very first one is
//!    zero bytes;
//! 4. push the record into the host pipeline; the loop paces at
//!    USBComFPS = 35.
//!
//! The 640-B wire record carries RAW blob data (the classifier input,
//! `SVM_Class_ball/head_*.dat` training) - the marker x/y/z values of
//! the host-side xEye3Ddata are computed ON THE HOST. Our model keeps
//! the raw bytes and decodes the one byte that is understood on the
//! wire: the status byte at +0x25C (the f660 transform @0xf660).
//!
//! If no type-0xF0 frame answers the startup probe (read 64 B, ten
//! tries, 100 ms - the vendor probe @RVA 0x10640), the vendor runs
//! the `ens_code.ldr` upload state machine (@RVA 0xf790) on the same
//! endpoint pair first. That state machine's wire bytes are the open
//! follow-up; until they are pinned (one capture with the kit-37
//! unlocked app in a game), the probe is the decision point.

use super::{
    MAGIC_D2H, MAGIC_H2D, RECORD_FRAME_LEN, RECORD_STATUS_OFF, RECORD_TRAILER_OFF, RECORD_WIRE_LEN,
    TYPE_RECORD_CMD, TYPE_RECORD_READY,
};

/// A 64-B device-to-host frame read from EP 0x83.
#[derive(Debug, Clone)]
pub struct StatusFrame {
    /// Full frame bytes (the payload semantics are unpinned - kept raw).
    pub bytes: [u8; RECORD_FRAME_LEN],
}

impl StatusFrame {
    /// Frame type (u16 @ +4).
    pub fn frame_type(&self) -> u16 {
        u16::from_le_bytes([self.bytes[4], self.bytes[5]])
    }

    /// Parse a 64-B read: it must carry the OD magic and announce a
    /// record (type [`TYPE_RECORD_READY`]).
    pub fn parse(buf: &[u8]) -> Option<StatusFrame> {
        if buf.len() < RECORD_FRAME_LEN {
            return None;
        }
        if u32::from_le_bytes(buf[0..4].try_into().ok()?) != MAGIC_D2H {
            return None;
        }
        let frame_type = u16::from_le_bytes([buf[4], buf[5]]);
        if frame_type != TYPE_RECORD_READY {
            return None;
        }
        let mut bytes = [0u8; RECORD_FRAME_LEN];
        bytes.copy_from_slice(&buf[..RECORD_FRAME_LEN]);
        Some(StatusFrame { bytes })
    }
}

/// The flags the vendor f660 transform (@RVA 0xf660) decodes from the
/// wire record's status byte at +0x25C.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecordStatus {
    /// Raw status byte (record +0x25C).
    pub raw: u8,
    /// Status bit 0 -> the xEye3Ddata mode word (record +0x3B0);
    /// the pipe writer skips the frame when mode == 1.
    pub mode: u32,
    /// Extended flags (status bit 7 set): flag A = bit 4 == 0.
    pub flag_a: bool,
    /// Extended flags (status bit 7 set): flag B = bit 5 == 0.
    pub flag_b: bool,
    /// Extended flags (status bit 7 set): flag C = bit 3 == 0.
    pub flag_c: bool,
}

impl RecordStatus {
    /// Decode the status byte exactly like f660: bit 7 gates the three
    /// flag bits (4/5/3, inverted); bit 0 is the mode word.
    pub fn from_raw(raw: u8) -> RecordStatus {
        let ext = raw & 0x80 != 0;
        RecordStatus {
            raw,
            mode: u32::from(raw & 1),
            flag_a: ext && raw & 0x10 == 0,
            flag_b: ext && raw & 0x20 == 0,
            flag_c: ext && raw & 0x08 == 0,
        }
    }
}

/// One 640-B raw record read from EP 0x83.
///
/// The payload region +0x000..+0x25C is the raw blob data (the
/// classifier input - layout unpinned, kept for offline analysis);
/// the trailer magic and status byte are verified/decoded here.
#[derive(Debug, Clone)]
pub struct WireRecord {
    /// Full record bytes (64 B status frame header excluded - this is
    /// the 0x280-B payload read).
    pub bytes: [u8; RECORD_WIRE_LEN],
}

impl WireRecord {
    /// Parse the 0x280-B read: length exact, OD magic trailer at
    /// +0x27C (usbCom loop @0x11d13).
    pub fn parse(buf: &[u8]) -> Option<WireRecord> {
        if buf.len() != RECORD_WIRE_LEN {
            return None;
        }
        if u32::from_le_bytes(buf[RECORD_TRAILER_OFF..RECORD_TRAILER_OFF + 4].try_into().ok()?)
            != MAGIC_D2H
        {
            return None;
        }
        let mut bytes = [0u8; RECORD_WIRE_LEN];
        bytes.copy_from_slice(buf);
        Some(WireRecord { bytes })
    }

    /// The decoded status byte (f660 transform).
    pub fn status(&self) -> RecordStatus {
        RecordStatus::from_raw(self.bytes[RECORD_STATUS_OFF])
    }

    /// Non-zero byte count of the raw blob region (bench statistic:
    /// real sensor data vs. an all-zero record - the same idea as the
    /// video plane's PlaneStats contrast check).
    pub fn blob_nonzero(&self) -> usize {
        self.bytes[..RECORD_STATUS_OFF].iter().filter(|&&b| b != 0).count()
    }

    /// The raw blob region (the classifier input).
    pub fn blob(&self) -> &[u8] {
        &self.bytes[..RECORD_STATUS_OFF]
    }
}

/// A 64-B host-to-device frame of type [`TYPE_RECORD_CMD`] for EP 0x03.
#[derive(Debug, Clone)]
pub struct Type7Command {
    /// Full frame bytes.
    pub bytes: [u8; RECORD_FRAME_LEN],
}

impl Type7Command {
    /// Payload offset inside the 64-B frame (usbCom builds the frame at
    /// +0 and lets ClassifyAndTrack fill from +0xC).
    pub const PAYLOAD_OFF: usize = 0x0c;

    /// The zero ACK: the vendor loop's first write (a zeroed 64-B
    /// buffer, memset @0x119a6 before the loop starts).
    pub fn ack_zeros() -> Type7Command {
        Type7Command { bytes: [0u8; RECORD_FRAME_LEN] }
    }

    /// Build a type-7 command with a payload (the frame the vendor
    /// classifier fills: PU magic, type 7, byte 0 at +0xA, payload at
    /// +0xC - construction @0x11d91).
    pub fn with_payload(payload: &[u8]) -> Type7Command {
        let mut bytes = [0u8; RECORD_FRAME_LEN];
        bytes[..4].copy_from_slice(&MAGIC_H2D.to_le_bytes());
        bytes[4..6].copy_from_slice(&TYPE_RECORD_CMD.to_le_bytes());
        bytes[0x0a] = 0;
        let n = payload.len().min(RECORD_FRAME_LEN - Self::PAYLOAD_OFF);
        bytes[Self::PAYLOAD_OFF..Self::PAYLOAD_OFF + n].copy_from_slice(&payload[..n]);
        Type7Command { bytes }
    }

    /// Frame type (u16 @ +4).
    pub fn frame_type(&self) -> u16 {
        u16::from_le_bytes([self.bytes[4], self.bytes[5]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_frame_bytes(frame_type: u16) -> [u8; RECORD_FRAME_LEN] {
        let mut b = [0u8; RECORD_FRAME_LEN];
        b[..4].copy_from_slice(&MAGIC_D2H.to_le_bytes());
        b[4..6].copy_from_slice(&frame_type.to_le_bytes());
        b
    }

    #[test]
    fn status_frame_accepts_only_od_type_f0() {
        let f = StatusFrame::parse(&status_frame_bytes(TYPE_RECORD_READY)).unwrap();
        assert_eq!(f.frame_type(), TYPE_RECORD_READY);

        // wrong type: the 7F 55 session lives on EP 0x82, but a link
        // status frame (type 4) read from 0x83 must NOT pass as a
        // record announcement
        assert!(StatusFrame::parse(&status_frame_bytes(4)).is_none());
        // PU magic (an echo of our own command) must not pass either
        let mut pu = status_frame_bytes(TYPE_RECORD_READY);
        pu[..4].copy_from_slice(&MAGIC_H2D.to_le_bytes());
        assert!(StatusFrame::parse(&pu).is_none());
        // short reads reject
        assert!(StatusFrame::parse(&status_frame_bytes(0xf0)[..63]).is_none());
    }

    #[test]
    fn wire_record_needs_exact_len_and_trailer() {
        let mut rec = [0u8; RECORD_WIRE_LEN];
        // no trailer -> reject
        assert!(WireRecord::parse(&rec).is_none());
        rec[RECORD_TRAILER_OFF..RECORD_TRAILER_OFF + 4].copy_from_slice(&MAGIC_D2H.to_le_bytes());
        let r = WireRecord::parse(&rec).unwrap();
        assert_eq!(r.blob_nonzero(), 0);
        assert_eq!(r.status().mode, 0);
        // 639 or 641 bytes: reject (the usbCom loop demands exactly 0x280)
        assert!(WireRecord::parse(&rec[..RECORD_WIRE_LEN - 1]).is_none());
        assert!(WireRecord::parse(&[&rec[..], rec[..1].as_ref()].concat()).is_none());
    }

    #[test]
    fn status_byte_decodes_like_the_f660_transform() {
        // bit 0 -> mode 1 (pipe-writer skip flag)
        let s = RecordStatus::from_raw(0x01);
        assert_eq!(s.mode, 1);
        assert!(!s.flag_a && !s.flag_b && !s.flag_c, "bit 7 clear -> flags stay 0");
        // bit 7 set, bits 4/5/3 clear -> all three flags true (f660: the
        // flag is (status & bit) == 0, gated on 0x80)
        let s = RecordStatus::from_raw(0x80);
        assert_eq!(s.mode, 0);
        assert!(s.flag_a && s.flag_b && s.flag_c);
        // bit 7 set, bits 4/5/3 set -> all three flags false
        let s = RecordStatus::from_raw(0xB8);
        assert!(!s.flag_a && !s.flag_b && !s.flag_c);
        // mixed: bit 5 set only -> flag B false, others true
        let s = RecordStatus::from_raw(0xA0);
        assert!(s.flag_a && s.flag_c);
        assert!(!s.flag_b);
    }

    #[test]
    fn wire_record_status_and_blob_reported() {
        let mut rec = [0u8; RECORD_WIRE_LEN];
        rec[RECORD_TRAILER_OFF..RECORD_TRAILER_OFF + 4].copy_from_slice(&MAGIC_D2H.to_le_bytes());
        rec[0] = 0xAA;
        rec[1] = 0xBB;
        rec[RECORD_STATUS_OFF] = 0x81;
        let r = WireRecord::parse(&rec).unwrap();
        assert_eq!(r.blob_nonzero(), 2);
        assert_eq!(r.blob().len(), RECORD_STATUS_OFF);
        assert_eq!(r.status().raw, 0x81);
        assert_eq!(r.status().mode, 1);
        assert!(r.status().flag_a, "0x81: bit7 set, bit4 clear -> flag_a");
    }

    #[test]
    fn type7_command_layout_matches_the_vendor_builder() {
        // zero ACK: 64 zero bytes (the vendor's first write)
        let ack = Type7Command::ack_zeros();
        assert_eq!(ack.bytes, [0u8; RECORD_FRAME_LEN]);

        // payload command: PU magic + type 7 + payload @ +0xC
        let cmd = Type7Command::with_payload(&[0x11, 0x22, 0x33]);
        assert_eq!(u32::from_le_bytes(cmd.bytes[0..4].try_into().unwrap()), MAGIC_H2D);
        assert_eq!(cmd.frame_type(), TYPE_RECORD_CMD);
        assert_eq!(cmd.frame_type(), 7, "the vendor builder writes word 7 @ +4");
        assert_eq!(cmd.bytes[0x0a], 0, "byte 0 at +0xA (usbCom @0x11da3)");
        assert_eq!(&cmd.bytes[0x0c..0x0f], &[0x11, 0x22, 0x33]);
        // payload longer than the frame is clipped, never overflows
        let long = Type7Command::with_payload(&[0xFFu8; 256]);
        assert_eq!(long.bytes[RECORD_FRAME_LEN - 1], 0xFF);
    }

    #[test]
    fn constants_cite_the_disassembly() {
        // the sizes the usbCom loop and the ens states actually use
        assert_eq!(RECORD_WIRE_LEN, 0x280, "read len @0x11c80");
        assert_eq!(RECORD_TRAILER_OFF, 0x27c, "trailer check @0x11d13");
        assert_eq!(RECORD_STATUS_OFF, 0x25c, "f660 reads byte @ +0x25C");
        assert_eq!(TYPE_RECORD_READY, 0xF0, "type check @0x11c59");
        assert_eq!(TYPE_RECORD_CMD, 7, "word 7 @0x11d97");
        assert_eq!(RECORD_FRAME_LEN, 64, "64-B frames everywhere in usbCom/ens");
    }
}
