//! ENS startup replay (turn 49): the cold-start state machine (the
//! `ens_code.ldr` code-load upload) and the hot-start path driven
//! against a scripted device, asserting every frame the driver puts
//! on the wire byte-for-byte.
//!
//! The wire format was decoded statically from xEyeUSB.dll v2.6.0.6
//! (`0xf790` cold machine, `0x10640`/`0x10900` hot start, crc.dll
//! `GenerateCRC32`) - the PU/OD traffic no capture ever saw. These
//! tests pin the Rust replay to that decode:
//!
//! 1. the cold-start happy path (all states incl. the pairing loop):
//!    the exact announce frame {size, CRC}, the raw 64-B chunks, every
//!    state's PU frame and the decoded outcome (device ID, DSP
//!    version, lens params);
//! 2. handle-match off: type 0x26, state 6 skipped;
//! 3. the CRC verdict failure: the OD type-3 answer with a wrong word
//!    -> the vendor -11 error;
//! 4. the hot-start path: probe alive, three record cycles, two ACKs,
//!    the lens + DID exchange;
//! 5. the probe silence: hot start returns None (cold start needed).

use idong::mock::{reply, MockTransport, Scripted};
use idong::protocol::ens::{
    codeload_chunks, crc32_ens, pu_codeload_announce, pu_frame, pu_frame_word, ColdStart, HotStart,
    OdFrame,
};
use idong::protocol::{
    ENS_CODELOAD_OK_WORD, ENS_DEVICE_ID_LEN, ENS_DEVICE_VER_LEN, ENS_LENS_FRAME_LEN,
    ENS_LENS_PARAM_LEN, ENS_LINK_FOUND, ENS_LINK_NOT_FOUND, ENS_LINK_OVER, ENS_LINK_REPLY_FOUND,
    ENS_LINK_REPLY_NOT_FOUND, ENS_LINK_REPLY_OVER, ENS_LINK_REPLY_TIMEOUT, ENS_LINK_TIMEOUT,
    ENS_TYPE_CODELOAD, ENS_TYPE_CODELOAD_DONE, ENS_TYPE_DEVICE_ID, ENS_TYPE_DID,
    ENS_TYPE_HANDLE_CFG, ENS_TYPE_HOT_START, ENS_TYPE_ID_REQ, ENS_TYPE_KEEPALIVE_RX,
    ENS_TYPE_KEEPALIVE_TX, ENS_TYPE_LENS, ENS_TYPE_LINK, ENS_TYPE_MATCH_ON, ENS_TYPE_START,
    MAGIC_D2H, MAGIC_H2D, RECORD_FRAME_LEN, RECORD_WIRE_LEN, TYPE_RECORD_CMD, TYPE_RECORD_READY,
};

/// The synthetic loader image the mock device accepts (130 B - three
/// chunks, the last one partial: exercises the zero-pad path).
fn ldr_bytes() -> Vec<u8> {
    (0..130u8).collect()
}

fn od(frame_type: u16, fill: impl FnOnce(&mut [u8; RECORD_FRAME_LEN])) -> Vec<u8> {
    let mut b = [0u8; RECORD_FRAME_LEN];
    b[..4].copy_from_slice(&MAGIC_D2H.to_le_bytes());
    b[4..6].copy_from_slice(&frame_type.to_le_bytes());
    fill(&mut b);
    b.to_vec()
}

fn od64(frame_type: u16) -> Vec<u8> {
    od(frame_type, |_| {})
}

/// The DID frame: 32 B ID at +8, 8 B version at +0x28.
fn did_frame() -> Vec<u8> {
    od(ENS_TYPE_DEVICE_ID, |b| {
        for (i, slot) in b[8..8 + ENS_DEVICE_ID_LEN].iter_mut().enumerate() {
            *slot = b'A' + (i as u8) % 26;
        }
        b[0x28..0x28 + ENS_DEVICE_VER_LEN].copy_from_slice(b"V2.6.0.6");
    })
}

/// The 128-B lens frame: recognizable params at +8.
fn lens_frame(frame_type: u16) -> Vec<u8> {
    let mut b = [0u8; ENS_LENS_FRAME_LEN];
    b[..4].copy_from_slice(&MAGIC_D2H.to_le_bytes());
    b[4..6].copy_from_slice(&frame_type.to_le_bytes());
    for (i, slot) in b[8..ENS_LENS_FRAME_LEN].iter_mut().enumerate() {
        *slot = (i % 251) as u8;
    }
    b.to_vec()
}

fn link_frame(sub: u16) -> Vec<u8> {
    od(ENS_TYPE_LINK, |b| {
        b[8..10].copy_from_slice(&sub.to_le_bytes());
    })
}

fn f0_frame() -> Vec<u8> {
    od64(TYPE_RECORD_READY)
}

fn wire_record(seed: u8) -> Vec<u8> {
    let mut r = vec![0u8; RECORD_WIRE_LEN];
    for (i, b) in r.iter_mut().enumerate() {
        *b = seed.wrapping_add(i as u8);
    }
    r[0x27c..0x280].copy_from_slice(&MAGIC_D2H.to_le_bytes());
    r
}

/// The scripted device for the full cold start with the pairing loop.
fn cold_script() -> Vec<Scripted> {
    vec![
        reply(did_frame()),             // state 1: OD 1
        reply(od64(ENS_TYPE_CODELOAD)), // state 2: ready to load
        reply(od(ENS_TYPE_CODELOAD_DONE, |b| {
            b[8..10].copy_from_slice(&ENS_CODELOAD_OK_WORD.to_le_bytes());
        })),
        reply(od64(ENS_TYPE_HANDLE_CFG)),      // state 4: the prompt
        reply(od64(ENS_TYPE_HANDLE_CFG)),      // state 4: the ack
        reply(did_frame()),                    // state 5: OD 1 (ID + version)
        reply(od64(ENS_TYPE_KEEPALIVE_RX)),    // state 6: keep-alive -> 0xFFE
        reply(link_frame(ENS_LINK_FOUND)),     // found -> PU 4/0x11
        reply(link_frame(ENS_LINK_NOT_FOUND)), // not found -> PU 4/0xFF
        reply(link_frame(ENS_LINK_TIMEOUT)),   // timeout -> PU 4/3
        reply(link_frame(ENS_LINK_OVER)),      // over -> PU 4/9 + exit
        reply(lens_frame(ENS_TYPE_LENS)),      // state 7: OD 6 (128 B)
    ]
}

#[test]
fn cold_start_is_byte_exact() {
    let ldr = ldr_bytes();
    let mut m = MockTransport::new(cold_script());
    {
        let mut cs = ColdStart::new(&mut m, &ldr);
        let out = cs.run(true).expect("the scripted device accepts the upload");
        assert_eq!(
            cs.milestones,
            vec![
                "startup-begin",
                "code-load",
                "skip",
                "pairing-select",
                "id-version",
                "pairing",
                "lens-params",
            ],
            "the vendor state order (0xf960's milestone logs)"
        );
        // the outcome: the DID fields and the lens block
        let (id, ver) =
            OdFrame::parse(&did_frame(), ENS_TYPE_DEVICE_ID).unwrap().device_id().unwrap();
        assert_eq!(out.device_id, id);
        assert_eq!(out.dsp_version, ver);
        let lens = lens_frame(ENS_TYPE_LENS);
        assert_eq!(out.lens_params.as_slice(), &lens[8..8 + ENS_LENS_PARAM_LEN]);
    }

    // ---- the wire, write by write ----
    let w = &m.writes;
    let mut i = 0usize;
    // state 1: PU type 1 ("Tx DSP Start")
    assert_eq!(w[i], pu_frame(ENS_TYPE_START).to_vec());
    i += 1;
    // state 2: the announce {size, crc}, then the RAW chunks
    let crc = crc32_ens(&ldr);
    assert_eq!(w[i], pu_codeload_announce(ldr.len() as u32, crc).to_vec());
    i += 1;
    for chunk in codeload_chunks(&ldr) {
        assert_eq!(w[i], chunk.to_vec(), "raw file chunk, no framing");
        i += 1;
    }
    // state 4: match ON -> type 0x25 (NeedHandleMatch = 1)
    assert_eq!(w[i], pu_frame(ENS_TYPE_MATCH_ON).to_vec());
    i += 1;
    // state 5: PU 0x8001
    assert_eq!(w[i], pu_frame(ENS_TYPE_ID_REQ).to_vec());
    i += 1;
    // state 6: keep-alive reply 0xFFE, then the four link replies
    assert_eq!(w[i], pu_frame(ENS_TYPE_KEEPALIVE_TX).to_vec());
    i += 1;
    assert_eq!(w[i], pu_frame_word(ENS_TYPE_LINK, ENS_LINK_REPLY_FOUND).to_vec());
    i += 1;
    assert_eq!(w[i], pu_frame_word(ENS_TYPE_LINK, ENS_LINK_REPLY_NOT_FOUND).to_vec());
    i += 1;
    assert_eq!(w[i], pu_frame_word(ENS_TYPE_LINK, ENS_LINK_REPLY_TIMEOUT).to_vec());
    i += 1;
    assert_eq!(w[i], pu_frame_word(ENS_TYPE_LINK, ENS_LINK_REPLY_OVER).to_vec());
    i += 1;
    // state 7: PU type 6 (the lens ACK)
    assert_eq!(w[i], pu_frame(ENS_TYPE_LENS).to_vec());
    i += 1;
    assert_eq!(i, w.len(), "no other writes: state 3 (skip) writes nothing");
}

#[test]
fn cold_start_without_handle_match_skips_state_6() {
    let ldr = ldr_bytes();
    let script: Vec<Scripted> = vec![
        reply(did_frame()),
        reply(od64(ENS_TYPE_CODELOAD)),
        reply(od(ENS_TYPE_CODELOAD_DONE, |b| {
            b[8..10].copy_from_slice(&ENS_CODELOAD_OK_WORD.to_le_bytes());
        })),
        reply(od64(ENS_TYPE_HANDLE_CFG)),
        reply(od64(ENS_TYPE_HANDLE_CFG)),
        reply(did_frame()),
        // no pairing frames: state 6 is skipped when match is off
        reply(lens_frame(ENS_TYPE_LENS)),
    ];
    let mut m = MockTransport::new(script);
    let mut cs = ColdStart::new(&mut m, &ldr);
    let out = cs.run(false).expect("match-off path");
    assert_eq!(cs.milestones[5], "pairing-skipped", "state 6 skipped");
    // state 4 sent type 0x26 (match OFF)
    let w = &m.writes;
    let idx = 2 + codeload_chunks(&ldr).count();
    assert_eq!(w[idx][4..6], 0x0026u16.to_le_bytes());
    // no keep-alive replies on the wire
    assert!(
        !w.iter().any(|f| f[4..6] == ENS_TYPE_KEEPALIVE_TX.to_le_bytes()),
        "no keep-alive replies"
    );
    let _ = out;
}

#[test]
fn cold_start_crc_verdict_failure_maps_the_vendor_code() {
    let ldr = ldr_bytes();
    let script: Vec<Scripted> = vec![
        reply(did_frame()),
        reply(od64(ENS_TYPE_CODELOAD)),
        // the verdict word is WRONG (the device computed a different CRC)
        reply(od(ENS_TYPE_CODELOAD_DONE, |b| {
            b[8..10].copy_from_slice(&0x0011u16.to_le_bytes());
        })),
    ];
    let mut m = MockTransport::new(script);
    let mut cs = ColdStart::new(&mut m, &ldr);
    let err = cs.run(true).expect_err("the bad verdict aborts");
    let msg = format!("{err}");
    assert!(msg.contains("code-load"), "state name: {msg}");
    assert!(msg.contains("-11"), "the vendor return code: {msg}");
    assert!(msg.contains("crc result is not correct"), "the vendor log: {msg}");
    // the full upload WAS sent before the verdict (announce + 3 chunks)
    assert_eq!(m.writes.len(), 2 + 3, "announce + 3 chunks, nothing after");
}

#[test]
fn hot_start_full_path() {
    let mut script: Vec<Scripted> = Vec::new();
    // three record cycles
    for k in 0..3u8 {
        script.push(reply(f0_frame()));
        script.push(reply(wire_record(k)));
    }
    // the finish: OD 0x10 (lens, 128 B), OD 0x9001 (DID)
    script.push(reply(lens_frame(ENS_TYPE_HOT_START)));
    script.push(reply(od(ENS_TYPE_DID, |b| {
        b[8..8 + ENS_DEVICE_ID_LEN].copy_from_slice(&[0xAB; ENS_DEVICE_ID_LEN]);
        b[0x28..0x28 + ENS_DEVICE_VER_LEN].copy_from_slice(b"HOTSTART");
    })));
    let mut m = MockTransport::new(script);
    let mut hs = HotStart::new(&mut m);
    let out = hs.run().expect("the hot path").expect("the stream was alive");
    assert_eq!(hs.milestones, vec!["probe-alive", "hot-start-ok"]);
    assert_eq!(out.device_id, [0xAB; ENS_DEVICE_ID_LEN]);
    assert_eq!(&out.dsp_version, b"HOTSTART");
    let lens = lens_frame(ENS_TYPE_HOT_START);
    assert_eq!(out.lens_params.as_slice(), &lens[8..8 + ENS_LENS_PARAM_LEN]);

    // the wire: exactly two type-7 ACKs (with the PU magic - the probe
    // builds {magic, type 7}, unlike the steady-state loop's zeroed
    // first ACK), then PU 0x10, PU 0x9001
    let ack = {
        let mut b = [0u8; RECORD_FRAME_LEN];
        b[..4].copy_from_slice(&MAGIC_H2D.to_le_bytes());
        b[4..6].copy_from_slice(&TYPE_RECORD_CMD.to_le_bytes());
        b.to_vec()
    };
    assert_eq!(m.writes[0], ack);
    assert_eq!(m.writes[1], ack);
    assert_eq!(m.writes[2][4..6], ENS_TYPE_HOT_START.to_le_bytes());
    assert_eq!(m.writes[3][4..6], ENS_TYPE_DID.to_le_bytes());
    assert_eq!(m.writes.len(), 4);
}

#[test]
fn hot_start_probe_silence_returns_none() {
    // no scripted reads at all: every probe read times out
    let mut m = MockTransport::new(vec![]);
    let mut hs = HotStart::new(&mut m);
    let out = hs.run().expect("silence is not an error");
    assert!(out.is_none(), "None = the cold start is needed");
    assert!(m.writes.is_empty(), "nothing is written on a silent probe");
}
