//! P2.5 video bring-up on Linux - the UVC plane of the 0471:2500
//! composite (turn 38; turn 44: both backends live in the crate and
//! the run survives the sensor's self-reboots). THE hardware test of
//! the video backend.
//!
//! ```sh
//! cargo run --features usb --example video -- --probe
//! cargo run --features usb --example video -- --cmos --frames 10
//! cargo run --features usb --example video -- --iso --cmos --frames 10
//! cargo run --features usb --example video -- --cmos --frames 10 --recover
//! cargo run --features usb --example video -- --iso --cmos --rate 300 --recover
//! ```

//! # The P2.6 rate question (turn 45)
//!
//! The turn-44 bench measured 2.1 fps on the raw ISO path vs ~25 fps
//! device-side through uvcvideo, same device, same hour. The loops this
//! example ran were the suspect: per-frame host work (PNG encode, two
//! planes of stats) sat between a reaped URB and its resubmission, so
//! the ring ran dry during our own processing and ASAP scheduling
//! hopped the next URB to a distant microframe - frames the device
//! produced simply never arrived (invisible to every error counter).
//! Both loops are now measure-first (requeue/resubmit BEFORE any
//! per-frame work; the bytes are copied out first), drop accounting
//! counts what the device made vs what we saw (seq gaps / PTS gaps),
//! and `--rate N` measures N frames with the ring never starved by
//! host work, reporting host wall-clock fps next to the PTS cadence -
//! the one-command answer to "what does this firmware actually
//! produce?". The turn-44 numbers (1/5/5/4/4 cadence = 2.1 fps
//! delivered, ~25 fps produced) are exactly the starvation signature
//! this removes.
//!
//! Modes and flags:
//! * `--probe`        list every /dev/video* node with driver, formats
//!                    and frame sizes (QUERYCAP + ENUM_FMT +
//!                    ENUM_FRAMESIZES + G_FMT). READ-ONLY, no streaming,
//!                    no USB writes - the 10-second answer to "what does
//!                    Linux see of the sensor's video interface?".
//! * `--cmos`         consent to the CMOS register writes on MI_04 (the
//!                    observed vendor session init, the same 17 writes
//!                    as src/examples/session_full.rs). Without it the stream
//!                    is attempted without any session - still a valid
//!                    experiment (the result is informative either way).
//! * `--iso`          the P2.5b raw usbfs-ISO backend: take the VS
//!                    interface from uvcvideo, run the observed
//!                    probe/commit, select the streaming alt-setting
//!                    (the bench-proven Linux requirement), stream EP
//!                    0x85 directly.
//! * `--alt N`        stream from this VS alt-setting instead of the
//!                    auto pick (alt 1 = 812 B/microframe, alt 2 =
//!                    2412). `--iso` only.
//! * `--no-set-alt`   skip the SET_INTERFACE before the ISO ring - the
//!                    capture-mirroring mode. The turn-43 bench proved
//!                    it cannot work on Linux (SUBMITURB ENOENT: the
//!                    host controller has no transfer ring for EP 0x85
//!                    at alt 0); kept as the A/B control. `--iso` only.
//! * `--device PATH`  use this /dev/videoN instead of the sysfs
//!                    auto-match (src/transport/v4l2.rs). Ignored with
//!                    `--iso`.
//! * `--frames N`    stop after N frames (default 10).
//! * `--rate N`      P2.6 (turn 45): rate-measurement mode - capture N
//!                    frames with the ring never starved by host work
//!                    and report the full rate table: host-wall-clock
//!                    fps, device-made fps (PTS/seq gap accounting),
//!                    and the PTS cadence summary. Sets the frame
//!                    budget to N (overrides --frames). Turn 47: the
//!                    promise is now real - per-frame work (stats,
//!                    console lines, PNG/bin writes) runs on a WORKER
//!                    thread, and PNGs are written only for the first 3
//!                    frames; the reap/QBUF loop only copies bytes.
//! * `--ring N`      `--iso` only: URB ring depth (default 16, 2..=64).
//!                    Each URB is 8 microframes x wMaxPacketSize, so N
//!                    URBs = N ms of ISO buffer. Run 11 ran with 4
//!                    (= 4 ms): any host hiccup longer than that
//!                    starved the ring and 86.8% of the device's frames
//!                    never arrived (1675 dropped / 255 delivered).
//! * `--save N`      keep the raw .bin of frame N (repeatable, any
//!                    mode). Normally only the first 3 frames get a
//!                    .bin - `--save 106 --save 107` keeps the frames
//!                    whose channel-B sd spiked in run 11 for the
//!                    offline `--analyze` pass.
//! * `--analyze DIR` no device touched: run the channel analysis
//!                    (turn 47) over the frameNNNN.bin files a run
//!                    left in DIR - per-frame A/B stats, row-mean
//!                    structure, A/B correlation, and the channel-B
//!                    image verdict (`is B an image too?` - the run-11
//!                    hand-cover question, answered from the bytes).
//! * `--outdir PATH` output folder (default: video_runN - the first
//!                    nonexistent or EMPTY one, the kit-37 runN rule).
//! * `--recover`     P2.4b (turn 44): survive the sensor's self-reboot.
//!                    The sensor drops to its 0471:2600 bootloader every
//!                    31-157 s (documented firmware behavior - the
//!                    vendor engine babysits it by re-flashing on every
//!                    landing; on this bench the DRIVER is that engine).
//!                    With `--recover` a mid-run reboot is handled
//!                    automatically: wait for the bootloader landing,
//!                    re-flash the pinned 8051 image (the ungated
//!                    FLASH_CONFIRMED path - the exact vendor sequence,
//!                    readback-verified before the commit key), wait for
//!                    the 2500 re-enumeration, re-init the session and
//!                    RESUME capture until N frames are done. The frame
//!                    counter, output folder and pooled stats carry
//!                    over across recoveries.
//!
//! Why V4L2: on Linux the uvcvideo kernel driver claims the sensor's UVC
//! VideoStreaming interface (MI_01) and does the probe/commit negotiation
//! itself; the vendor interface MI_04 stays free for the session transport
//! - so ONE process can run the session (rusb) and the video (V4L2)
//! side by side, exactly like the vendor engine on Windows. (The
//! turn-41 bench had uvcvideo fail STREAMON with EIO; the turn-44 bench
//! PASSED through uvcvideo right after a T8 flash - the EIO is
//! state-dependent, both observations are recorded in
//! docs/LINUX_TESTS.md.)
//!
//! Why --iso (P2.5b): the raw path drives EP 0x85 with no kernel UVC
//! driver in the way - the deterministic backend. Turn 44: the ioctl
//! engines of BOTH backends live in the crate now
//! (`transport::v4l2_stream`, `transport::usbfs_iso` - promoted on the
//! T7/T9 bench PASSes); this example is the bench script on top of
//! them.
//!
//! What PASS looks like: 640x480 frames, 614400 B each, channel A (even
//! bytes) the scene image with real contrast, channel B (odd bytes) the
//! second, low-contrast channel. Both are saved as grayscale PNGs plus
//! the raw interleaved bytes for analysis. Frame assembly: PTS advance
//! or the EOF-flagged final payload (whose data belongs to the closing
//! frame - the turn-44 assembler fix; the bench had shown every frame
//! split into a 612000-B body + a 2400-B tail under one PTS).

#[cfg(all(feature = "usb", target_os = "linux"))]
mod bench {
    use std::fs::{self, File, OpenOptions};
    use std::io;
    use std::os::fd::AsRawFd;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use idong::protocol::video::{PlaneStats, VideoAssembler};
    use idong::protocol::{PID_BOOTLOADER, PID_RUNTIME, VID};
    use idong::transport::usbfs_iso;
    use idong::transport::v4l2::{
        fourcc_str, iface_belongs_to_port, sensor_nodes, sensor_usb_ports, video_nodes,
    };
    use idong::transport::v4l2_stream::{uapi, V4l2Dev};
    use idong::{Session, UsbTransport};

    /// How many self-reboot recoveries one run may absorb (--recover).
    const MAX_RECOVERIES: u32 = 12;

    pub struct Args {
        probe: bool,
        cmos: bool,
        iso: bool,
        alt: Option<u8>,
        no_set_alt: bool,
        recover: bool,
        frames: u32,
        /// `--rate N`: rate-measurement mode - capture N frames with
        /// the ring never starved by per-frame work (all reporting and
        /// file writes run on a worker thread; PNGs only for the first
        /// 3 frames), reporting host-wall fps, device-made fps (gap
        /// accounting), and the PTS cadence summary. The P2.6 answer
        /// to "what does the device actually produce?"
        rate_probe: Option<u32>,
        /// `--ring N` (--iso): URB ring depth. Run 11 starved a 4-URB
        /// ring (4 ms of ISO buffer) with per-frame PNG work and lost
        /// 86.8% of the frames; 16 URBs of buffer plus the worker
        /// thread make the ring drought-proof.
        ring: usize,
        /// `--save N` (repeatable): always keep frame N's raw .bin (for
        /// `--analyze`), whatever the mode's normal .bin policy is.
        save: Vec<u32>,
        /// `--analyze DIR`: offline channel analysis of saved
        /// frameNNNN.bin files - no device touched.
        analyze: Option<String>,
        device: Option<String>,
        outdir: Option<String>,
    }

    impl Args {
        fn parse() -> Args {
            let a: Vec<String> = std::env::args().skip(1).collect();
            let mut args = Args {
                probe: false,
                cmos: false,
                iso: false,
                alt: None,
                no_set_alt: false,
                recover: false,
                frames: 10,
                rate_probe: None,
                ring: 16,
                save: Vec::new(),
                analyze: None,
                device: None,
                outdir: None,
            };
            let mut i = 0;
            while i < a.len() {
                match a[i].as_str() {
                    "--probe" => args.probe = true,
                    "--cmos" => args.cmos = true,
                    "--iso" => args.iso = true,
                    "--no-set-alt" => args.no_set_alt = true,
                    "--recover" => args.recover = true,
                    "--frames" | "--device" | "--outdir" | "--alt" | "--rate" | "--ring"
                    | "--save" | "--analyze" => {
                        let val = a.get(i + 1).cloned();
                        match (a[i].as_str(), val) {
                            ("--frames", Some(v)) => {
                                args.frames = v.parse().unwrap_or(10).clamp(1, 300)
                            }
                            ("--device", Some(v)) => args.device = Some(v),
                            ("--outdir", Some(v)) => args.outdir = Some(v),
                            ("--analyze", Some(v)) => args.analyze = Some(v),
                            ("--alt", Some(v)) => match v.parse::<u8>() {
                                Ok(n) => args.alt = Some(n),
                                Err(_) => eprintln!("warning: --alt {v:?} is not 0-255"),
                            },
                            ("--rate", Some(v)) => {
                                // --rate stands alone: capture & count N
                                // frames; the worker thread + the
                                // first-3-PNG policy is what keeps the
                                // ring full (the turn-45 promise, really
                                // implemented in turn 47).
                                args.rate_probe = Some(v.parse().unwrap_or(60).clamp(1, 3000));
                            }
                            ("--ring", Some(v)) => {
                                args.ring = v.parse().unwrap_or(16).clamp(2, 64);
                            }
                            ("--save", Some(v)) => match v.parse::<u32>() {
                                Ok(n) => args.save.push(n),
                                Err(_) => eprintln!("warning: --save {v:?} is not a frame number"),
                            },
                            _ => eprintln!("warning: {} needs a value", a[i]),
                        }
                        i += 1;
                    }
                    other => eprintln!("warning: unknown flag {other:?}"),
                }
                i += 1;
            }
            // --rate sets its own frame budget (overriding --frames).
            if let Some(n) = args.rate_probe {
                args.frames = n;
            }
            args
        }
    }

    fn cstr(bytes: &[u8]) -> String {
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).to_string()
    }

    /// What survives a self-reboot recovery: the output folder, the
    /// frame counter, the pooled plane stats and the rate measurement.
    /// Everything else (session, transports, URB ring, video node,
    /// frame worker) dies with the device and is rebuilt per attempt.
    ///
    /// Turn 47: the accounting is PER SESSION now. The run-11 numbers
    /// were misleading because the old RunState mixed them across
    /// recoveries - the "device rate 1.0 fps" divided delivered frames
    /// by a PTS span that included ~30 s reboot gaps each, the "host
    /// wall rate 51.5 fps" divided the TOTAL frame count by the LAST
    /// attempt's span, and the "1675 drops" counted the reboot gaps
    /// (the device was OFF then - nothing was dropped) on top of the
    /// real starvation losses.
    #[derive(Default)]
    struct RunState {
        /// Frames delivered to our loop (all sessions summed).
        got: u32,
        any_a: Option<PlaneStats>,
        any_b: Option<PlaneStats>,
        /// V4L2 last attempt: (frames_in_span, span_us) - the kernel
        /// timestamp rate line.
        rate: Option<(u32, f64)>,
        /// PTS cadence deltas in PTS_PER_FRAME units (all sessions,
        /// in order - the worker appends per session).
        pts_deltas_fr: Vec<f64>,
        recoveries: u32,
        /// Captures that produced frames (recovery separates them).
        sessions: u32,
        /// Frames the DEVICE made (delivered + intra-session gaps) -
        /// the honest device-side production count.
        made: u64,
        /// Device-side frames the bus delivered but the loop did NOT
        /// process in time: V4L2 buffer seq gaps (uvcvideo counted a
        /// buffer, our loop missed it), ISO PTS gaps > 1 frame period
        /// WITHIN one session (reboot gaps are not drops - the device
        /// was in its bootloader).
        drops: u64,
        /// Sum of the per-session PTS spans (ticks) - first to last
        /// frame of each session, no reboot gaps inside.
        pts_span_ticks: u64,
        /// Sum of the per-session ACTIVE wall spans (first frame
        /// delivered to last frame of the session).
        wall_active: Duration,
        /// The whole run's wall time (boot to summary).
        run_started: Option<Instant>,
        /// The last frame's full channel report (the B verdict,
        /// turn 47).
        last_report: Option<idong::protocol::video::ChannelReport>,
    }

    /// One frame handed to the worker thread: everything the per-frame
    /// reporting needs. The reap/QBUF loop NEVER does per-frame host
    /// work beyond building this struct (a Vec allocation + memcpy) -
    /// run 11 proved what happens otherwise: two PNG encodes + two
    /// plane-stats passes + a println per frame starved a 4-ms ISO
    /// ring and 86.8% of the device's frames never arrived.
    struct FrameJob {
        /// Global frame number (the output file name + counter).
        no: u32,
        /// ISO: the frame's PTS (the cadence accounting).
        pts: Option<u32>,
        /// V4L2: the kernel's buffer sequence number.
        seq: Option<u32>,
        /// V4L2: the kernel buffer timestamp (us) - the measured-rate line.
        ts: Option<i64>,
        bytes: Vec<u8>,
    }

    /// The per-session tally the frame worker returns when the
    /// capture loop is done (or the device went away).
    #[derive(Default)]
    struct SessionTally {
        /// Frames the worker processed this session.
        delivered: u32,
        any_a: Option<PlaneStats>,
        any_b: Option<PlaneStats>,
        /// Intra-session PTS gaps only (ISO).
        dropped: u64,
        /// first..last frame PTS of THIS session (ISO).
        first_pts: Option<u32>,
        last_pts: Option<u32>,
        pts_deltas_fr: Vec<f64>,
        /// V4L2: the session's seq range (drop accounting).
        seq_first: Option<u32>,
        seq_last: Option<u32>,
        /// V4L2: first/last kernel buffer timestamp (us) of the session.
        ts_first: Option<i64>,
        ts_last: Option<i64>,
        /// The last frame's full channel report (the B verdict).
        last_report: Option<idong::protocol::video::ChannelReport>,
        /// PNGs skipped because the worker fell behind (long
        /// normal-mode runs; --rate never writes PNGs beyond frame 3).
        pngs_skipped: u32,
    }

    /// The per-frame work horse: stats, console line, .bin + PNG
    /// writes, PTS/seq accounting - everything the streaming loop
    /// must NOT do. One worker per capture session, ordered.
    struct FrameSink {
        tx: crossbeam_channel::Sender<FrameJob>,
        tally: crossbeam_channel::Receiver<SessionTally>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl FrameSink {
        /// Spawn the session's worker. `iso` picks the console-line
        /// format; `png_first3_only` is the --rate policy; `save` is
        /// the --save frame list (always keep those .bins).
        fn spawn(outdir: PathBuf, iso: bool, png_first3_only: bool, save: Vec<u32>) -> FrameSink {
            let (tx, rx) = crossbeam_channel::bounded::<FrameJob>(256);
            let (ttx, tally) = crossbeam_channel::bounded::<SessionTally>(1);
            let handle = std::thread::spawn(move || {
                let mut t = SessionTally::default();
                let pix = pix_for();
                for job in rx.iter() {
                    let r = idong::protocol::video::channel_report(&job.bytes);
                    let dt = match t.last_pts {
                        Some(prev) if job.pts.is_some() => format!(
                            " (dt {:4.1} fr)",
                            job.pts.unwrap().wrapping_sub(prev) as f64
                                / idong::protocol::video::PTS_PER_FRAME as f64
                        ),
                        _ => String::new(),
                    };
                    if iso {
                        println!(
                            "      frame {:3}: pts {}{dt} {} B  chA mean {:6.1} sd {:5.1}  chB mean {:6.1} sd {:5.1}",
                            job.no,
                            job.pts.unwrap_or(0),
                            job.bytes.len(),
                            r.a.mean,
                            r.a.stddev,
                            r.b.mean,
                            r.b.stddev
                        );
                    } else {
                        println!(
                            "      frame {:3}: seq {:3} {} B  chA mean {:6.1} sd {:5.1} [{:3},{:3}]  chB mean {:6.1} sd {:5.1} [{:3},{:3}]",
                            job.no,
                            job.seq.unwrap_or(0),
                            job.bytes.len(),
                            r.a.mean,
                            r.a.stddev,
                            r.a.min,
                            r.a.max,
                            r.b.mean,
                            r.b.stddev,
                            r.b.min,
                            r.b.max
                        );
                    }
                    // intra-session drop accounting (ISO): a PTS jump
                    // larger than one frame period = frames the device
                    // made that our starved loop never saw. The FIRST
                    // frame of a session has no delta.
                    if let Some(pts) = job.pts {
                        if let Some(prev) = t.last_pts {
                            t.dropped += idong::protocol::video::count_dropped_frames(
                                prev,
                                pts,
                                idong::protocol::video::PTS_PER_FRAME,
                                true,
                            );
                            t.pts_deltas_fr.push(
                                pts.wrapping_sub(prev) as f64
                                    / idong::protocol::video::PTS_PER_FRAME as f64,
                            );
                        }
                    }
                    if let Some(seq) = job.seq {
                        // the first delta of a session is not a drop
                        if t.seq_last.is_some() {
                            t.dropped += idong::protocol::video::count_dropped_frames(
                                t.seq_last.unwrap(),
                                seq,
                                1,
                                false,
                            );
                        }
                        t.seq_first.get_or_insert(seq);
                        t.seq_last = Some(seq);
                    }
                    if let Some(ts) = job.ts {
                        // first..last kernel timestamp of the session
                        if t.ts_first.is_none() {
                            t.ts_first = Some(ts);
                        }
                        t.ts_last = Some(ts);
                    }
                    // .bin policy: first 3 frames + every --save frame
                    if job.no < 3 || save.contains(&job.no) {
                        let _ =
                            fs::write(outdir.join(format!("frame{:04}.bin", job.no)), &job.bytes);
                    }
                    // PNG policy: all frames (normal mode), first 3
                    // only (--rate) - and never when the worker is
                    // behind (the channel backlog says so), so a long
                    // normal run degrades to stats-only instead of
                    // starving the ring via a full channel.
                    let want_png = !png_first3_only || job.no < 3;
                    if want_png {
                        if png_first3_only || rx.len() < 32 {
                            let (a, b) = idong::protocol::video::split_channels(&job.bytes);
                            write_plane_png(
                                &outdir.join(format!("frame{:04}_chA.png", job.no)),
                                &a,
                                &pix,
                            );
                            write_plane_png(
                                &outdir.join(format!("frame{:04}_chB.png", job.no)),
                                &b,
                                &pix,
                            );
                        } else {
                            t.pngs_skipped += 1;
                        }
                    }
                    t.any_a = merge(t.any_a, r.a);
                    t.any_b = merge(t.any_b, r.b);
                    if job.pts.is_some() {
                        t.first_pts.get_or_insert(job.pts.unwrap());
                        t.last_pts = job.pts;
                    }
                    t.last_report = Some(r);
                    t.delivered += 1;
                }
                // the channel closed: hand the tally back
                let _ = ttx.send(t);
            });
            FrameSink { tx, tally, handle: Some(handle) }
        }

        /// Queue one frame. Crossbeam `send` on a bounded channel
        /// BLOCKS while 256 frames are already queued - that backpressure
        /// is the safety valve: a worker that somehow falls behind slows
        /// the reap loop (visible in the cadence) instead of exhausting
        /// memory; --rate mode never queues PNG work at all.
        fn send(&self, job: FrameJob) {
            let _ = self.tx.send(job);
        }

        /// End the session: close the job channel, wait for the worker
        /// to drain it, return the tally.
        fn finish(mut self) -> SessionTally {
            drop(self.tx);
            let t = self.tally.recv().unwrap_or_default();
            if let Some(h) = self.handle.take() {
                let _ = h.join();
            }
            t
        }
    }

    /// One capture attempt ends either complete or with the device gone
    /// (the 31-157 s self-reboot, or an unplug). Setup failures exit
    /// inline exactly like the pre-turn-44 script did.
    enum AttemptOutcome {
        /// N frames captured (or the budget spent) - the run is over.
        Done,
        /// The sensor went away mid-attempt.
        Gone(String),
    }

    pub fn run() {
        let args = Args::parse();
        if let Some(dir) = &args.analyze {
            analyze_dir(&PathBuf::from(dir));
            return;
        }
        if args.probe {
            probe(&args);
            return;
        }
        // the P2.4b recovery loop (turn 44): each attempt is a full
        // session + backend setup; --recover absorbs the self-reboots
        // between attempts. Without it the behavior is exactly the old
        // one: a gone device ends the run with the summary of what was
        // captured.
        let mut st = RunState { run_started: Some(Instant::now()), ..RunState::default() };
        let outdir = match &args.outdir {
            Some(d) => PathBuf::from(d),
            None => next_run_dir(),
        };
        fs::create_dir_all(&outdir).expect("create output dir");
        let image = if args.recover {
            println!("--recover: verifying the pinned 8051 image (no device touched)...");
            match load_recovery_image() {
                Some(img) => {
                    println!("      PASS: sha256 pins hold (raw hex + 21,411-B image)");
                    println!("      a self-reboot landing will be re-flashed automatically");
                    Some(img)
                }
                None => std::process::exit(1),
            }
        } else {
            None
        };
        if args.iso {
            println!("=== idong P2.5b raw usbfs-ISO bring-up ===");
        } else {
            println!("=== idong P2.5 video bring-up ===");
        }
        println!();
        loop {
            let outcome = if args.iso {
                iso_attempt(&args, &outdir, &mut st)
            } else {
                v4l2_attempt(&args, &outdir, &mut st)
            };
            match outcome {
                AttemptOutcome::Done => break,
                AttemptOutcome::Gone(e) => {
                    if !args.recover {
                        gone_no_recover(&e, st.got > 0);
                        break;
                    }
                    if st.recoveries >= MAX_RECOVERIES {
                        println!(
                            "FAIL: {MAX_RECOVERIES} self-reboot recoveries spent - the sensor"
                        );
                        println!("      is not staying up (unstable port? power?). Sending the");
                        println!("      console output back decides the next move.");
                        break;
                    }
                    st.recoveries += 1;
                    println!(
                        "      self-reboot #{}/{} mid-run ({e})",
                        st.recoveries, MAX_RECOVERIES
                    );
                    println!("      (the documented 31-157 s firmware cycle - not a driver bug)");
                    match recover_after_reboot(image.as_deref().unwrap_or(&[])) {
                        true => continue,
                        false => {
                            println!("      recovery failed - rerun with --recover or flash_mcu");
                            break;
                        }
                    }
                }
            }
        }
        if st.got == 0 {
            finish_empty();
        } else {
            finish(&args, &st, &outdir);
        }
    }

    /// The old behavior verbatim: a gone device without --recover.
    fn gone_no_recover(e: &str, started: bool) {
        if started {
            println!("      device went away mid-run: {e}");
            println!("      (the sensor self-reboots every 31-157 s - the documented");
            println!("      firmware cycle; the vendor engine re-flashes it, this");
            println!("      driver does the same with: --recover)");
        } else {
            println!("      FAIL: {e}");
            println!("      - sensor plugged in? udev rule installed? in the bootloader");
            println!("        after a self-reboot (flash it: flash_mcu --confirm, or just");
            println!("        add --recover here - the driver flashes and resumes)");
        }
    }

    fn finish_empty() -> ! {
        println!("FAIL: no frames within the budget.");
        println!("      checks: rerun WITH --cmos; is another app using the node;");
        println!("      dmesg | tail -50; formats: run --probe first and send both outputs.");
        println!("      The raw ISO backend (--iso) and --recover are the other dials.");
        std::process::exit(1);
    }

    fn finish(args: &Args, st: &RunState, outdir: &Path) {
        println!();
        println!(
            "frames captured : {}{}",
            st.got,
            if st.recoveries > 0 {
                format!(
                    " ({} self-reboot recovery/recoveries, {} capture session(s))",
                    st.recoveries, st.sessions
                )
            } else if st.sessions > 1 {
                format!(" ({} capture sessions)", st.sessions)
            } else {
                String::new()
            }
        );
        // host wall-clock rate over the ACTIVE streaming spans (turn 47):
        // every capture session's first..last frame delivery, summed.
        // The run-11 line divided the TOTAL frame count by the LAST
        // session's span and printed 51.5 fps - 10x the truth.
        let wall_s = st.wall_active.as_secs_f64();
        if wall_s > 0.0 && st.got > 1 {
            let delivered_fps = (st.got - 1) as f64 / wall_s.max(f64::EPSILON);
            println!(
                "host wall rate  : {delivered_fps:.1} fps over {wall_s:.1} s active streaming ({} frames delivered to our loop)",
                st.got
            );
        }
        if let Some(t0) = st.run_started {
            println!(
                "total wall time : {:.1} s (includes recoveries + setup)",
                t0.elapsed().as_secs_f64()
            );
        }
        // device-made accounting (turn 47): delivered + intra-session
        // gap losses; the reboot gaps count nowhere (the device was in
        // its bootloader - run 11's "1675 drops" wrongly included them).
        if st.drops > 0 {
            println!(
                "device drops    : {} frames the device made, our loop missed (intra-session PTS/seq gaps)",
                st.drops
            );
        } else {
            println!("device drops    : 0 (every frame the device made was delivered)");
        }
        // the device's true production rate: frames made over the ACTIVE
        // wall spans - assumption-free, the number T11 exists for.
        let made = st.made.max(st.got as u64);
        if wall_s > 0.0 && made > 1 {
            let device_fps = (made - 1) as f64 / wall_s.max(f64::EPSILON);
            println!(
                "device rate     : {device_fps:.1} fps ({} made / active span, wall clock)",
                made
            );
        }
        // PTS cadence summary (ISO): the deltas in PTS_PER_FRAME units,
        // summarized - the run-11 full dump was a 250-entry line.
        if !st.pts_deltas_fr.is_empty() {
            let mut sorted = st.pts_deltas_fr.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let median = sorted[sorted.len() / 2];
            let max = sorted[sorted.len() - 1];
            let head: Vec<String> =
                st.pts_deltas_fr.iter().take(10).map(|d| format!("{d:.1}")).collect();
            println!(
                "pts cadence     : first 10: {} ... median {median:.1}, max {max:.1} frame periods (PTS_PER_FRAME = {} units)",
                head.join("/"),
                idong::protocol::video::PTS_PER_FRAME
            );
            // the PTS tick-rate cross-check: with a flat cadence and
            // zero drops, frame period (wall) x PTS_PER_FRAME = the
            // device clock. The 15.36 MHz constant came from the golden
            // capture's 8.0 fps; the uvcvideo bench measured ~25 fps -
            // a clean run settles which is right.
            if st.drops == 0 && st.got > 1 && wall_s > 0.0 {
                let frame_period_s = wall_s.max(f64::EPSILON) / (st.got - 1) as f64;
                let implied_hz = idong::protocol::video::PTS_PER_FRAME as f64 / frame_period_s;
                println!(
                    "pts tick rate   : {implied_hz:.2} MHz implied (PTS_PER_FRAME / wall frame period)"
                );
                let assumed = idong::protocol::video::DEVICE_CLOCK_HZ as f64;
                if (implied_hz - assumed).abs() / assumed > 0.10 {
                    println!(
                        "                  MISMATCH vs the assumed {assumed} Hz - the DEVICE_CLOCK_HZ"
                    );
                    println!(
                        "                  constant needs the measured value (48 MHz would read {:.2}x)",
                        implied_hz / assumed
                    );
                } else {
                    println!("                  agrees with DEVICE_CLOCK_HZ - the constant stands");
                }
            }
        }
        if !args.iso {
            if let Some((n, span_us)) = st.rate {
                if n > 1 && span_us > 0.0 {
                    let fps = (n - 1) as f64 * 1_000_000.0 / span_us;
                    println!("measured rate   : {fps:.1} fps (kernel timestamps, last session)");
                }
            }
        }
        println!(
            "session         : {}",
            if args.cmos { "held (MI_04 claim)" } else { "none (--cmos absent)" }
        );
        println!("channel A stats : {:?}", st.any_a);
        println!("channel B stats : {:?}", st.any_b);
        // the channel-B verdict (turn 47): pixel sd AND row-mean
        // structure must BOTH be present for an image - the run-11
        // hand-cover data answered this one (B: 127.3 +- 6.4, no image).
        if let Some(r) = st.last_report {
            println!(
                "channel B       : {} (last frame: sd {:.1}, row spread {:.2}, A/B corr {:+.2})",
                r.b_verdict(),
                r.b.stddev,
                r.b_rows.spread_vs_sd,
                r.corr_ab
            );
            if !r.b_is_image() {
                println!(
                    "                  full pass over the saved .bins: --analyze {}",
                    outdir.display()
                );
            }
        }
        if let Some(a) = st.any_a {
            if a.has_contrast() {
                println!("PASS: channel A carries a real image (contrast present).");
            } else {
                println!("WARN: channel A is flat - registers not initialized? (use --cmos)");
            }
            if a.n > 0 {
                println!("PASS: {} channel-A samples pooled.", a.n);
            }
        }
        println!("output: {} (PNGs per channel + raw .bin of the first frames)", outdir.display());
        println!("next steps:");
        println!("  - rerun the clean rate table: --iso --cmos --rate 300 --recover");
        println!("    (the worker-thread loop: expect dt 1.0 flat, drops 0)");
        println!("  - offline channel analysis of the saved .bins: --analyze {}", outdir.display());
        println!("  - the record plane (markers): records --cmos (T12)");
    }

    // ---------------- --probe (unchanged behavior) ----------------

    fn probe(args: &Args) {
        println!("=== idong P2.5 video probe (read-only) ===");
        println!();
        let nodes = video_nodes();
        if nodes.is_empty() {
            println!("FAIL: no /dev/video* nodes at all on this system.");
            println!("      uvcvideo did not create a node for the sensor - collect:");
            println!("        dmesg | tail -50");
            println!("        lsusb -v -d 0471:2500   (or 0471:2600)");
            std::process::exit(1);
        }
        let ports = sensor_usb_ports(VID, PID_RUNTIME);
        println!("sensor 0471:{:04x} attached at usb port(s): {:?}", PID_RUNTIME, ports);
        let boot = sensor_usb_ports(VID, PID_BOOTLOADER);
        if !boot.is_empty() {
            println!("note: bootloader device 0471:2600 at {boot:?} - the UVC node exists");
            println!("      only in the runtime state (the vendor flash loads it)");
        }
        println!();
        for n in &nodes {
            println!("[{}] {}", n.index, n.dev.display());
            match n.usb_iface.as_deref() {
                Some(i) => println!("      usb interface : {i}"),
                None => println!("      usb interface : (no sysfs link - cannot auto-match)"),
            }
            let is_sensor = n
                .usb_iface
                .as_deref()
                .map(|i| ports.iter().any(|p| iface_belongs_to_port(i, p)))
                .unwrap_or(false);
            println!("      matches sensor: {}", is_sensor);
            let dev = match V4l2Dev::open(&n.dev) {
                Ok(d) => d,
                Err(e) => {
                    println!("      (cannot open: {e})");
                    if e.kind() == io::ErrorKind::PermissionDenied {
                        println!("      permissions: add yourself to the 'video' group:");
                        println!("        sudo usermod -aG video $USER   # then re-login");
                    }
                    continue;
                }
            };
            let c = dev.caps();
            println!(
                "      driver {} card {:?} bus {:?}",
                cstr(&c.driver),
                cstr(&c.card),
                cstr(&c.bus_info)
            );
            let cap_capture = dev.is_capture();
            let cap_stream = dev.device_caps() & uapi::CAP_STREAMING != 0;
            println!("      capture={} streaming={}", cap_capture, cap_stream);
            if !cap_capture {
                println!("      (not a capture node - no formats listed)");
                continue;
            }
            for (desc, fourcc) in dev.enum_formats() {
                let sizes = dev.enum_framesizes(fourcc).unwrap_or_default();
                println!(
                    "      format {} ({:?}) fourcc {}",
                    fourcc_str(fourcc),
                    desc,
                    fourcc_str(fourcc)
                );
                for (w, h) in sizes {
                    println!("          {}x{}", w, h);
                }
            }
            match dev.get_format() {
                Ok(p) => println!(
                    "      current: {}x{} {} bpl {} size {}",
                    p.width,
                    p.height,
                    fourcc_str(p.pixelformat),
                    p.bytesperline,
                    p.sizeimage
                ),
                Err(e) => println!("      (G_FMT: {e})"),
            }
        }
        println!();
        println!("PASS: probe complete. Send this output back.");
        println!("next: cargo run --features usb --example video -- --cmos --frames 10");
        if let Some(d) = &args.device {
            println!("(--device {d} was ignored in probe mode)");
        }
    }

    // ---------------- the V4L2 backend attempt ----------------

    /// One full V4L2 capture attempt. Setup failures exit inline (the
    /// old script behavior); the capture loop returns [`AttemptOutcome`]
    /// when the frames are done or the device went away.
    fn v4l2_attempt(args: &Args, outdir: &Path, st: &mut RunState) -> AttemptOutcome {
        // [1/4] session init on MI_04 (rusb), only with --cmos
        // The transport lives in a fixed Option slot; the Session borrows
        // that slot for the whole stream (drop order frees it correctly).
        // The slot's value is never read directly - keeping it alive IS
        // the point (it holds the MI_04 claim for the Session).
        #[allow(unused_assignments)]
        let mut transport: Option<UsbTransport> = None;
        #[allow(unused_assignments)]
        let mut session: Option<Session> = None;
        if args.cmos {
            println!("[1/4] session init on MI_04 (--cmos: EEPROM reads + CMOS registers)");
            let t = match UsbTransport::open_runtime() {
                Ok(t) => t,
                Err(e) => {
                    // the self-reboot may already have happened: recover
                    // if asked, else the old hint block
                    return AttemptOutcome::Gone(format!("cannot open MI_04: {e}"));
                }
            };
            transport = Some(t);
            let t = transport.as_mut().unwrap();
            match Session::probe(&mut *t) {
                Ok(idong::Phase::Runtime) => {}
                Ok(_) => {
                    return AttemptOutcome::Gone(
                        "device not in runtime phase (bootloader landing?)".into(),
                    );
                }
                Err(e) => return AttemptOutcome::Gone(format!("version probe: {e}")),
            }
            let s = match Session::connect(&mut *t, idong::Phase::Runtime, &[], false) {
                Ok(s) => s,
                Err(e) => {
                    return AttemptOutcome::Gone(format!("session init: {e}"));
                }
            };
            println!("      device ID {}", s.info().device_id);
            println!("      PASS: session init complete (register tables + readbacks)");
            session = Some(s);
            // pre-flight 0x0205 poll (turn 47): ONE attempt while not
            // streaming - the old ~10 Hz in-stream loop only ever
            // starved the DQBUF loop ~3 s per unanswered poll (the
            // device never answers mid-stream on this bench).
            if let Some(s) = session.as_mut() {
                match s.poll_stream() {
                    Ok(_) => println!("      0x0205: answered pre-stream - session held"),
                    Err(_) => println!("      0x0205: silent pre-stream (not fatal) - no polls"),
                }
            }
        } else {
            println!("[1/4] session init SKIPPED (no --cmos)");
            println!("      the vendor flow always inits the sensor registers before");
            println!("      video; without it the stream may stay empty (still a valid");
            println!("      experiment - a FAIL here does not reject the backend).");
        }

        // [2/4] pick the video node
        println!("[2/4] video node");
        let node: PathBuf = match &args.device {
            Some(d) => {
                println!("      using --device {d}");
                PathBuf::from(d)
            }
            None => {
                let nodes = video_nodes();
                if nodes.is_empty() {
                    println!("      FAIL: no /dev/video* nodes at all (uvcvideo did not bind).");
                    println!("      collect: dmesg | tail -50  and  lsusb -v -d 0471:2500");
                    std::process::exit(1);
                }
                let ports = sensor_usb_ports(VID, PID_RUNTIME);
                let hits = sensor_nodes(&nodes, &ports);
                match hits.len() {
                    1 => {
                        let n = hits[0];
                        println!(
                            "      sensor at usb {:?} -> {} (iface {:?})",
                            ports,
                            n.dev.display(),
                            n.usb_iface
                        );
                        n.dev.clone()
                    }
                    0 => {
                        // after a self-reboot the runtime is gone and no
                        // video node matches - recoverable, not fatal
                        return AttemptOutcome::Gone(
                            "no /dev/video* node matched the sensor (device gone?)".into(),
                        );
                    }
                    _ => {
                        // Turn-39: a composite sensor exposes SEVERAL nodes
                        // on the same interface - pick the one that can
                        // actually capture (read-only QUERYCAP, like --probe).
                        println!(
                            "      several nodes matched the sensor (iface {:?}):",
                            hits[0].usb_iface
                        );
                        let mut pick: Option<&idong::transport::v4l2::VideoNode> = None;
                        for n in &hits {
                            let is_capture =
                                V4l2Dev::open(&n.dev).map(|d| d.is_capture()).unwrap_or(false);
                            println!("        {}  capture={}", n.dev.display(), is_capture);
                            if is_capture {
                                pick = Some(n);
                            }
                        }
                        match pick {
                            Some(n) => {
                                println!("      PASS: the capture node is {}", n.dev.display());
                                n.dev.clone()
                            }
                            None => {
                                println!("      FAIL: no matched node can capture;");
                                println!("      pick one manually: --device /dev/videoN");
                                std::process::exit(1);
                            }
                        }
                    }
                }
            }
        };

        // [3/4] open + negotiate
        println!("[3/4] open + format");
        // open with retry (turn 47): after a recovery the /dev/video
        // node is fresh, and udev can take a moment to apply its
        // permissions - a PermissionDenied right after a re-enumeration
        // is the same settle race the bootloader node shows. Retry
        // 3x/1 s before treating it as fatal.
        let mut dev = None;
        for attempt in 0..3u32 {
            match V4l2Dev::open(&node) {
                Ok(d) => {
                    dev = Some(d);
                    break;
                }
                Err(e) => {
                    let gone = e.kind() == io::ErrorKind::NotFound
                        || e.raw_os_error() == Some(libc_enodev());
                    if gone || attempt == 2 {
                        if gone {
                            return AttemptOutcome::Gone(format!("open {}: {e}", node.display()));
                        }
                        println!("      FAIL: cannot open {}: {e}", node.display());
                        if e.kind() == io::ErrorKind::PermissionDenied {
                            println!(
                                "      permissions: sudo usermod -aG video $USER, then re-login"
                            );
                        }
                        std::process::exit(1);
                    }
                    println!(
                        "      open {}: {e} - retrying ({} of 2, udev settle)",
                        node.display(),
                        attempt + 1
                    );
                    std::thread::sleep(Duration::from_millis(1000));
                }
            }
        }
        let Some(mut dev) = dev else {
            return AttemptOutcome::Gone(format!("open {}: no node", node.display()));
        };
        let c = dev.caps();
        println!(
            "      driver {} card {:?} bus {:?}",
            cstr(&c.driver),
            cstr(&c.card),
            cstr(&c.bus_info)
        );
        if !dev.is_capture() {
            println!("      FAIL: node is not a video capture device");
            std::process::exit(1);
        }
        if dev.device_caps() & uapi::CAP_STREAMING == 0 {
            println!("      WARN: no streaming I/O - this tool only implements mmap streaming");
        }
        let mut pix = match dev.get_format() {
            Ok(p) => p,
            Err(e) => {
                println!("      FAIL: G_FMT: {e}");
                std::process::exit(1);
            }
        };
        println!(
            "      current format: {}x{} {} bpl {} size {}",
            pix.width,
            pix.height,
            fourcc_str(pix.pixelformat),
            pix.bytesperline,
            pix.sizeimage
        );
        if pix.width != 640 || pix.height != 480 {
            println!("      negotiating 640x480 (the vendor commits 640x480 x 2 B)...");
            pix.width = 640;
            pix.height = 480;
            pix = match dev.set_format(pix) {
                Ok(p) => p,
                Err(e) => {
                    println!("      FAIL: S_FMT: {e}");
                    std::process::exit(1);
                }
            };
            println!(
                "      negotiated: {}x{} {} bpl {} size {}",
                pix.width,
                pix.height,
                fourcc_str(pix.pixelformat),
                pix.bytesperline,
                pix.sizeimage
            );
        }
        if pix.width == 0 || pix.sizeimage == 0 {
            println!("      FAIL: unusable format (zero size) - ENUM_FMT said:");
            for (desc, fourcc) in dev.enum_formats() {
                println!("        {:?} {}", desc, fourcc_str(fourcc));
            }
            std::process::exit(1);
        }

        // [4/4] stream
        println!("[4/4] streaming {} frame(s)", args.frames);
        println!("      output dir: {}", outdir.display());
        let nbufs = match dev.request_buffers(4) {
            Ok(n) => n,
            Err(e) => {
                println!("      FAIL: REQBUFS: {e}");
                println!("      (another process streaming from the node? close cheese/guvcview)");
                std::process::exit(1);
            }
        };
        if nbufs == 0 {
            println!("      FAIL: driver granted 0 buffers");
            std::process::exit(1);
        }
        for i in 0..nbufs {
            if let Err(e) = dev.map_buffer(i) {
                println!("      FAIL: mmap buffer {i}: {e}");
                std::process::exit(1);
            }
            if let Err(e) = dev.queue(i) {
                println!("      FAIL: QBUF {i}: {e}");
                std::process::exit(1);
            }
        }
        if let Err(e) = dev.stream_on() {
            println!("      FAIL: STREAMON: {e}");
            println!("      this is the alt-setting decision point: the UVC streaming");
            println!("      interface may need an alt the firmware does not expose.");
            println!("      collect dmesg | tail -50 - the usbfs-ISO backend (P2.5b)");
            println!("      is the fallback if uvcvideo cannot stream this device.");
            std::process::exit(1);
        }
        println!("      PASS: streaming started ({} mmap buffers)", nbufs);

        let t_start = Instant::now();
        let mut attempt_frames = 0u32;
        let mut wall_first: Option<Instant> = None;
        let mut wall_last: Option<Instant> = None;
        let mut gone: Option<String> = None;
        // the frame worker (turn 47): the DQBUF loop only copies bytes
        // and requeues; stats, console lines and file writes happen on
        // the worker thread.
        let sink = FrameSink::spawn(
            outdir.to_path_buf(),
            false,
            args.rate_probe.is_some(),
            args.save.clone(),
        );

        while st.got + attempt_frames < args.frames && t_start.elapsed() < Duration::from_secs(30) {
            match dev.dq(500) {
                Ok(Some((idx, bytesused, seq, ts))) => {
                    // MEASURE-FIRST: return the buffer to the driver
                    // BEFORE any per-frame host work (turn 45) - and the
                    // per-frame work now runs on the worker anyway. The
                    // frame bytes are copied out first (the buffer is
                    // reused the moment it is requeued).
                    let bytes = dev.buffer_bytes(idx, bytesused).to_vec();
                    if let Err(e) = dev.queue(idx) {
                        println!("      FAIL: requeue: {e}");
                        gone = Some(format!("requeue: {e}"));
                        break;
                    }
                    let now = Instant::now();
                    wall_first.get_or_insert(now);
                    wall_last = Some(now);
                    sink.send(FrameJob {
                        no: st.got + attempt_frames,
                        pts: None,
                        seq: Some(seq),
                        ts: Some(ts),
                        bytes,
                    });
                    attempt_frames += 1;
                }
                Ok(None) => continue,
                Err(e) => {
                    gone = Some(format!("DQBUF: {e}"));
                    break;
                }
            }
        }
        dev.stream_off();
        let tally = sink.finish();
        if attempt_frames > 0 {
            st.sessions += 1;
            if let (Some(w0), Some(w1)) = (wall_first, wall_last) {
                st.wall_active += w1.duration_since(w0);
            }
            if let (Some(t0), Some(t1)) = (tally.ts_first, tally.ts_last) {
                if t1 > t0 {
                    st.rate = Some((attempt_frames, (t1 - t0) as f64));
                }
            }
        }
        merge_tally(st, tally);
        if let Some(e) = gone {
            return AttemptOutcome::Gone(e);
        }
        AttemptOutcome::Done
    }

    /// ENODEV as a plain digit (the example keeps no libc import - the
    /// engines own the raw calls; ENODEV = 19 on every Linux ABI).
    fn libc_enodev() -> i32 {
        19
    }

    // ---------------- the --iso (P2.5b) backend attempt ----------------

    /// One raw usbfs-ISO capture attempt (same shape as the V4L2 one:
    /// inline exits for setup failures, [`AttemptOutcome`] for the
    /// capture phase).
    fn iso_attempt(args: &Args, outdir: &Path, st: &mut RunState) -> AttemptOutcome {
        use idong::protocol::video::ProbeCommit;
        use idong::transport::usbfs as pure;

        // [1/5] session init on MI_04 (same as the V4L2 path, --cmos)
        // the transport slot is kept alive to the end: dropping it frees
        // MI_04 AFTER the stream stops (the session owns no resource)
        #[allow(unused_assignments)]
        let mut transport: Option<UsbTransport> = None;
        #[allow(unused_assignments)]
        let mut session: Option<Session> = None;
        if args.cmos {
            println!("[1/5] session init on MI_04 (--cmos: EEPROM reads + CMOS registers)");
            let t = match UsbTransport::open_runtime() {
                Ok(t) => t,
                Err(e) => return AttemptOutcome::Gone(format!("cannot open MI_04: {e}")),
            };
            transport = Some(t);
            let t = transport.as_mut().unwrap();
            match Session::probe(&mut *t) {
                Ok(idong::Phase::Runtime) => {}
                Ok(_) => {
                    return AttemptOutcome::Gone(
                        "device not in runtime phase (bootloader landing?)".into(),
                    );
                }
                Err(e) => return AttemptOutcome::Gone(format!("version probe: {e}")),
            }
            let s = match Session::connect(&mut *t, idong::Phase::Runtime, &[], false) {
                Ok(s) => s,
                Err(e) => return AttemptOutcome::Gone(format!("session init: {e}")),
            };
            println!("      device ID {}", s.info().device_id);
            println!("      PASS: session init complete");
            session = Some(s);
            // pre-flight 0x0205 poll (turn 47): the ONE poll while not
            // streaming. The old ~10 Hz loop inside the ring was pure
            // loss: the device never answers mid-stream on this bench
            // (every run since turn 42), and each unanswered poll
            // blocked the reap loop ~3 s (the reply-timeout budget) -
            // run 11 shows exactly that as the first dt 4.0/5.0/6.0
            // staircase after every recovery.
            if let Some(s) = session.as_mut() {
                match s.poll_stream() {
                    Ok(_) => println!("      0x0205: answered pre-stream - session held"),
                    Err(_) => println!("      0x0205: silent pre-stream (not fatal) - no polls"),
                }
            }
            println!("      (no 0x0205 polls during the ring - the device ignores them");
            println!("       while streaming; the golden capture answers mid-stream on");
            println!("       Windows, one open difference)");
        } else {
            println!("[1/5] session init SKIPPED (no --cmos)");
            println!("      the vendor flow always inits the sensor registers before");
            println!("      video; without it the stream may stay empty (still a valid");
            println!("      experiment - a FAIL here does not reject the backend).");
        }

        // [2/5] the raw device node + descriptors
        println!("[2/5] raw device node (/dev/bus/usb)");
        // rusb discovery finds bus/addr; the session transport holds MI_04
        // on the SAME node - the usbfs fd must be a SECOND open of it.
        let devices = idong::list_devices();
        let runtime = devices.iter().find(|d| d.state == idong::DeviceState::Runtime);
        let Some(dev) = runtime else {
            return AttemptOutcome::Gone("no 0471:2500 runtime device on the bus".into());
        };
        let node_path = format!("/dev/bus/usb/{:03}/{:03}", dev.bus, dev.address);
        println!("      opening {node_path} (runtime, bus {} addr {})", dev.bus, dev.address);
        let node = match OpenOptions::new().read(true).write(true).open(&node_path) {
            Ok(f) => f,
            Err(e) => {
                println!("      FAIL: open {node_path}: {e}");
                if e.kind() == io::ErrorKind::PermissionDenied {
                    println!("      permissions: the 42-idong.rules udev rule covers the");
                    println!("      /dev/bus/usb node too - is it installed? (replug after)");
                }
                std::process::exit(1);
            }
        };
        // The raw config descriptor: a standard GET_DESCRIPTOR(config)
        // control transfer on ep 0 needs no interface claim.
        let fd = node.as_raw_fd();
        let cfg_blob = match usbfs_iso::read_config_descriptor(fd) {
            Ok(b) => b,
            Err(e) => {
                println!("      FAIL: GET_DESCRIPTOR(config): {e}");
                std::process::exit(1);
            }
        };
        let alts = pure::walk_config(&cfg_blob);
        let vs = match pure::video_streaming_iface(&alts) {
            Some(vs) => vs,
            None => {
                println!("      FAIL: no UVC VideoStreaming interface in the config");
                std::process::exit(1);
            }
        };
        let iface = vs[0].number;
        println!("      config walked: {} interface alts", alts.len());
        for a in &vs {
            if let Some(e) = a.iso_in() {
                println!(
                    "      VS iface {} alt {} : ISO IN 0x{:02x} = {} B/microframe",
                    iface, a.alt, e.address, e.wmax_total
                );
            } else {
                println!("      VS iface {} alt {} : no endpoint", iface, a.alt);
            }
        }
        // the streaming alt: the auto pick (largest ISO IN bandwidth) or
        // the --alt override
        let auto_alt = pure::best_streaming_alt(&vs).expect("an ISO alt exists");
        let stream_alt: &pure::InterfaceAlt = match args.alt {
            None => auto_alt,
            Some(n) => match vs.iter().copied().find(|a| a.alt == n) {
                Some(a) if a.iso_in().is_some() => {
                    println!("      --alt {n} overrides the auto pick (alt {})", auto_alt.alt);
                    a
                }
                Some(_) => {
                    println!("      FAIL: --alt {n} has no ISO IN endpoint (alt 0 is the");
                    println!("      zero-bandwidth alt - use alt 1 or 2)");
                    std::process::exit(1);
                }
                None => {
                    let have: Vec<u8> = vs.iter().map(|a| a.alt).collect();
                    println!("      FAIL: --alt {n}: no such alt (VS alts: {have:?})");
                    std::process::exit(1);
                }
            },
        };
        let ep_addr = stream_alt.iso_in().expect("an ISO alt was checked").address;
        let packet_size = stream_alt.iso_in().map(|e| e.wmax_total).unwrap_or(0) as usize;
        println!("      streaming alt {} -> {} B per microframe", stream_alt.alt, packet_size);

        // [3/5] claim the VS interface + UVC negotiation
        println!("[3/5] DISCONNECT_CLAIM MI_{:02} + UVC probe/commit", iface);
        if let Err(e) = usbfs_iso::disconnect_claim(fd, iface, "uvcvideo") {
            println!("      FAIL: DISCONNECT_CLAIM: {e}");
            println!("      (is uvcvideo bound? try: ls /sys/class/video4linux)");
            std::process::exit(1);
        }
        println!("      PASS: MI_{:02} claimed away from uvcvideo (reversible on exit)", iface);
        let golden = ProbeCommit::GOLDEN;
        let probe_body = pure::probe_commit_body(&golden);
        for (cs, label) in [(pure::UVC_CS_PROBE, "probe"), (pure::UVC_CS_COMMIT, "commit")] {
            let req = pure::uvc_set_cur(iface, cs, &probe_body);
            match usbfs_iso::control_out(fd, &req, 5000) {
                Ok(_) => println!("      PASS: {label} SET_CUR sent ({} B)", req.length),
                Err(e) => {
                    println!("      FAIL: {label} SET_CUR: {e}");
                    println!("      (the firmware may reject the negotiation without a");
                    println!("      session - rerun with --cmos)");
                    std::process::exit(1);
                }
            }
        }
        // GET_CUR(probe) after the negotiation (turn 45): what the device
        // ACTUALLY settled on - the one number that decides the
        // frame-rate question if the device honors the interval. The
        // turn-44 runs measured 2.1 fps (ISO) vs 25 fps (uvcvideo) on
        // the same device the same hour; if GET_CUR(probe) answers with
        // an interval the ISO path never sent, the golden commit's
        // 333333 (30 fps nominal) is not what the firmware runs.
        let get_probe = pure::uvc_get_cur(iface, pure::UVC_CS_PROBE, 26);
        match usbfs_iso::control_in(fd, &get_probe, 5000) {
            Ok(body) => match ProbeCommit::parse(&body) {
                Some(pc) => {
                    println!(
                        "      GET_CUR(probe) reply: interval {} ({} fps), payload {} B",
                        pc.frame_interval,
                        pc.fps(),
                        pc.max_payload_transfer_size
                    );
                }
                None => println!("      GET_CUR(probe) reply: {} B, unparsable", body.len()),
            },
            Err(e) => {
                println!("      (GET_CUR probe: {e} - not fatal, firmware may not implement it)")
            }
        }

        // [4/5] SET_INTERFACE - the question the first bench run
        // decided (turn 43): the vendor capture streams with NO alt
        // change, but Linux arms an ISO endpoint only in the selected
        // alt setting - with the VS interface at alt 0 (no endpoints)
        // the host controller has no transfer ring for EP 0x85 and every
        // SUBMITURB fails with ENOENT (os error 2). Default: select the
        // streaming alt like any UVC host; --no-set-alt keeps the
        // capture-mirroring mode as the A/B control.
        println!("[4/5] SET_INTERFACE MI_{iface:02} -> alt {}", stream_alt.alt);
        let mut alt_set = false;
        if args.no_set_alt {
            println!("      SKIPPED (--no-set-alt: capture-mirroring mode - expect the");
            println!("      SUBMITURB ENOENT of the turn-43 run; A/B control only)");
        } else {
            match usbfs_iso::set_interface(fd, iface, stream_alt.alt) {
                Ok(_) => {
                    alt_set = true;
                    println!(
                        "      PASS: alt {} selected - ISO IN 0x{ep_addr:02x} armed ({} B/microframe)",
                        stream_alt.alt, packet_size
                    );
                }
                Err(e) => {
                    println!("      FAIL: SET_INTERFACE alt {}: {e}", stream_alt.alt);
                    if e.raw_os_error() == Some(pure::ERR_ENOSPC) {
                        println!("      (bus bandwidth - retry with --alt 1)");
                    }
                    println!("      collect dmesg | tail -50 - this decides P2.5b");
                    std::process::exit(1);
                }
            }
        }

        // [5/5] the ISO URB ring
        println!("[5/5] ISO URB ring on EP 0x{ep_addr:02x} ({} frames wanted)", args.frames);
        let npackets = 8usize; // 8 microframes per URB = 1 ms per re-submission
                               // ring depth: --ring N (turn 47). Run 11 ran 4 URBs = 4 ms of
                               // ISO buffer; two PNG encodes on the reap thread later the ring
                               // was dry for ~100 ms at a time and 86.8% of the device's
                               // frames never arrived. 16 URBs = 16 ms of bus buffer, and the
                               // per-frame work lives on the worker thread now anyway.
        let ring = args.ring;
        let mut urbs: Vec<usbfs_iso::IsoUrb> = Vec::new();
        for _ in 0..ring {
            match usbfs_iso::IsoUrb::new(ep_addr, npackets, packet_size) {
                Ok(u) => urbs.push(u),
                Err(e) => {
                    println!("      FAIL: URB alloc: {e}");
                    std::process::exit(1);
                }
            }
        }
        for u in urbs.iter_mut() {
            if let Err(e) = unsafe { u.submit(fd) } {
                println!("      FAIL: SUBMITURB: {e}");
                if let Some(hint) = pure::submiturb_hint(e.raw_os_error()) {
                    println!("      {hint}");
                }
                println!("      collect dmesg | tail -50 - this decides P2.5b");
                std::process::exit(1);
            }
        }
        println!("      PASS: {ring} URBs x {npackets} microframes submitted (ASAP, {ring} ms of ISO buffer)");
        println!("      output dir: {}", outdir.display());

        let mut assembler = VideoAssembler::new();
        let mut payloads = 0u64;
        let mut errs = 0u64;
        // per-URB packet status classification (the ISO ring's own
        // health line): -EPROTO 71 = babble/ CRC, -EOVERFLOW 75 = device
        // babble, -EILSEQ 84 = illegal byte sequence, 0 = ok. The
        // kernel reports these per packet in the ISO desc array.
        let mut pkt_proto = 0u64;
        let mut pkt_overflow = 0u64;
        let mut pkt_nilseq = 0u64;
        let mut attempt_frames = 0u32;
        let mut wall_first: Option<Instant> = None;
        let mut wall_last: Option<Instant> = None;
        let mut gone: Option<String> = None;
        // the frame worker (turn 47): the reap loop copies packet bytes,
        // resubmits, assembles and hands closed frames over - the stats,
        // console lines and PNG/bin writes happen on the worker thread,
        // NEVER between a reap and its resubmit.
        let sink = FrameSink::spawn(
            outdir.to_path_buf(),
            true,
            args.rate_probe.is_some(),
            args.save.clone(),
        );
        let t_start = Instant::now();
        'outer: while st.got + attempt_frames < args.frames
            && t_start.elapsed() < Duration::from_secs(30)
        {
            // reap one URB (non-blocking) + poll on the fd
            let reaped = match usbfs_iso::wait_reap(fd, 100) {
                Ok(Some(p)) => p,
                Ok(None) => continue, // nothing reaped yet
                Err(e) => {
                    gone = Some(format!("REAPURB: {e}"));
                    break 'outer;
                }
            };
            // find which of our URBs was reaped (the returned pointer)
            let idx = urbs
                .iter()
                .position(|u| std::ptr::eq(u.ptr() as *mut u8, reaped as *mut u8))
                .expect("reaped URB belongs to the ring");
            let results = unsafe { urbs[idx].packet_results() };
            // MEASURE-FIRST (turn 45): copy the reaped packets out and
            // resubmit the URB BEFORE any frame work. The old order let
            // the ring drain during stats + PNG writes - a reaped ring
            // slot was empty for whole milliseconds while the device
            // kept producing; ASAP scheduling then jumps the next URB
            // to the far future and frames are missed on the bus (never
            // visible in any error counter - the payloads simply never
            // arrive). Copying 8 x 2412 B is microseconds; PNGs are not
            // - which is why they run on the worker thread now too.
            let mut pkt_bytes: Vec<(usize, Vec<u8>)> = Vec::with_capacity(npackets);
            for (i, (actual, status)) in results.iter().enumerate() {
                if *status != 0 {
                    errs += 1;
                    // classify the nonzero statuses (Linux ABI digits)
                    match *status {
                        71 => pkt_proto += 1,    // -EPROTO
                        75 => pkt_overflow += 1, // -EOVERFLOW (babble)
                        84 => pkt_nilseq += 1,   // -EILSEQ
                        _ => {}
                    }
                    continue;
                }
                if *actual == 0 {
                    continue;
                }
                let bytes = unsafe { urbs[idx].packet_bytes(i, *actual) }.to_vec();
                pkt_bytes.push((i, bytes));
            }
            // the URB is free now: back into the ring before processing
            if let Err(e) = unsafe { urbs[idx].submit(fd) } {
                if let Some(hint) = pure::submiturb_hint(e.raw_os_error()) {
                    println!("      re-submit failed: {e}\n      {hint}");
                } else {
                    println!("      re-submit failed: {e}");
                }
                gone = Some(format!("resubmit: {e}"));
                break 'outer;
            }
            // feed each packet as one UVC payload: the URB layout gives
            // per-packet boundaries directly (the capture's ISO-record
            // walk needed the 2412-B stride because USBPcap flattens all
            // packets of one URB; usbfs hands them to us separated)
            payloads += pkt_bytes.len() as u64;
            for (_, bytes) in &pkt_bytes {
                if let Some(frame) = assembler.push_payload(bytes) {
                    let now = Instant::now();
                    wall_first.get_or_insert(now);
                    wall_last = Some(now);
                    sink.send(FrameJob {
                        no: st.got + attempt_frames,
                        pts: Some(frame.pts),
                        seq: None,
                        ts: None,
                        bytes: frame.data,
                    });
                    attempt_frames += 1;
                    if st.got + attempt_frames >= args.frames {
                        break 'outer;
                    }
                }
            }
        }

        // stop the ring before touching the interface state: discard
        // every still-submitted URB and drain the reaps (unlink is
        // asynchronous), then return the VS interface to alt 0 (the
        // standard UVC teardown - frees the ISO bandwidth) and release
        // the claim so uvcvideo can rebind (the kernel re-probes on the
        // next open of /dev/video*). After a self-reboot the node is a
        // ghost - every step is best-effort then.
        usbfs_iso::discard_and_drain(fd, &mut urbs, 200);
        if alt_set {
            match usbfs_iso::set_interface(fd, iface, 0) {
                Ok(_) => println!("      teardown: VS alt 0 restored (ISO bandwidth freed)"),
                Err(e) => {
                    println!("      teardown: alt 0 restore: {e} (ignored - replug resets it)")
                }
            }
        }
        let _ = usbfs_iso::release_interface(fd, iface);

        // drain the worker and merge its session tally
        let tally = sink.finish();
        if attempt_frames > 0 {
            st.sessions += 1;
            if let (Some(w0), Some(w1)) = (wall_first, wall_last) {
                st.wall_active += w1.duration_since(w0);
            }
        }
        merge_tally(st, tally);

        println!();
        println!(
            "payloads        : {payloads} ({} errors: {} proto, {} overflow, {} ilseq)",
            errs, pkt_proto, pkt_overflow, pkt_nilseq
        );
        if let Some(e) = gone {
            return AttemptOutcome::Gone(e);
        }
        AttemptOutcome::Done
    }

    /// Pool two OPTIONAL plane stats (turn 47): the worker returns
    /// per-session Option<PlaneStats>; the run state pools them.
    fn merge_opt(prev: Option<PlaneStats>, other: Option<PlaneStats>) -> Option<PlaneStats> {
        match (prev, other) {
            (None, o) => o,
            (p, None) => p,
            (Some(p), Some(o)) => merge(Some(p), o),
        }
    }

    /// Fold one session's worker tally into the run state (turn 47):
    /// the per-session numbers stay separate from the run totals -
    /// delivered frames add up, PTS spans and active wall spans sum,
    /// the deltas append in order. What the tally counted as drops is
    /// INTRA-session only (the device was off during the reboots - run
    /// 11's "1675 drops" wrongly included ~1000 reboot-gap frames).
    fn merge_tally(st: &mut RunState, t: SessionTally) {
        st.got += t.delivered;
        st.any_a = merge_opt(st.any_a, t.any_a);
        st.any_b = merge_opt(st.any_b, t.any_b);
        st.drops += t.dropped;
        st.made += t.delivered as u64 + t.dropped;
        st.pts_deltas_fr.extend_from_slice(&t.pts_deltas_fr);
        if t.last_report.is_some() {
            st.last_report = t.last_report;
        }
        if let (Some(first), Some(last)) = (t.first_pts, t.last_pts) {
            if last >= first {
                st.pts_span_ticks += (last - first) as u64;
            } else {
                // a 32-bit PTS wrap inside one session: wrapping span
                st.pts_span_ticks += last.wrapping_sub(first) as u64;
            }
        }
        if t.pngs_skipped > 0 {
            println!("      (PNGs skipped for {} frames - the worker fell behind;", t.pngs_skipped);
            println!("       --rate keeps the ring full for pure measurement)");
        }
    }

    /// The offline channel analysis (turn 47): `--analyze DIR` walks the
    /// frameNNNN.bin files a run left behind and prints the A/B
    /// structure analysis - the run-11 hand-cover questions (is B an
    /// image? how does B track the scene?) answered from the bytes, no
    /// device needed.
    fn analyze_dir(dir: &Path) {
        println!("=== idong P2.6a channel analysis (offline, no device) ===");
        println!("      dir: {}", dir.display());
        let mut bins: Vec<PathBuf> = match fs::read_dir(dir) {
            Ok(rd) => rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("frame") && n.ends_with(".bin"))
                        .unwrap_or(false)
                })
                .collect(),
            Err(e) => {
                println!("      FAIL: read_dir {}: {e}", dir.display());
                std::process::exit(1);
            }
        };
        bins.sort();
        if bins.is_empty() {
            println!("      FAIL: no frameNNNN.bin files in {}", dir.display());
            println!("      (a capture run writes them for its first 3 frames and");
            println!("       every --save N frame)");
            std::process::exit(1);
        }
        println!();
        let mut pooled_a: Option<PlaneStats> = None;
        let mut pooled_b: Option<PlaneStats> = None;
        let mut b_image_votes = 0u32;
        let mut max_b_sd = 0.0f64;
        for p in &bins {
            let bytes = match fs::read(p) {
                Ok(b) => b,
                Err(e) => {
                    println!("      (skip {}: {e})", p.display());
                    continue;
                }
            };
            let r = idong::protocol::video::channel_report(&bytes);
            println!(
                "      {}: {} B  chA mean {:6.1} sd {:5.1} rows[{:5.1}..{:5.1}]  chB mean {:6.1} sd {:5.1} rows[{:5.1}..{:5.1}]  corr {:+.3}  B: {}",
                p.file_name().unwrap_or_default().to_string_lossy(),
                bytes.len(),
                r.a.mean,
                r.a.stddev,
                r.a_rows.min,
                r.a_rows.max,
                r.b.mean,
                r.b.stddev,
                r.b_rows.min,
                r.b_rows.max,
                r.corr_ab,
                r.b_verdict()
            );
            pooled_a = merge(pooled_a, r.a);
            pooled_b = merge(pooled_b, r.b);
            if r.b_is_image() {
                b_image_votes += 1;
            }
            max_b_sd = max_b_sd.max(r.b.stddev);
        }
        println!();
        println!("frames analyzed : {}", bins.len());
        if let Some(pa) = pooled_a {
            println!(
                "channel A      : mean {:.1} sd {:.1} [{},{}] over {} samples",
                pa.mean, pa.stddev, pa.min, pa.max, pa.n
            );
        }
        if let Some(pb) = pooled_b {
            println!(
                "channel B      : mean {:.1} sd {:.1} [{},{}] over {} samples",
                pb.mean, pb.stddev, pb.min, pb.max, pb.n
            );
        }
        println!(
            "B image votes   : {b_image_votes}/{} (pixel sd >= 12 AND row spread >= 0.4)",
            bins.len()
        );
        if b_image_votes == 0 {
            println!("verdict         : channel B carries NO image - A is the only image plane.");
            println!("                  (the marker/3D plane lives on the MI_04 record pair");
            println!("                  0x83/0x03 per the turn-46 disassembly - records --cmos)");
        } else {
            println!(
                "verdict         : {b_image_votes} frame(s) carry a B image - rerun --save on"
            );
            println!("                  those frames and compare with the A PNGs");
        }
        if max_b_sd >= 12.0 && b_image_votes == 0 {
            println!("note            : B sd peaked at {max_b_sd:.1} (scene-coupled noise - the");
            println!("                  run-11 bright frames read B sd up to ~19; sd alone is not");
            println!("                  an image without row structure)");
        }
    }

    // ---------------- P2.4b: the self-reboot recovery ----------------

    /// Load the pinned 8051 image for the recovery path (both SHA-256
    /// pins, nothing opened). Prints its own failures.
    fn load_recovery_image() -> Option<Vec<u8>> {
        let (hex_path, hex) = match idong::firmware::read_hex(None) {
            Ok(read) => read,
            Err(e) => {
                println!("      FAIL: {e}");
                return None;
            }
        };
        println!("      hex file    : {} ({} B)", hex_path.display(), hex.len());
        if let Err(e) = idong::FirmwareSet::verify_hex(&hex) {
            println!("      FAIL: {e}");
            return None;
        }
        let fs = idong::FirmwareSet { ldr: Vec::new(), bit: Vec::new(), hex };
        match fs.mcu_image() {
            Ok(image) => Some(image),
            Err(e) => {
                println!("      FAIL: {e}");
                None
            }
        }
    }

    /// The P2.4b recovery: the sensor self-rebooted (it does, every
    /// 31-157 s). Wait for the 0471:2600 bootloader landing, re-flash
    /// the pinned image (the ungated FLASH_CONFIRMED path - the exact
    /// vendor sequence, readback-verified before the commit key; T8
    /// PASS x2 on the turn-41 bench), wait for the 2500
    /// re-enumeration. True = a runtime device is on the bus again.
    fn recover_after_reboot(image: &[u8]) -> bool {
        use idong::transport::reenum::wait_runtime;

        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            let current = idong::list_devices();
            let pids: Vec<u16> = current.iter().map(|d| d.pid).collect();
            match idong::bus_state(&pids) {
                idong::BusState::Bootloader => break,
                idong::BusState::Runtime => {
                    // came back by itself (not the observed shape - the
                    // firmware stays in the bootloader until flashed -
                    // but resuming is correct either way)
                    println!("      recovery: a runtime device is on the bus - resuming");
                    return true;
                }
                _ => {}
            }
            if Instant::now() >= deadline {
                println!("      FAIL: no sensor on the bus within 180 s - replug and rerun");
                return false;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        println!("      recovery: bootloader landing - re-flashing (the vendor sequence)");
        // open the bootloader WITH retry (turn 47): a fresh
        // re-enumeration can briefly appear before udev applies the
        // node's permissions - run 11's 9th landing failed the whole
        // recovery with "Access denied" on the FIRST open; the eight
        // before it opened fine. Retry while the bootloader is still
        // on the bus; anything else keeps the old single-attempt fate.
        let mut t = None;
        for attempt in 0..5u32 {
            match idong::UsbTransport::open_bootloader() {
                Ok(t0) => {
                    t = Some(t0);
                    break;
                }
                Err(e) => {
                    let still_bootloader = {
                        let pids: Vec<u16> = idong::list_devices().iter().map(|d| d.pid).collect();
                        matches!(idong::bus_state(&pids), idong::BusState::Bootloader)
                    };
                    if attempt == 4 || !still_bootloader {
                        println!("      recovery: open bootloader: {e}");
                        if !still_bootloader {
                            println!("      (the bootloader left the bus - unplugged?)");
                        }
                        return false;
                    }
                    println!(
                        "      recovery: open bootloader: {e} - retrying ({} of 4, udev settle)",
                        attempt + 1
                    );
                    std::thread::sleep(Duration::from_millis(1000));
                }
            }
        }
        let Some(mut t) = t else { return false };
        match idong::protocol::runtime::RuntimeSession::new(&mut t).flash_mcu(image) {
            Ok(_) => println!("      recovery: flash ok (readback matched), launched"),
            Err(e) => {
                println!("      recovery: flash FAILED: {e}");
                println!("      (the readback aborts BEFORE the commit key - rerun)");
                return false;
            }
        }
        drop(t); // release the interface: the device re-enumerates now
        match wait_runtime(Duration::from_secs(30), Duration::from_millis(500)) {
            Ok(d) => {
                println!(
                    "      recovery: runtime re-enumerated (bus {}, addr {})",
                    d.bus, d.address
                );
                true
            }
            Err(e) => {
                println!("      recovery: {e}");
                false
            }
        }
    }

    /// Pool two plane stats, WEIGHTED by sample count (turn 44: the
    /// bench summary averaged per-frame means pair-wise, so a 1200-B
    /// tail frame outweighed its 612000-B sibling and reported a bogus
    /// pooled mean).
    fn merge(prev: Option<PlaneStats>, s: PlaneStats) -> Option<PlaneStats> {
        Some(match prev {
            None => s,
            Some(p) => {
                let n = p.n + s.n;
                let (wp, ws) =
                    if n > 0 { (p.n as f64 / n as f64, s.n as f64 / n as f64) } else { (0.5, 0.5) };
                PlaneStats {
                    mean: p.mean * wp + s.mean * ws,
                    min: p.min.min(s.min),
                    max: p.max.max(s.max),
                    stddev: p.stddev * wp + s.stddev * ws,
                    n,
                }
            }
        })
    }

    /// A minimal PixFormat for the PNG writer (640x480, stride 1280).
    fn pix_for() -> uapi::PixFormat {
        uapi::PixFormat {
            width: idong::protocol::video::RASTER_WIDTH as u32,
            height: 480,
            pixelformat: 0,
            field: 0,
            bytesperline: idong::protocol::video::RASTER_STRIDE as u32,
            sizeimage: 614400,
            colorspace: 0,
            priv_: 0,
            flags: 0,
            ycbcr_enc: 0,
            quantization: 0,
            xfer_func: 0,
        }
    }

    /// The kit-37 runN rule: first nonexistent or EMPTY folder.
    fn next_run_dir() -> PathBuf {
        for n in 1..=999 {
            let p = PathBuf::from(format!("video_run{n}"));
            let empty = fs::read_dir(&p).map(|mut d| d.next().is_none()).unwrap_or(true);
            if !p.exists() || empty {
                return p;
            }
        }
        PathBuf::from("video_out")
    }

    fn write_plane_png(path: &Path, plane: &[u8], pix: &uapi::PixFormat) {
        let stride =
            if pix.bytesperline > 0 { pix.bytesperline as usize } else { (pix.width as usize) * 2 };
        if stride == 0 || stride % 2 != 0 {
            eprintln!("warning: odd stride {stride} - {} not written", path.display());
            return;
        }
        let w = (stride / 2) as u32;
        let h = (plane.len() / (stride / 2)) as u32;
        if w == 0 || h == 0 {
            eprintln!("warning: empty plane - {} not written", path.display());
            return;
        }
        let file = match File::create(path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("warning: {}: {e}", path.display());
                return;
            }
        };
        let mut enc = png::Encoder::new(io::BufWriter::new(file), w, h);
        enc.set_color(png::ColorType::Grayscale);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = match enc.write_header() {
            Ok(wr) => wr,
            Err(e) => {
                eprintln!("warning: {}: {e}", path.display());
                return;
            }
        };
        if let Err(e) = writer.write_image_data(&plane[..(w * h) as usize]) {
            eprintln!("warning: {}: {e}", path.display());
        }
    }
}

#[cfg(all(feature = "usb", target_os = "linux"))]
fn main() {
    bench::run();
}

#[cfg(not(all(feature = "usb", target_os = "linux")))]
fn main() {
    eprintln!("the P2.5 video test runs on Linux with the usb feature:");
    eprintln!("  cargo run --features usb --example video -- --probe");
    eprintln!("  cargo run --features usb --example video -- --cmos --frames 10");
    eprintln!("  cargo run --features usb --example video -- --cmos --frames 10 --recover");
}
