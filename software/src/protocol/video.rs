//! Video plane - the UVC streaming interface (turn 35).
//!
//! Decoded from the turn-35 capture (`idong_capture.7z` run1, the
//! `xEyeThirdLens.exe` session - the first capture to reach the video
//! phase; 82.8 MB of isochronous IN on endpoint 0x85 over 120 s while the
//! engine polled cmd `0x0205` on the vendor interface at ~30 Hz).
//!
//! # What the capture shows
//!
//! * The runtime composite (0471:2500) negotiates a **standard UVC
//!   probe/commit** on its VS interface: `bFormatIndex=1`,
//!   `bFrameIndex=1`, frame interval 333333 (30 fps nominal),
//!   `dwMaxVideoFrameSize = 614400` (= 640×480×2). See [`ProbeCommit`].
//! * Video then flows as **isochronous IN on EP 0x85**. No `SET_INTERFACE`
//!   alt > 0 was observed on the wire - the vendor's composite presents
//!   the streaming alt implicitly.
//! * Each ISO transfer carries several UVC payloads; each payload is
//!   **[12-byte UVC payload header][<= 2400 B image data]**. The header
//!   form is `EOH|SCR|PTS` (flags `0x8c`/`0x8d`, FID toggling):
//!   `[len][flags][pts LE u32][scr 6 B]`.
//! * **PTS is constant across all payloads of one frame** and advances by
//!   exactly 1,920,000 per frame. With the device clock at 15.36 MHz that
//!   is 125 ms - the observed steady-state frame rate is **8.0 fps**
//!   (the negotiated 30 fps is an upper bound only).
//! * Assembled frame bytes form a **640-pixel, byte-interleaved
//!   two-channel raster** (row stride 1280 B): channel A (even bytes)
//!   carries the scene image, channel B (odd bytes) a second,
//!   low-contrast channel. Verified statistically: row-to-row correlation
//!   0.998 (A) / 0.97 (B) at width 640 per plane, and the two planes are
//!   mutually uncorrelated (|r| < 0.15 at any pixel shift - they are two
//!   different images, not hi/lo bytes of one pixel).
//! * Frame boundaries: PTS advance (and FID toggle). The vendor firmware
//!   toggles FID only at ISO-transfer boundaries, so frames span 2-3
//!   transfers; [`VideoAssembler`] closes on PTS like the vendor's
//!   DirectShow filter does.
//! * **EOF carries the frame tail (turn-44 bench fact).** The Linux
//!   T9 run (turn 44) showed the firmware flags the final payload of
//!   EVERY frame with EOF from the first frame on: 255 full payloads
//!   + one short EOF payload of the same PTS = exactly 614400 B.
//!   [`VideoAssembler`] therefore completes the frame ON the EOF
//!   payload, with that payload's data included (the capture-era
//!   close-before-append order split every frame into a 612000-B body
//!   + a 2400-B tail under one PTS).
//!
//! # Transport seam
//!
//! The video plane does NOT run over the vendor bulk interface (EP
//! 0x02/0x82) - it is a separate isochronous stream. [`VideoSource`]
//! abstracts it: one `read_transfer` returns one raw ISO transfer (the
//! concatenated UVC payloads). The mock replays captures; the Linux
//! implementation (transport::usb, V4L2 `VIDIOC_DQBUF` on the UVC device
//! or usbfs ISO URBs on interface alt 1) plugs in behind it.

use crate::error::{IdongError, ProtocolError, Result};

/// UVC payload header flags (payload byte 1).
pub mod uvc_flags {
    pub const FID: u8 = 0x01;
    pub const EOF: u8 = 0x02;
    pub const PTS: u8 = 0x04;
    pub const SCR: u8 = 0x08;
    pub const ERR: u8 = 0x40;
    pub const EOH: u8 = 0x80;
}

/// Raster width in pixels (per channel) - `dwMaxVideoFrameSize` 614400
/// = 640×480×2 and the measured 1280-byte row stride.
pub const RASTER_WIDTH: usize = 640;
/// Row stride in bytes: 640 px × 2 interleaved channels.
pub const RASTER_STRIDE: usize = 1280;
/// Image bytes per UVC payload (max observed; a frame's final payload may
/// be shorter - the first transfer of the capture ends with a 1236-B one).
pub const PAYLOAD_DATA: usize = 2400;
/// PTS advance per video frame (device clock units at 15.36 MHz → 125 ms;
/// the observed steady-state frame rate is 8.0 fps).
pub const PTS_PER_FRAME: u32 = 1_920_000;
/// Device clock the PTS counts in (deduced: 1,920,000 units per frame at
/// the measured 8.0 fps → 15.36 MHz, a standard crystal).
pub const DEVICE_CLOCK_HZ: u32 = 15_360_000;
/// Nominal (negotiated) frame interval in 100 ns units - 333333 = 30 fps.
pub const FRAME_INTERVAL_100NS: u32 = 333_333;

/// One parsed UVC payload header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UvcHeader {
    /// `bHeaderLength` (payload byte 0): header bytes before image data.
    pub length: u8,
    /// Flag byte (payload byte 1): FID/EOF/PTS/SCR/EOH/ERR bits.
    pub flags: u8,
    /// Presentation timestamp (bytes 2..6, LE) - constant within a frame.
    pub pts: u32,
    /// Source clock reference (bytes 6..12): 4-B STC + 2-B SOF counter.
    pub scr: [u8; 6],
}

impl UvcHeader {
    /// Parse the header at the front of `p`, returning it with the image
    /// data slice after it. `None` = malformed (length field vs flags).
    pub fn parse(p: &[u8]) -> Option<(UvcHeader, &[u8])> {
        if p.len() < 2 {
            return None;
        }
        let length = p[0];
        let flags = p[1];
        if length < 2 || length as usize > p.len() {
            return None;
        }
        let mut h = UvcHeader { length, flags, pts: 0, scr: [0; 6] };
        let mut off = 2usize;
        if flags & uvc_flags::PTS != 0 {
            if off + 4 > p.len() {
                return None;
            }
            h.pts = u32::from_le_bytes([p[off], p[off + 1], p[off + 2], p[off + 3]]);
            off += 4;
        }
        if flags & uvc_flags::SCR != 0 {
            if off + 6 > p.len() {
                return None;
            }
            h.scr.copy_from_slice(&p[off..off + 6]);
            off += 6;
        }
        // The vendor sends exactly 12-B headers (PTS+SCR+2); reject any
        // other layout rather than guessing past it.
        if off != length as usize {
            return None;
        }
        Some((h, &p[length as usize..]))
    }

    /// Frame-ID bit - toggles at frame starts (vendor: at transfer
    /// boundaries; frames span 2-3 transfers).
    pub fn fid(&self) -> u8 {
        self.flags & uvc_flags::FID
    }

    /// End-of-frame flag (the firmware starts setting this only late in a
    /// session; assembly relies on PTS advances).
    pub fn is_eof(&self) -> bool {
        self.flags & uvc_flags::EOF != 0
    }

    /// Payload error flag (close the frame, drop its bytes).
    pub fn is_error(&self) -> bool {
        self.flags & uvc_flags::ERR != 0
    }
}

/// One assembled video frame (UVC headers stripped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    /// Frame PTS (device clock units, see [`DEVICE_CLOCK_HZ`]).
    pub pts: u32,
    /// FID bit of the payloads that built this frame.
    pub fid: u8,
    /// Raw frame bytes - `RASTER_STRIDE`-strided, two interleaved channels.
    pub data: Vec<u8>,
}

impl VideoFrame {
    /// Number of complete raster rows (a partial tail row may exist
    /// between frames; the vendor streams through it - kept verbatim).
    pub fn rows(&self) -> usize {
        self.data.len() / RASTER_STRIDE
    }

    /// Deinterleave into `(channel_a, channel_b)` planes: `data.len()/2`
    /// bytes each, `RASTER_WIDTH` pixels per row. Channel A = even bytes
    /// (the scene image), channel B = odd bytes (the second channel).
    pub fn split_channels(&self) -> (Vec<u8>, Vec<u8>) {
        split_channels(&self.data)
    }
}

/// Deinterleave a two-channel raster into its planes.
pub fn split_channels(data: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut a = Vec::with_capacity(data.len() / 2);
    let mut b = Vec::with_capacity(data.len() / 2);
    for pair in data.chunks_exact(2) {
        a.push(pair[0]);
        b.push(pair[1]);
    }
    (a, b)
}

/// Basic statistics of one raster plane (turn 38) - the PASS/FAIL
/// summary of the P2.5 stream test: a real scene plane has contrast
/// (`stddev > 0`); an all-flat plane means no image (uninitialized
/// sensor, blank stream). Population stddev, `f64` accumulation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneStats {
    pub mean: f64,
    pub min: u8,
    pub max: u8,
    pub stddev: f64,
    /// Sample count of the plane - the merge weight. Turn 44: the
    /// bench summary averaged per-frame means pair-wise, so a 1200-B
    /// tail frame outweighed its 612000-B sibling; weighting by `n`
    /// keeps pooled summaries honest across mixed-size frames.
    pub n: u64,
}

impl PlaneStats {
    /// True when the plane carries any contrast at all (not flat).
    pub fn has_contrast(&self) -> bool {
        self.min != self.max
    }
}

/// Compute [`PlaneStats`] for a plane; an empty plane yields all zeros
/// (defined, never NaN).
pub fn plane_stats(p: &[u8]) -> PlaneStats {
    if p.is_empty() {
        return PlaneStats { mean: 0.0, min: 0, max: 0, stddev: 0.0, n: 0 };
    }
    let mut min = u8::MAX;
    let mut max = u8::MIN;
    let mut sum = 0u64;
    let mut sq = 0u64;
    for &v in p {
        min = min.min(v);
        max = max.max(v);
        sum += v as u64;
        sq += (v as u64) * (v as u64);
    }
    let n = p.len() as f64;
    let mean = sum as f64 / n;
    let var = (sq as f64 / n - mean * mean).max(0.0);
    PlaneStats { mean, min, max, stddev: var.sqrt(), n: p.len() as u64 }
}

/// Count the frames the DEVICE produced but the host loop never saw,
/// from consecutive delivered frame markers - the P2.6 drop accounting
/// (turn 45). Shared by both backends' summary lines.
///
/// * V4L2: `markers` are the driver's per-buffer sequence numbers
///   (uvcvideo counts every completed buffer). A gap of `k` between
///   consecutive delivered seqs means `k - 1` frames were produced but
///   our loop was too slow to see them (host-side drop, not device
///   stall). Wraparound (u32) counts as no gap.
/// * ISO: `markers` are the frame PTS values in device-clock units; a
///   gap larger than [`PTS_PER_FRAME`] means whole frames never reached
///   the ring. Converting: `lost = gap / PTS_PER_FRAME - 1`.
///
/// A decreasing marker (device reboot, recovery re-arm) resets the
/// accounting: the count restarts from there, no negative drops.
pub fn count_dropped_frames(prev: u32, next: u32, per_step: u32, marker_is_pts: bool) -> u64 {
    if next <= prev {
        return 0; // reset/reboot: no drop can be inferred
    }
    let gap = next.wrapping_sub(prev);
    if marker_is_pts {
        // PTS in device-clock units: whole frame periods lost.
        gap.checked_div(per_step).map(|whole| whole.saturating_sub(1) as u64).unwrap_or(0)
    } else {
        // seq counters: consecutive integers, one frame per count.
        gap.checked_sub(1).map(|g| g as u64).unwrap_or(0)
    }
}

/// Incremental frame assembler over UVC payloads. A frame closes when
/// the PTS advances (the vendor's own boundary rule) or on an
/// EOF-flagged payload - and an EOF payload's data belongs to the
/// frame it completes (UVC semantics; the turn-44 bench proved the
/// firmware sets EOF on every frame's final payload, carrying the
/// frame's tail bytes). An error-flagged payload closes the frame but
/// its own bytes are dropped with the payload.
///
/// UVC payload headers carry **no length field**: one payload = one ISO
/// packet, delivered whole by [`VideoSource::read_payload`] (the Linux
/// backend returns one payload per `VIDIOC_DQBUF` / ISO packet). The
/// capture-replay path uses [`VideoAssembler::push_capture_transfer`]
/// with the observed 2412-B packet stride.
#[derive(Debug, Default)]
pub struct VideoAssembler {
    pending: Vec<u8>,
    pending_pts: Option<u32>,
    pending_fid: u8,
    /// Frames emitted.
    pub frames: u64,
    /// Payloads consumed.
    pub payloads: u64,
    /// Payloads rejected (bad header) - the whole frame is lost.
    pub errors: u64,
}

impl VideoAssembler {
    pub fn new() -> VideoAssembler {
        VideoAssembler::default()
    }

    /// Feed one UVC payload (header + data). Returns a completed frame
    /// when this payload closes one (PTS advance, EOF or error).
    pub fn push_payload(&mut self, payload: &[u8]) -> Option<VideoFrame> {
        let (hdr, data) = match UvcHeader::parse(payload) {
            Some(x) => x,
            None => {
                self.errors += 1;
                return None;
            }
        };
        self.payloads += 1;

        let pts_change = matches!(self.pending_pts, Some(prev) if hdr.pts != prev);

        // A PTS advance closes the pending frame BEFORE this payload:
        // the new PTS starts the new frame (the vendor boundary rule).
        let mut out =
            if pts_change && !self.pending.is_empty() { Some(self.take_pending()) } else { None };
        if self.pending_pts.is_none() || pts_change {
            self.pending_pts = Some(hdr.pts);
            self.pending_fid = hdr.fid();
        }
        // An EOF payload COMPLETES the frame: its data belongs to the
        // closing frame. The turn-44 bench run proved the firmware
        // flags the final payload of every frame this way - the old
        // close-before-append order split each frame into a 612000-B
        // body + a 2400-B tail under the SAME PTS (255 + 1 payloads =
        // exactly 614400 B). An error payload's own bytes are dropped
        // with the payload; the frame still closes.
        if !hdr.is_error() {
            self.pending.extend_from_slice(data);
        }
        if (hdr.is_eof() || hdr.is_error()) && !self.pending.is_empty() {
            out = out.or_else(|| Some(self.take_pending()));
        }
        out
    }

    /// Emit the pending accumulation as a frame (the single close path).
    fn take_pending(&mut self) -> VideoFrame {
        let f = VideoFrame {
            pts: self.pending_pts.unwrap_or(0),
            fid: self.pending_fid,
            data: std::mem::take(&mut self.pending),
        };
        self.frames += 1;
        f
    }

    /// Capture-replay path: split one USBPcap ISO record (the
    /// concatenated payloads of one URB) at the observed fixed packet
    /// stride (turn-35 capture: 2412 B = 12-B header + 2400 B data;
    /// the final packet of the record is short). Each split is fed to
    /// [`Self::push_payload`]; the function returns every frame that
    /// completes.
    pub fn push_capture_transfer(&mut self, transfer: &[u8], stride: usize) -> Vec<VideoFrame> {
        let mut out = Vec::new();
        if stride == 0 {
            self.errors += 1;
            return out;
        }
        for packet in transfer.chunks(stride) {
            if let Some(f) = self.push_payload(packet) {
                out.push(f);
            }
        }
        out
    }
}

/// A source of UVC payloads (one read = one ISO packet: header + data).
/// Implemented by the Linux video backend (one `VIDIOC_DQBUF` / ISO
/// packet per call); mocked in tests from captures.
pub trait VideoSource {
    /// Read one UVC payload; empty vec = no payload within the budget.
    fn read_payload(&mut self) -> Result<Vec<u8>>;
}

impl VideoSource for Box<dyn VideoSource + Send> {
    fn read_payload(&mut self) -> Result<Vec<u8>> {
        (**self).read_payload()
    }
}

/// UVC probe/commit control payload (the 26-B UVC 1.0 form observed;
/// multi-byte fields little-endian, as UVC defines).
///
/// Wire order after the 8-B setup stage:
/// `[bmHint 2][bFormatIndex][bFrameIndex][bFrameInterval 4]
/// [wKeyFrameRate 2][wPFrameRate 2][wCompQuality 2][wCompWindowSize 2]
/// [wDelay 2][dwMaxVideoFrameSize 4][dwMaxPayloadTransferSize 4]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeCommit {
    pub bm_hint: u16,
    pub format_index: u8,
    pub frame_index: u8,
    pub frame_interval: u32,
    pub max_video_frame_size: u32,
    pub max_payload_transfer_size: u32,
}

impl ProbeCommit {
    /// The vendor's negotiated values, golden from the turn-35 capture
    /// (the commit body): format 1, frame 1, interval 333333 (30 fps),
    /// frame size 614400 (640×480×2), the device's payload size 962
    /// (the firmware actually streams 2400-B sub-payloads - the value
    /// is carried verbatim, not trusted).
    pub const GOLDEN: ProbeCommit = ProbeCommit {
        bm_hint: 0,
        format_index: 1,
        frame_index: 1,
        frame_interval: FRAME_INTERVAL_100NS,
        max_video_frame_size: 614_400,
        max_payload_transfer_size: 962,
    };

    /// The host's probe SET_CUR body as captured (26 B, after the 8-B
    /// setup stage): the host proposes `dwMaxPayloadTransferSize = 0`.
    pub const HOST_PROBE: [u8; 26] = [
        0x00, 0x00, // bmHint
        0x01, // bFormatIndex
        0x01, // bFrameIndex
        0x15, 0x16, 0x05, 0x00, // bFrameInterval = 333333 (30 fps)
        0x00, 0x00, // wKeyFrameRate
        0x00, 0x00, // wPFrameRate
        0x00, 0x00, // wCompQuality
        0x00, 0x00, // wCompWindowSize
        0x00, 0x00, // wDelay
        0x00, 0x60, 0x09, 0x00, // dwMaxVideoFrameSize = 614400
        0x00, 0x00, 0x00, 0x00, // dwMaxPayloadTransferSize = 0 (probe)
    ];

    /// The device's commit SET_CUR body as captured (26 B): identical
    /// except `dwMaxPayloadTransferSize = 0x000003c2 = 962`.
    pub const DEVICE_COMMIT: [u8; 26] = [
        0x00, 0x00, 0x01, 0x01, 0x15, 0x16, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x60, 0x09, 0x00, 0xc2, 0x03, 0x00, 0x00,
    ];

    /// Parse a probe/commit body (26 B); `None` = too short.
    pub fn parse(p: &[u8]) -> Option<ProbeCommit> {
        if p.len() < 26 {
            return None;
        }
        Some(ProbeCommit {
            bm_hint: u16::from_le_bytes([p[0], p[1]]),
            format_index: p[2],
            frame_index: p[3],
            frame_interval: u32::from_le_bytes([p[4], p[5], p[6], p[7]]),
            max_video_frame_size: u32::from_le_bytes([p[18], p[19], p[20], p[21]]),
            max_payload_transfer_size: u32::from_le_bytes([p[22], p[23], p[24], p[25]]),
        })
    }

    /// Frame rate implied by the interval, in fps.
    pub fn fps(&self) -> f64 {
        10_000_000.0 / self.frame_interval as f64
    }
}

/// The stream state carried by the cmd `0x0205` poll reply (see
/// [`crate::protocol::runtime::RuntimeSession::poll_stream`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamPollState {
    /// A new video frame is available (reply code 0x04).
    pub new_frame: bool,
    /// Video frame counter (LE u32 at reply body +2) - advances by 1 per
    /// new frame while streaming (~30 Hz in the capture).
    pub frame_no: u32,
    /// The aux word at reply body +6 (BE-ish counter, meaning open).
    pub aux: [u8; 4],
    /// The fixed 8-B status trailer at reply body +26.
    pub trailer: [u8; 8],
}

impl StreamPollState {
    /// Trailer of a "new frame" reply (capture offsets 32..40).
    pub const TRAILER_NEW: [u8; 8] = [0xff, 0xff, 0xff, 0x00, 0x1f, 0x00, 0x05, 0x00];
    /// Trailer of an "empty" reply.
    pub const TRAILER_EMPTY: [u8; 8] = [0xff, 0xff, 0xff, 0x00, 0x1f, 0x00, 0x04, 0x00];

    /// Parse from a cmd-0x0205 reply body (58 B after the 6-B header).
    pub fn parse(body: &[u8]) -> Result<StreamPollState> {
        if body.len() < 34 {
            return Err(IdongError::Protocol(ProtocolError::Short { need: 34, have: body.len() }));
        }
        let new_frame = body[1] == 0x04;
        let frame_no = u32::from_le_bytes([body[2], body[3], body[4], body[5]]);
        let mut aux = [0u8; 4];
        aux.copy_from_slice(&body[6..10]);
        let mut trailer = [0u8; 8];
        trailer.copy_from_slice(&body[26..34]);
        Ok(StreamPollState { new_frame, frame_no, aux, trailer })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_stats_constant_plane_is_flat() {
        let s = plane_stats(&[0x7f; 1000]);
        assert_eq!(s.mean, 127.0);
        assert_eq!((s.min, s.max), (0x7f, 0x7f));
        assert_eq!(s.stddev, 0.0);
        assert!(!s.has_contrast());
        assert_eq!(s.n, 1000);
    }

    #[test]
    fn plane_stats_two_value_plane() {
        // 2 x 0x00 + 2 x 0xff: mean 127.5, population stddev 127.5
        let s = plane_stats(&[0x00, 0x00, 0xff, 0xff]);
        assert_eq!(s.mean, 127.5);
        assert_eq!((s.min, s.max), (0x00, 0xff));
        assert!((s.stddev - 127.5).abs() < 1e-9);
        assert!(s.has_contrast());
        assert_eq!(s.n, 4);
    }

    #[test]
    fn plane_stats_empty_plane_is_defined() {
        let s = plane_stats(&[]);
        assert_eq!((s.mean, s.min, s.max, s.stddev, s.n), (0.0, 0, 0, 0.0, 0));
    }

    #[test]
    fn drop_accounting_v4l2_seq_gaps() {
        use super::count_dropped_frames;
        // consecutive deliveries: no drop
        assert_eq!(count_dropped_frames(0, 1, 1, false), 0);
        assert_eq!(count_dropped_frames(5, 6, 1, false), 0);
        // the turn-44 run6 shape: +3 seq steps = 2 frames missed per step
        assert_eq!(count_dropped_frames(4, 7, 1, false), 2);
        assert_eq!(count_dropped_frames(100, 106, 1, false), 5);
        // wraparound: u32 seq wrapping is not a drop
        assert_eq!(count_dropped_frames(u32::MAX - 2, 1, 1, false), 0);
        // a restart/reset (seq going backwards) resets accounting
        assert_eq!(count_dropped_frames(50, 3, 1, false), 0);
    }

    #[test]
    fn drop_accounting_iso_pts_gaps() {
        use super::count_dropped_frames;
        let per = PTS_PER_FRAME; // 1_920_000 device-clock units / frame
                                 // one frame period exactly: normal cadence, no drop
        assert_eq!(count_dropped_frames(1000 * per, 1001 * per, per, true), 0);
        // the turn-44 run4 cadence: 1/5/5/4/4... frame-period deltas -
        // the 5-period steps each hid 4 device frames that our starved
        // ring never delivered (that is the whole P2.6 finding)
        assert_eq!(count_dropped_frames(0, 5 * per, per, true), 4);
        assert_eq!(count_dropped_frames(0, 4 * per, per, true), 3);
        assert_eq!(count_dropped_frames(0, 1 * per, per, true), 0);
        // a sub-frame PTS jitter is not a drop either
        assert_eq!(count_dropped_frames(0, per - 1, per, true), 0);
        // PTS going backwards (assembler reset / recovery): no drop
        assert_eq!(count_dropped_frames(9 * per, per, per, true), 0);
    }

    #[test]
    fn drop_accounting_mixed_run6_scenario() {
        use super::count_dropped_frames;
        // run6's delivered seq list: 0..4, then 7,10,13,... - simulate
        // the sum the example now computes inline
        let seqs = [0u32, 1, 2, 3, 4, 7, 10, 13, 16, 19, 22, 25, 28, 30, 33];
        let drops: u64 = seqs.windows(2).map(|w| count_dropped_frames(w[0], w[1], 1, false)).sum();
        // 4->7..33 steps: nine +2 gaps and one +1 gap (28->30) = 19
        assert_eq!(drops, 19);
    }

    #[test]
    fn plane_stats_half_ramp_known_values() {
        // 0..=15: mean 7.5, mean of squares 1240/16 = 77.5
        // -> stddev sqrt(77.5 - 7.5^2) = sqrt(21.25)
        let p: Vec<u8> = (0u16..16).map(|v| v as u8).collect();
        let s = plane_stats(&p);
        assert_eq!(s.mean, 7.5);
        assert_eq!((s.min, s.max), (0, 15));
        let want = 21.25f64.sqrt();
        assert!((s.stddev - want).abs() < 1e-9, "stddev {} vs {want}", s.stddev);
        assert_eq!(s.n, 16);
    }

    /// A 12-B UVC payload header (EOH|SCR|PTS base) + data, at a given
    /// PTS with extra flag bits - the bench header form.
    fn bench_payload(pts: u32, extra_flags: u8, data: &[u8]) -> Vec<u8> {
        let mut p = vec![12u8, 0x8c | extra_flags];
        p.extend_from_slice(&pts.to_le_bytes());
        p.extend_from_slice(&[0u8; 6]); // SCR
        p.extend_from_slice(data);
        p
    }

    #[test]
    fn eof_payload_carries_the_frame_tail() {
        // The turn-44 T9 bench shape (video_run1): one PTS group = 255
        // full 2400-B payloads + ONE short payload of the SAME PTS
        // flagged EOF - 256 x 2400 = exactly 614400 B. The EOF
        // payload's data belongs to the closing frame; the old
        // close-before-append order split every frame into a
        // 612000-B body + a 2400-B tail under one PTS.
        let pts = 1_778_368_910u32;
        let mut asm = VideoAssembler::new();
        for _ in 0..255 {
            assert!(asm.push_payload(&bench_payload(pts, 0, &[0xA5; 2400])).is_none());
        }
        let closed = asm
            .push_payload(&bench_payload(pts, uvc_flags::EOF, &[0x5A; 2400]))
            .expect("the EOF payload completes the frame");
        assert_eq!(closed.pts, pts);
        assert_eq!(closed.data.len(), 256 * 2400); // = dwMaxVideoFrameSize
        assert_eq!(&closed.data[..2400], &[0xA5; 2400]);
        assert_eq!(&closed.data[255 * 2400..], &[0x5A; 2400]);
        assert_eq!(asm.frames, 1);
        assert_eq!(asm.payloads, 256);
        assert_eq!(asm.errors, 0);
        // no tail leaks into the next frame: the next PTS group starts
        // and closes with exactly its own bytes
        let pts2 = pts + PTS_PER_FRAME;
        assert!(asm.push_payload(&bench_payload(pts2, 0, &[0x11; 100])).is_none());
        let closed2 = asm
            .push_payload(&bench_payload(pts2, uvc_flags::EOF, &[0x22; 20]))
            .expect("EOF closes frame 2");
        assert_eq!(closed2.data.len(), 120);
        assert_eq!(&closed2.data[..100], &[0x11; 100]);
        assert_eq!(&closed2.data[100..], &[0x22; 20]);
    }

    #[test]
    fn error_payload_closes_without_its_bytes() {
        // An ERR-flagged payload closes the frame but its own bytes are
        // dropped with the payload (UVC: the payload is bad).
        let pts = 1_778_368_910u32;
        let mut asm = VideoAssembler::new();
        assert!(asm.push_payload(&bench_payload(pts, 0, &[0x33; 100])).is_none());
        let closed = asm
            .push_payload(&bench_payload(pts, uvc_flags::ERR, &[0x99; 50]))
            .expect("the error payload closes the frame");
        assert_eq!(closed.data.len(), 100);
        assert!(closed.data.iter().all(|&v| v == 0x33));
    }
}
