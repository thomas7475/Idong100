//! FPGA bitstream-download replay (turn 50): the `xeyezHal_DownLoadFpga`
//! flow driven against a scripted device, asserting every frame the
//! driver puts on the wire byte-for-byte.
//!
//! The wire format was decoded statically from xeyezHal.dll v2.6.0.6
//! (`DownLoadFpga` impl @0x10040, `SendFpgaCode` @0xd570, the frame
//! builder @0xb2f0, the reply check inside `flow_open` @0xb380) - the
//! ~236 kB write no capture ever contained (every capture started
//! after `McuManager25::connect` had already run it). These tests pin
//! the Rust replay to that decode:
//!
//! 1. the happy path: START -> QUERY_CFG -> the 12-B announce {cmd
//!    0x0202, flag 4, size BE} -> the bit-reversed file in 64 KiB raw
//!    writes -> END -> DONE, and the milestone list;
//! 2. the bit-reversal: `fpga_bit_reverse` == the vendor's literal
//!    `(b & 1) << 7 | ...` chain, byte-for-byte over a full LFSR-ish
//!    buffer;
//! 3. a silent device at START (empty read) -> the vendor -10015
//!    short-reply code, with the cause preserved in the message;
//! 4. a wrong ack byte at QUERY_CFG -> the vendor -10014 frame code;
//! 5. a transport failure on a chunk write -> the vendor -10012
//!    chunk-tx code;
//! 6. an empty image -> the vendor -10013 fopen analog.

use idong::mock::{reply, timeout, MockTransport};
use idong::protocol::fpga::{
    fpga_bit_reverse, fpga_send_announce, fpga_step_frame, FpgaDownload, FPGA_CHUNK,
};

/// The 8-B reply frame the HAL accepts: magic 0x557f + ack 0x01 @+4
/// (bytes 2/3 echo the command, 5..8 kept zero here).
fn ok_reply(cmd: u16) -> Vec<u8> {
    let mut b = fpga_step_frame(cmd, 0);
    b[4] = 0x01;
    b.to_vec()
}

/// The exact vendor bit-reversal chain (`SendFpgaCode` @0xd605-0xd6dd):
/// `(b & 1) << 7 | (b & 2) << 5 | (b & 4) << 3 | (b & 8) << 1 |
/// (b & 0x10) >> 1 | (b & 0x20) >> 3 | (b & 0x40) >> 5 | (b & 0x80) >> 7`.
fn vendor_bit_reverse_ref(b: u8) -> u8 {
    let mut out = 0u8;
    out |= (b & 1) << 7;
    out |= (b & 2) << 5;
    out |= (b & 4) << 3;
    out |= (b & 8) << 1;
    out |= (b & 0x10) >> 1;
    out |= (b & 0x20) >> 3;
    out |= (b & 0x40) >> 5;
    out |= (b & 0x80) >> 7;
    out
}

#[test]
fn happy_path_writes_the_exact_frames() {
    // 130 B image: two 64 B writes + a 2 B tail at 64 KiB chunking it
    // is ONE chunk - the chunking test is the big-image case below.
    let size = 130usize;
    let image: Vec<u8> = (0..size as u8).collect();
    let script = vec![
        reply(ok_reply(0x0002)),
        reply(ok_reply(0x0102)),
        reply(ok_reply(0x0202)),
        reply(ok_reply(0x0302)),
        reply(ok_reply(0x0102)),
    ];
    let mut m = MockTransport::new(script);
    let mut dl = FpgaDownload::new(&mut m);
    let rep = dl.run(&image).expect("the download must succeed");
    assert_eq!(rep.size, size);
    assert_eq!(rep.chunks, 1);
    assert_eq!(
        dl.milestones,
        vec![
            "fpga-download-start",
            "fpga-query-cfg",
            "fpga-send-announce",
            "fpga-code-sent",
            "fpga-download-end",
            "fpga-download-done",
        ]
    );
    // every frame byte-for-byte:
    assert_eq!(m.writes[0], fpga_step_frame(0x0002, 0).to_vec(), "START");
    assert_eq!(m.writes[1], fpga_step_frame(0x0102, 0).to_vec(), "QUERY_CFG");
    assert_eq!(m.writes[2], fpga_send_announce(size).to_vec(), "announce");
    // the file goes out bit-reversed, in one raw write for this size
    assert_eq!(m.writes[3], fpga_bit_reverse(&image), "the reversed image");
    assert_eq!(m.writes[4], fpga_step_frame(0x0302, 0).to_vec(), "END");
    assert_eq!(m.writes[5], fpga_step_frame(0x0102, 1).to_vec(), "DONE");
    assert_eq!(m.writes.len(), 6, "nothing else on the wire");
    // the QUERY_CFG reply travels to the report raw
    let mut expected = [0u8; 8];
    expected.copy_from_slice(&ok_reply(0x0102));
    assert_eq!(rep.query_reply, expected);
}

#[test]
fn big_image_chunks_at_64_kib() {
    // 64 KiB + 100 B: two chunks, the second only 100 B - and the
    // announce still says the FULL size (the device knows it).
    let size = FPGA_CHUNK + 100;
    let image: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
    let script = vec![
        reply(ok_reply(0x0002)),
        reply(ok_reply(0x0102)),
        reply(ok_reply(0x0202)),
        reply(ok_reply(0x0302)),
        reply(ok_reply(0x0102)),
    ];
    let mut m = MockTransport::new(script);
    let mut dl = FpgaDownload::new(&mut m);
    let rep = dl.run(&image).expect("chunked download must succeed");
    assert_eq!(rep.chunks, 2);
    assert_eq!(m.writes.len(), 7); // 5 frames + 2 chunks
    assert_eq!(m.writes[3].len(), FPGA_CHUNK);
    assert_eq!(m.writes[4].len(), 100);
    assert_eq!(m.writes[3], fpga_bit_reverse(&image[..FPGA_CHUNK]));
    assert_eq!(m.writes[4], fpga_bit_reverse(&image[FPGA_CHUNK..]));
}

#[test]
fn bit_reverse_matches_the_vendor_chain() {
    // every u8 value, the Rust port against the literal vendor chain
    for b in 0..=255u8 {
        assert_eq!(
            fpga_bit_reverse(&[b])[0],
            vendor_bit_reverse_ref(b),
            "byte {b:#04x} diverges from the vendor chain"
        );
    }
    // involution: reverse twice == identity
    let data: Vec<u8> = (0..=255u8).collect();
    let once = fpga_bit_reverse(&data);
    assert_eq!(fpga_bit_reverse(&once), data);
}

#[test]
fn silent_start_maps_to_vendor_reply_code_with_cause() {
    let image = vec![1u8; 64];
    let mut m = MockTransport::new(vec![timeout()]);
    let mut dl = FpgaDownload::new(&mut m);
    let err = dl.run(&image).unwrap_err().to_string();
    assert!(err.contains("vendor -10015"), "{err}");
    // the T13 lesson: the cause (the injected timeout) stays visible
    assert!(err.contains("read timeout"), "{err}");
    // and nothing else was written after the failed START
    assert_eq!(m.writes.len(), 1);
}

#[test]
fn wrong_ack_maps_to_vendor_frame_code() {
    let image = vec![1u8; 64];
    // START acks, QUERY_CFG acks with a WRONG byte @+4 (0x00, not 0x01)
    let mut bad = fpga_step_frame(0x0102, 0);
    bad[4] = 0x00;
    let mut m = MockTransport::new(vec![reply(ok_reply(0x0002)), reply(bad.to_vec())]);
    let mut dl = FpgaDownload::new(&mut m);
    let err = dl.run(&image).unwrap_err().to_string();
    assert!(err.contains("vendor -10014"), "{err}");
    assert!(err.contains("QUERY_CFG"), "{err}");
    assert_eq!(m.writes.len(), 2, "the flow stops at the bad step");
}

#[test]
fn short_reply_maps_to_vendor_reply_code() {
    let image = vec![1u8; 64];
    // a 4-B bulk record: n != 8 -> the vendor's -10015 branch
    let mut m =
        MockTransport::new(vec![reply(ok_reply(0x0002)), reply(vec![0x7f, 0x55, 0x02, 0x01])]);
    let mut dl = FpgaDownload::new(&mut m);
    let err = dl.run(&image).unwrap_err().to_string();
    assert!(err.contains("vendor -10015"), "{err}");
}

#[test]
fn chunk_write_failure_maps_to_vendor_chunk_code() {
    let image = vec![7u8; FPGA_CHUNK + 10]; // two chunks, the second fails
                                            // A transport that fails the FIFTH write: START, QUERY, announce
                                            // and chunk 1/2 succeed, chunk 2/2 fails - the vendor's -10012.
    struct FailNthWrite {
        inner: MockTransport,
        n: usize,
    }
    impl idong::transport::Transport for FailNthWrite {
        fn write(&mut self, buf: &[u8]) -> idong::error::Result<()> {
            self.n += 1;
            if self.n == 5 {
                return Err(idong::error::IdongError::Usb("mock: write fail".into()));
            }
            self.inner.write(buf)
        }
        fn read(&mut self, max: usize) -> idong::error::Result<Vec<u8>> {
            self.inner.read(max)
        }
    }
    let mut t = FailNthWrite {
        inner: MockTransport::new(vec![
            reply(ok_reply(0x0002)),
            reply(ok_reply(0x0102)),
            reply(ok_reply(0x0202)),
        ]),
        n: 0,
    };
    let mut dl = FpgaDownload::new(&mut t);
    let err = dl.run(&image).unwrap_err().to_string();
    assert!(err.contains("vendor -10012"), "{err}");
    assert!(err.contains("chunk 2/2"), "{err}");
}

#[test]
fn empty_image_maps_to_vendor_fopen_code() {
    let mut m = MockTransport::new(vec![]);
    let mut dl = FpgaDownload::new(&mut m);
    let err = dl.run(&[]).unwrap_err().to_string();
    assert!(err.contains("vendor -10013"), "{err}");
    assert!(m.writes.is_empty(), "nothing may be written for an empty image");
}
