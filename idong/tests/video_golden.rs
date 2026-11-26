//! Video-plane golden tests - every byte literal copied from the turn-35
//! capture (`idong_capture.7z` run1, the xEyeThirdLens session; see
//! `research/idong100/agents/17-turn35/FINDINGS.md`).
//!
//! These tests are the P2.3-style pin of the video phase: the assembler,
//! the poll parser and the raster split must keep reproducing exactly what
//! the vendor stack put on the wire.

use idong::protocol::video::{
    split_channels, ProbeCommit, StreamPollState, UvcHeader, VideoAssembler, RASTER_STRIDE,
};

/// Capture offset 52.02 s, ISO transfer #1, payloads #1-#3: the 12-B UVC
/// payload headers (EOH|SCR|PTS, FID 0) and the first image bytes.
const PAYLOAD_1: [u8; 24] = [
    0x0c, 0x8c, 0x8e, 0xc1, 0xff, 0x69, 0x8b, 0x3c, 0x00, 0x6a, 0xce, 0x07, // header
    0xa5, 0x86, 0xa3, 0x74, 0xa3, 0x85, 0xa2, 0x74, 0xa2, 0x86, 0xa2, 0x75, // image
];
const PAYLOAD_2_HDR: [u8; 12] =
    [0x0c, 0x8c, 0x8e, 0xc1, 0xff, 0x69, 0x0e, 0x1e, 0x09, 0x6a, 0x2f, 0x00];
const PAYLOAD_3_HDR: [u8; 12] =
    [0x0c, 0x8c, 0x8e, 0xc1, 0xff, 0x69, 0x26, 0xd6, 0x14, 0x6a, 0xaf, 0x00];
/// Transfer #4 (capture offset 52.07 s) opens a new PTS (FID flips 0 -> 1):
/// first payload header of the NEXT frame.
const PAYLOAD_NEXT_FRAME_HDR: [u8; 12] =
    [0x0c, 0x8d, 0x8e, 0x0d, 0x1d, 0x6a, 0x40, 0x8e, 0x20, 0x6a, 0x2f, 0x01];

/// The cmd-0x0205 poll request, byte-for-byte (capture offset 0.02 s).
const POLL_REQ: [u8; 8] = [0x7f, 0x55, 0x05, 0x02, 0x00, 0x00, 0x00, 0x00];

/// The "new frame" reply (64 B, capture offset 0.02 s): frame counter
/// 0x00006d1b = 27,931.
fn poll_reply_new() -> Vec<u8> {
    let mut v = vec![
        0x7f, 0x55, 0x05, 0x02, 0x01, 0x00, // frame header
        0x00, 0x04, // status, code = 04 (new frame, 4-B arg)
        0x1b, 0x6d, 0x00, 0x00, // frame counter LE = 27931
        0x00, 0x6e, 0x23, 0xcb, 0x00, // aux
    ];
    v.extend(std::iter::repeat(0u8).take(26 + 6 - v.len())); // zeros to body+26
    v.extend_from_slice(&StreamPollState::TRAILER_NEW); // body+26..34
    v.extend(std::iter::repeat(0u8).take(64 - v.len())); // to 64 B
    assert_eq!(v.len(), 64);
    v
}

/// The "empty" reply (64 B, capture offset 0.08 s): code 00, the frame
/// counter is still present (0x00006f1b = 28,443).
fn poll_reply_empty() -> Vec<u8> {
    let mut v = vec![
        0x7f, 0x55, 0x05, 0x02, 0x01, 0x00, // frame header
        0x00, 0x00, // status, code = 00 (empty)
        0x1b, 0x6f, 0x00, 0x00, // frame counter LE = 28443
        0x00, 0x00, 0x00, 0x00, 0x00, // aux
    ];
    v.extend(std::iter::repeat(0u8).take(26 + 6 - v.len()));
    v.extend_from_slice(&StreamPollState::TRAILER_EMPTY);
    v.extend(std::iter::repeat(0u8).take(64 - v.len()));
    assert_eq!(v.len(), 64);
    v
}

#[test]
fn poll_request_bytes() {
    // The exact request the engine sends 30x per second while streaming.
    let req = idong::protocol::runtime::RtFrame::request(
        idong::protocol::runtime::CMD_STREAM_POLL,
        0,
        &[],
    )
    .unwrap();
    assert_eq!(req.encode(), POLL_REQ);
}

#[test]
fn poll_reply_parses() {
    let rep = idong::protocol::runtime::RtFrame::parse(&poll_reply_new()).unwrap();
    assert!(rep.reply);
    assert_eq!(rep.cmd, idong::protocol::runtime::CMD_STREAM_POLL);
    let st = StreamPollState::parse(&rep.body).unwrap();
    assert!(st.new_frame);
    assert_eq!(st.frame_no, 27_931);
    assert_eq!(st.trailer, StreamPollState::TRAILER_NEW);

    let rep = idong::protocol::runtime::RtFrame::parse(&poll_reply_empty()).unwrap();
    let st = StreamPollState::parse(&rep.body).unwrap();
    assert!(!st.new_frame);
    assert_eq!(st.frame_no, 28_443);
    assert_eq!(st.trailer, StreamPollState::TRAILER_EMPTY);
}

#[test]
fn poll_reply_roundtrip_via_mock() {
    use idong::mock::{reply, MockTransport};
    use idong::protocol::runtime::RuntimeSession;

    let mut m = MockTransport::new(vec![reply(poll_reply_new()), reply(poll_reply_empty())]);
    let mut s = RuntimeSession::new(&mut m);
    let a = s.poll_stream().unwrap();
    let b = s.poll_stream().unwrap();
    drop(s);
    assert!(a.new_frame && a.frame_no == 27_931);
    assert!(!b.new_frame && b.frame_no == 28_443);
    assert_eq!(m.writes, vec![POLL_REQ.to_vec(), POLL_REQ.to_vec()]);
}

#[test]
fn uvc_header_parses() {
    let (h, data) = UvcHeader::parse(&PAYLOAD_1).unwrap();
    assert_eq!(h.length, 12);
    assert_eq!(h.flags, 0x8c); // EOH|SCR|PTS, FID 0
    assert_eq!(h.pts, 1_778_368_910);
    assert_eq!(h.scr, [0x8b, 0x3c, 0x00, 0x6a, 0xce, 0x07]);
    assert_eq!(h.fid(), 0);
    assert!(!h.is_eof());
    assert_eq!(data, &[0xa5, 0x86, 0xa3, 0x74, 0xa3, 0x85, 0xa2, 0x74, 0xa2, 0x86, 0xa2, 0x75]);

    let (h2, _) = UvcHeader::parse(&PAYLOAD_NEXT_FRAME_HDR).unwrap();
    assert_eq!(h2.flags, 0x8d); // EOH|SCR|PTS + FID 1
    assert_eq!(h2.fid(), 1);
    assert_eq!(h2.pts, 1_780_288_910);
    assert_eq!(h2.pts.wrapping_sub(h.pts), 1_920_000); // exactly one frame
}

#[test]
fn uvc_header_rejects_malformed() {
    assert!(UvcHeader::parse(&[0x0c]).is_none());
    assert!(UvcHeader::parse(&[0x02, 0x8c, 0x00]).is_none()); // len < PTS+SCR needs
    assert!(UvcHeader::parse(&[0x0d, 0x8c, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).is_none());
}

#[test]
fn pts_advance_closes_frame() {
    // Payloads of one frame share the PTS; a new PTS closes it.
    let mut asm = VideoAssembler::new();
    let mut p1 = PAYLOAD_1.to_vec();
    p1.extend_from_slice(&[0x11; 64]); // pad to a fuller payload
    assert!(asm.push_payload(&p1).is_none());
    assert!(asm.push_payload(&PAYLOAD_2_HDR).is_none()); // same PTS -> same frame
    assert!(asm.push_payload(&PAYLOAD_3_HDR).is_none()); // same PTS -> same frame
    let mut next = PAYLOAD_NEXT_FRAME_HDR.to_vec();
    next.extend_from_slice(&[0x22; 8]);
    let closed = asm.push_payload(&next).unwrap();
    assert_eq!(closed.pts, 1_778_368_910);
    assert_eq!(closed.fid, 0);
    // 12 (payload1 data) + 64 + 0 + 0 image bytes accumulated
    assert_eq!(closed.data.len(), 12 + 64);
    assert_eq!(closed.data[..12], PAYLOAD_1[12..]);
    assert_eq!(asm.frames, 1);
    assert_eq!(asm.payloads, 4);
    assert_eq!(asm.errors, 0);
}

#[test]
fn eof_flag_closes_frame() {
    let mut asm = VideoAssembler::new();
    let mut p = PAYLOAD_1.to_vec();
    p.extend_from_slice(&[0x33; 4]);
    asm.push_payload(&p);
    // EOF-flagged header, same PTS -> still closes the frame.
    let mut eof = PAYLOAD_1[..12].to_vec();
    eof[1] = 0x8e; // EOH|SCR|PTS|EOF
    let closed = asm.push_payload(&eof).unwrap();
    assert_eq!(closed.data.len(), 12 + 4);
}

#[test]
fn transfer_walk_splits_payloads() {
    // One USBPcap ISO record = concatenated fixed-stride packets
    // (turn-35 capture: 2412 B each; last one short). The walker must
    // split at the stride and reassemble the image stream.
    let mut transfer = Vec::new();
    let mut p1 = PAYLOAD_1.to_vec();
    p1.extend(std::iter::repeat(0x55u8).take(2412 - 24)); // full-size payload 1
    transfer.extend_from_slice(&p1);
    let mut p2 = PAYLOAD_2_HDR.to_vec();
    p2.extend(std::iter::repeat(0x66u8).take(2400)); // full-size payload 2
    transfer.extend_from_slice(&p2);
    // payload 3: a short final packet (like the capture's 1236-B one)
    let mut p3 = PAYLOAD_3_HDR.to_vec();
    p3.extend(std::iter::repeat(0x77u8).take(100));
    transfer.extend_from_slice(&p3);

    let mut asm = VideoAssembler::new();
    let frames = asm.push_capture_transfer(&transfer, 2412);
    assert!(frames.is_empty()); // one PTS across all three payloads
    assert_eq!(asm.payloads, 3);
    assert_eq!(asm.errors, 0);

    // The next PTS closes the frame with exactly the three payloads' bytes.
    let mut next = PAYLOAD_NEXT_FRAME_HDR.to_vec();
    next.extend(std::iter::repeat(0x88u8).take(4));
    let closed = asm.push_payload(&next).unwrap();
    assert_eq!(closed.data.len(), 2400 + 2400 + 100);
    assert_eq!(&closed.data[..12], &PAYLOAD_1[12..]);
    assert!(closed.data[12..16].iter().all(|&v| v == 0x55));
}

#[test]
fn raster_split_interleaves_two_channels() {
    // The measured raster: 1280-B rows = 640 px x 2 interleaved channels.
    let mut frame_data = Vec::new();
    for px in 0..idong::protocol::video::RASTER_WIDTH {
        // even byte = channel A, odd byte = channel B
        frame_data.push((px % 251) as u8);
        frame_data.push(128u8);
    }
    assert_eq!(frame_data.len(), RASTER_STRIDE);
    let (a, b) = split_channels(&frame_data);
    assert_eq!(a.len(), RASTER_STRIDE / 2);
    assert_eq!(b.len(), RASTER_STRIDE / 2);
    assert_eq!(a[0], 0);
    assert_eq!(a[1], 1);
    assert!(b.iter().all(|&v| v == 128));
}

#[test]
fn probe_commit_golden() {
    // The host's probe: format 1, frame 1, 30 fps, frame size 614400,
    // payload size 0 (the host leaves it to the device).
    let probe = ProbeCommit::parse(&ProbeCommit::HOST_PROBE).unwrap();
    assert_eq!(probe.format_index, 1);
    assert_eq!(probe.frame_index, 1);
    assert_eq!(probe.frame_interval, 333_333);
    assert_eq!(probe.max_video_frame_size, 614_400);
    assert_eq!(probe.max_payload_transfer_size, 0);
    assert_eq!((probe.fps() + 0.5) as u32, 30);

    // The device's commit: identical but the payload size 962.
    let commit = ProbeCommit::parse(&ProbeCommit::DEVICE_COMMIT).unwrap();
    assert_eq!(commit, ProbeCommit::GOLDEN);
    assert_eq!(commit.max_payload_transfer_size, 962);
}

#[test]
fn device_clock_matches_measured_rate() {
    // 1,920,000 clock units per frame; the measured steady-state rate is
    // 8.0 fps -> the device clock is 15.36 MHz (a standard crystal).
    let frame_period_s = 1_920_000f64 / idong::protocol::video::DEVICE_CLOCK_HZ as f64;
    assert!((frame_period_s - 0.125).abs() < 1e-6);
    // and 8.0 fps is exactly 1 / 0.125.
    assert!((1.0 / frame_period_s - 8.0).abs() < 1e-3);
}
