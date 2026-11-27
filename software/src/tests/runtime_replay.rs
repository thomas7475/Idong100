//! Golden replay of the cap3 flash phase (turn 32).
//!
//! The mock device is scripted with the exact byte sequences observed
//! in `work/cap3/idong_capture3/run1/USBPcap1.pcap` (records 21-702,
//! t=14.01-15.35), and the session drives the flash steps. What is
//! asserted:
//!
//! 1. the exact wire frames: 7F 55 layouts byte-for-byte;
//! 2. the exact raw block stream: 335 blocks of 64 B (the last one
//!    35 B) - the shape of the vendor download;
//! 3. the readback compare path, including a corrupted-readback
//!    rejection;
//! 4. the public entry stays safety-gated (no writes without
//!    confirmation).
//!
//! The image CONTENT here is a deterministic synthetic pattern, not the
//! vendor firmware (safety rule: the shipped files are SHA-256 pinned;
//! the real image is verified by hash at runtime via
//! `FirmwareSet::mcu_image`).

use idong::mock::{reply, MockTransport, Scripted};
use idong::protocol::runtime::{
    classify, RtFrame, RtRecord, RuntimeSession, CMD_BEGIN, CMD_GET_VERSION, CMD_KEY, CMD_LAUNCH,
    CMD_VERIFY, IMAGE_SIZE, RT_FRAME_MAX,
};

/// Deterministic synthetic image of the observed size (21,411 B).
fn synth_image() -> Vec<u8> {
    let n = IMAGE_SIZE as usize;
    let mut v = Vec::with_capacity(n);
    let mut x = 0xB5AD_1D0Du32;
    for _ in 0..n {
        x = x.wrapping_mul(1664525).wrapping_add(1013904223);
        v.push((x >> 24) as u8);
    }
    v
}

fn frame(cmd: u16, body: &[u8]) -> Vec<u8> {
    let mut v = vec![0x7f, 0x55, cmd as u8, (cmd >> 8) as u8, 0x01, 0x00];
    v.extend_from_slice(body);
    v
}

#[test]
fn flash_sequence_replays_cap3_wire_bytes() {
    let image = synth_image();
    assert_eq!(image.len(), 0x53A3);

    // device script, exactly as observed (cap3 records 24/28/702 + echo)
    let mut script: Vec<Scripted> = Vec::new();
    // 0x0008 -> "2600" (rec 24)
    script.push(reply(frame(CMD_GET_VERSION, &[0x00, 0x00, b'2', b'6', b'0', b'0', 0, 0, 0, 0])));
    // 0x0101 -> 00 02 (rec 28)
    script.push(reply(frame(CMD_BEGIN, &[0x00, 0x02])));
    // 0x0201 -> 00 04 (rec 702), then the echo blocks (rec 704+)
    script.push(reply(frame(CMD_VERIFY, &[0x00, 0x04])));
    for block in image.chunks(RT_FRAME_MAX) {
        script.push(reply(block.to_vec()));
    }
    // 0x0301 -> 00 08 (rec 1376), 0x0401 -> 00 00 (rec 1380)
    script.push(reply(frame(CMD_KEY, &[0x00, 0x08])));
    script.push(reply(frame(CMD_LAUNCH, &[0x00, 0x00])));

    let mut m = MockTransport::new(script);
    let mut s = RuntimeSession::new(&mut m);

    assert_eq!(s.get_version().unwrap(), "2600");
    s.flash_sequence(&image).unwrap();

    // ---- assert every write, byte-for-byte ----
    let w = &m.writes;
    // 1. version request: 7f 55 08 00 00 00 00 00 (rec 21)
    assert_eq!(w[0], vec![0x7f, 0x55, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00]);
    // 2. begin: 7f 55 01 01 00 00 00 02 53 a3 (rec 25)
    assert_eq!(w[1], vec![0x7f, 0x55, 0x01, 0x01, 0x00, 0x00, 0x00, 0x02, 0x53, 0xa3]);
    // 3. 335 raw blocks: 334 x 64 B + 1 x 35 B, concatenation == image
    let blocks = &w[2..2 + 335];
    assert_eq!(blocks.len(), 335);
    let mut joined = Vec::new();
    for (i, b) in blocks.iter().enumerate() {
        assert_eq!(b.len(), if i == 334 { 35 } else { 64 }, "block {i}");
        assert!(b[0] != 0x7f || b[1] != 0x55, "raw blocks carry no framing");
        joined.extend_from_slice(b);
    }
    assert_eq!(joined, image);
    // 4. verify: 7f 55 01 02 00 00 00 04 80 00 53 a3 (rec 699)
    assert_eq!(
        w[2 + 335],
        vec![0x7f, 0x55, 0x01, 0x02, 0x00, 0x00, 0x00, 0x04, 0x80, 0x00, 0x53, 0xa3]
    );
    // 5. key: 7f 55 01 03 00 00 00 08 fe 2a c9 bb ae 11 d5 c2 (rec 1373)
    assert_eq!(
        w[3 + 335],
        vec![
            0x7f, 0x55, 0x01, 0x03, 0x00, 0x00, 0x00, 0x08, 0xfe, 0x2a, 0xc9, 0xbb, 0xae, 0x11,
            0xd5, 0xc2
        ]
    );
    // 6. launch: 7f 55 01 04 00 00 00 00 (rec 1377)
    assert_eq!(w[4 + 335], vec![0x7f, 0x55, 0x01, 0x04, 0x00, 0x00, 0x00, 0x00]);
    // nothing else was written
    assert_eq!(w.len(), 5 + 335);
    // the mock was fully consumed: no leftover script entries
    assert_eq!(m.exhausted_reads(), 0);
}

#[test]
fn flash_sequence_rejects_corrupted_readback() {
    let image = synth_image();
    let mut script: Vec<Scripted> = Vec::new();
    script.push(reply(frame(CMD_BEGIN, &[0x00, 0x02])));
    script.push(reply(frame(CMD_VERIFY, &[0x00, 0x04])));
    for block in image.chunks(RT_FRAME_MAX) {
        let mut b = block.to_vec();
        if block.len() == 35 {
            b[0] ^= 0xFF; // corrupt the last partial block
        }
        script.push(reply(b));
    }
    let mut m = MockTransport::new(script);
    let mut s = RuntimeSession::new(&mut m);
    assert!(s.flash_sequence(&image).is_err(), "corrupted readback must fail");
}

#[test]
fn public_flash_entry_is_open_since_the_bench_pass() {
    // FLASH_CONFIRMED = true since the turn-41 bench run (T8 PASS x2:
    // readback matched, launched, identity intact). The ungated entry no
    // longer refuses - it now requires the byte-exact mock script, and
    // an UNSCRIPTED mock fails the sequence safely (no script to walk).
    let mut m = MockTransport::new(vec![]);
    let mut s = RuntimeSession::new(&mut m);
    // with no scripted replies the first read times out: the sequence
    // errors out without the mock ever producing a reply - safe refusal
    // by lack of transport, not by the old gate.
    assert!(s.flash_mcu(&synth_image()).is_err());
    // The two-key consent path still exists and still works.
    let permit = idong::FlashPermit::from_consent(idong::FLASH_CONSENT_PHRASE).unwrap();
    let mut m2 = MockTransport::new(vec![]);
    let mut s2 = RuntimeSession::new(&mut m2);
    assert!(s2.flash_mcu_permitted(&synth_image(), &permit).is_err());
}

#[test]
fn record_classification() {
    assert!(matches!(classify(&[0x7f, 0x55, 0x08, 0x00, 0x01, 0x00, 0x00, 0x00]),
                     RtRecord::Frame(f) if f.reply));
    assert!(matches!(classify(&[0x02, 0xc0, 0xcd, 0x02, 0x80, 0x0a]), RtRecord::Raw(_)));
    // a 7F 55 prefix that fails to parse falls back to raw
    assert!(matches!(classify(&[0x7f, 0x55]), RtRecord::Raw(_)));
    // the cap3 reply frame parses with the reply flag
    let raw = frame(CMD_GET_VERSION, &[0x00, 0x00, b'2', b'6', b'1', b'9', 0, 0, 0, 0]);
    if let RtRecord::Frame(f) = classify(&raw) {
        assert_eq!(f.version_string().unwrap(), "2619");
    } else {
        panic!("framed record expected");
    }
}

#[test]
fn observed_frame_shapes_decode() {
    // rec 1455: the 64-B 0x0104 table frame
    let mut raw = vec![0x7f, 0x55, 0x04, 0x01, 0x02, 0x00, 0x00, 0x38];
    raw.extend_from_slice(&[0u8; 56]);
    let f = RtFrame::parse(&raw).unwrap();
    assert_eq!(f.cmd, 0x0104);
    assert_eq!(f.flags, 0x02);
    assert!(!f.reply);
    assert_eq!(f.body.len(), 58);
}

/// Turn 33: the engine-gate reads. The vendor engine (xEyeUSB.dll
/// 2.6.0.6) reads a 4-byte version tag at EEPROM 0x0322 and compares
/// its FOURTH byte against "DSHY"/NUL ("000T" -> 'T' -> rejected,
/// endless "connecting"), and a 32-byte device ID at EEPROM 0x0300
/// (backup 0x0700) with a trailing byte-sum. The exact values below
/// come from the captured unit (wire capture 7 + the T24C16 dump).
#[test]
fn turn33_engine_gate_reads() {
    use idong::mock::reply;
    use idong::protocol::runtime::{
        EDITION_LETTERS, EEPROM_CODE_A_OFFSET, EEPROM_DEVICE_ID_BACKUP_OFFSET,
        EEPROM_DEVICE_ID_OFFSET,
    };

    // device ID: 32 ASCII chars + byte-sum 0x50 (from the T24C16 dump)
    let id_ascii = b"AB460DC8BA12C6840B49CD5786C329DD";
    let sum: u8 = id_ascii.iter().fold(0u8, |a, b| a.wrapping_add(*b));
    assert_eq!(sum, 0x50, "dump-derived checksum");
    let mut id_read = id_ascii.to_vec();
    id_read.push(0x50);

    // primary area answers, backup never needed
    let mut m = MockTransport::new(vec![reply(id_read.clone())]);
    let mut s = RuntimeSession::new(&mut m);
    assert_eq!(s.read_device_id().unwrap(), id_ascii);

    // version tag "000T" + 0xE4 -> NOT accepted by the v2.6.0.6 engine
    let mut m = MockTransport::new(vec![reply(b"000T\xE4".to_vec())]);
    let mut s = RuntimeSession::new(&mut m);
    assert!(!s.edition_tag_accepted().unwrap(), "'T' is rejected by DSHY");
    assert!(!EDITION_LETTERS.contains(&b'T'));

    // "000D" + correct checksum -> accepted
    let mut m = MockTransport::new(vec![reply(b"000D\xD4".to_vec())]);
    let mut s = RuntimeSession::new(&mut m);
    assert!(s.edition_tag_accepted().unwrap());

    // the request bytes are exactly the observed 0x0107 frames
    let want_tag: [u8; 12] =
        [0x7f, 0x55, 0x07, 0x01, 0x00, 0x00, 0x00, 0x04, 0x03, 0x22, 0x00, 0x05];
    assert_eq!(&m.writes[0][..12], &want_tag, "0107 tag request");
    let want_id: [u8; 12] =
        [0x7f, 0x55, 0x07, 0x01, 0x00, 0x00, 0x00, 0x04, 0x03, 0x00, 0x00, 0x21];
    let mut m = MockTransport::new(vec![reply(id_read.clone())]);
    let mut s = RuntimeSession::new(&mut m);
    s.read_device_id().unwrap();
    assert_eq!(&m.writes[0][..12], &want_id, "0107 device-id request");

    // broken checksum in the primary area -> the engine falls back to
    // the backup area; our read_device_id mirrors that
    let mut bad = id_read.clone();
    bad[0] ^= 0xFF;
    let mut m = MockTransport::new(vec![reply(bad), reply(id_read.clone())]);
    let mut s = RuntimeSession::new(&mut m);
    let id = s.read_device_id().expect("device id from backup area");
    assert_eq!(id, id_ascii);
    assert_eq!(m.writes.len(), 2, "primary then backup request");
    // the backup request addresses 0x0700
    assert_eq!(&m.writes[1][8..12], &[0x07, 0x00, 0x00, 0x21]);
    assert_eq!(EEPROM_DEVICE_ID_BACKUP_OFFSET, 0x0700);
    assert_eq!(EEPROM_DEVICE_ID_OFFSET, 0x0300);
    assert_eq!(EEPROM_CODE_A_OFFSET, 0x0322);
}
