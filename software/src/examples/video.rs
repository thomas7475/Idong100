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
//! ```
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

    use idong::protocol::video::{
        plane_stats, split_channels, PlaneStats, VideoAssembler, VideoFrame,
    };
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
                    "--frames" | "--device" | "--outdir" | "--alt" => {
                        let val = a.get(i + 1).cloned();
                        match (a[i].as_str(), val) {
                            ("--frames", Some(v)) => {
                                args.frames = v.parse().unwrap_or(10).clamp(1, 300)
                            }
                            ("--device", Some(v)) => args.device = Some(v),
                            ("--outdir", Some(v)) => args.outdir = Some(v),
                            ("--alt", Some(v)) => match v.parse::<u8>() {
                                Ok(n) => args.alt = Some(n),
                                Err(_) => eprintln!("warning: --alt {v:?} is not 0-255"),
                            },
                            _ => eprintln!("warning: {} needs a value", a[i]),
                        }
                        i += 1;
                    }
                    other => eprintln!("warning: unknown flag {other:?}"),
                }
                i += 1;
            }
            args
        }
    }

    fn cstr(bytes: &[u8]) -> String {
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).to_string()
    }

    /// What survives a self-reboot recovery: the output folder, the
    /// frame counter, the pooled plane stats and the per-attempt rate
    /// measurement. Everything else (session, transports, URB ring,
    /// video node) dies with the device and is rebuilt per attempt.
    #[derive(Default)]
    struct RunState {
        got: u32,
        any_a: Option<PlaneStats>,
        any_b: Option<PlaneStats>,
        /// (frames_in_span, span_us) of the last attempt - the fps line.
        rate: Option<(u32, f64)>,
        /// ISO: the PTS of the last closed frame, for cadence deltas.
        last_pts: Option<u32>,
        first_pts: Option<u32>,
        pts_deltas_fr: Vec<f64>,
        recoveries: u32,
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
        if args.probe {
            probe(&args);
            return;
        }
        // the P2.4b recovery loop (turn 44): each attempt is a full
        // session + backend setup; --recover absorbs the self-reboots
        // between attempts. Without it the behavior is exactly the old
        // one: a gone device ends the run with the summary of what was
        // captured.
        let mut st = RunState::default();
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
                format!(" ({} self-reboot recovery/recoveries)", st.recoveries)
            } else {
                String::new()
            }
        );
        if args.iso {
            if let (Some(first), Some(last)) = (st.first_pts, st.last_pts) {
                let deltas = &st.pts_deltas_fr;
                if !deltas.is_empty() {
                    let joined: Vec<String> = deltas.iter().map(|d| format!("{d:.1}")).collect();
                    // device-clock seconds between the first and last
                    // frame PTS: the device-side production rate
                    let span_s = last.wrapping_sub(first) as f64
                        / idong::protocol::video::DEVICE_CLOCK_HZ as f64;
                    let fps = if span_s > 0.0 { (st.got - 1) as f64 / span_s } else { 0.0 };
                    println!(
                        "pts cadence     : deltas {} frame periods (125 ms base)",
                        joined.join("/")
                    );
                    println!("device rate     : {fps:.1} fps over the captured span (pts clock)");
                }
            }
        } else if let Some((n, span_us)) = st.rate {
            if n > 1 && span_us > 0.0 {
                let fps = (n - 1) as f64 * 1_000_000.0 / span_us;
                println!("measured rate   : {fps:.1} fps (capture timestamps, last attempt)");
            }
        }
        println!(
            "session         : {}",
            if args.cmos { "held (MI_04 + 0x0205 polls)" } else { "none (--cmos absent)" }
        );
        println!("channel A stats : {:?}", st.any_a);
        println!("channel B stats : {:?}", st.any_b);
        if let Some(a) = st.any_a {
            if a.has_contrast() {
                println!("PASS: channel A carries a real image (contrast present).");
            } else {
                println!("WARN: channel A is flat - registers not initialized? (use --cmos)");
            }
        }
        if let Some(a) = st.any_a {
            if a.n > 0 {
                println!("PASS: {} channel-A samples pooled.", a.n);
            }
        }
        println!("output: {} (PNGs per channel + raw .bin of the first frames)", outdir.display());
        println!("next steps:");
        println!("  - send the PNGs + this console output back");
        println!("  - the long-run soak: add --recover and --frames 60+");
        println!("  - if channel B decodes as an image too, the marker/3D plane is next");
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
        let mut dev = match V4l2Dev::open(&node) {
            Ok(d) => d,
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound || e.raw_os_error() == Some(libc_enodev()) {
                    return AttemptOutcome::Gone(format!("open {}: {e}", node.display()));
                }
                println!("      FAIL: cannot open {}: {e}", node.display());
                if e.kind() == io::ErrorKind::PermissionDenied {
                    println!("      permissions: sudo usermod -aG video $USER, then re-login");
                }
                std::process::exit(1);
            }
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
        let mut t_last_poll = Instant::now() - Duration::from_secs(1);
        let mut poll_errors = 0u32;
        let mut polls_live = true;
        let mut first_ts: Option<i64> = None;
        let mut last_ts: Option<i64> = None;
        let mut attempt_frames = 0u32;

        while st.got < args.frames && t_start.elapsed() < Duration::from_secs(30) {
            // keep the 0x0205 stream poll alive (~10 Hz) while streaming,
            // exactly the vendor pattern (30 Hz on the wire); stop after
            // three consecutive unanswered polls (it is not required for
            // V4L2 streaming - the vendor engine sends it, we observe).
            if polls_live
                && session.is_some()
                && t_last_poll.elapsed() >= Duration::from_millis(100)
            {
                match session.as_mut().unwrap().poll_stream() {
                    Ok(st8) => {
                        poll_errors = 0;
                        if st8.new_frame {
                            println!(
                                "      0x0205: new frame #{} (frame counter advanced)",
                                st8.frame_no
                            );
                        }
                    }
                    Err(_) => {
                        poll_errors += 1;
                        if poll_errors >= 3 {
                            polls_live = false;
                            println!("      0x0205: unanswered x3 - stream polls off (not fatal)");
                        }
                    }
                }
                t_last_poll = Instant::now();
            }
            match dev.dq(500) {
                Ok(Some((idx, bytesused, seq, ts))) => {
                    let bytes = dev.buffer_bytes(idx, bytesused);
                    let (a, b) = split_channels(bytes);
                    let sa = plane_stats(&a);
                    let sb = plane_stats(&b);
                    println!(
                        "      frame {:3}: seq {:3} {} B  chA mean {:6.1} sd {:5.1} [{:3},{:3}]  chB mean {:6.1} sd {:5.1} [{:3},{:3}]",
                        st.got, seq, bytesused, sa.mean, sa.stddev, sa.min, sa.max, sb.mean, sb.stddev, sb.min, sb.max
                    );
                    if st.got < 3 {
                        let raw = outdir.join(format!("frame{:04}.bin", st.got));
                        let _ = fs::write(&raw, bytes);
                    }
                    write_plane_png(&outdir.join(format!("frame{:04}_chA.png", st.got)), &a, &pix);
                    write_plane_png(&outdir.join(format!("frame{:04}_chB.png", st.got)), &b, &pix);
                    st.any_a = merge(st.any_a, sa);
                    st.any_b = merge(st.any_b, sb);
                    first_ts.get_or_insert(ts);
                    last_ts = Some(ts);
                    st.got += 1;
                    attempt_frames += 1;
                    if let Err(e) = dev.queue(idx) {
                        println!("      FAIL: requeue: {e}");
                        break;
                    }
                }
                Ok(None) => continue,
                Err(e) => {
                    dev.stream_off();
                    return AttemptOutcome::Gone(format!("DQBUF: {e}"));
                }
            }
        }
        dev.stream_off();
        if attempt_frames > 1 {
            let span_us = (last_ts.unwrap_or(0) - first_ts.unwrap_or(0)).max(1) as f64;
            st.rate = Some((attempt_frames, span_us));
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
        let ring = 4usize; // four URBs in flight
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
        println!("      PASS: {ring} URBs x {npackets} microframes submitted (ASAP)");
        println!("      output dir: {}", outdir.display());

        let mut assembler = VideoAssembler::new();
        let mut payloads = 0u64;
        let mut errs = 0u64;
        let mut t_last_poll = Instant::now() - Duration::from_secs(1);
        let mut poll_errors = 0u32;
        let mut polls_live = true;
        let mut gone: Option<String> = None;
        let t_start = Instant::now();
        'outer: while st.got < args.frames && t_start.elapsed() < Duration::from_secs(30) {
            // keep the 0x0205 stream poll alive (~10 Hz) as in the V4L2 run
            if polls_live
                && session.is_some()
                && t_last_poll.elapsed() >= Duration::from_millis(100)
            {
                match session.as_mut().unwrap().poll_stream() {
                    Ok(_) => poll_errors = 0,
                    Err(_) => {
                        poll_errors += 1;
                        if poll_errors >= 3 {
                            polls_live = false;
                            println!("      0x0205: unanswered x3 - stream polls off (not fatal)");
                        }
                    }
                }
                t_last_poll = Instant::now();
            }
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
            // feed each packet as one UVC payload: the URB layout gives
            // per-packet boundaries directly (the capture's ISO-record
            // walk needed the 2412-B stride because USBPcap flattens all
            // packets of one URB; usbfs hands them to us separated)
            for (i, (actual, status)) in results.iter().enumerate() {
                if *status != 0 {
                    errs += 1;
                    continue;
                }
                if *actual == 0 {
                    continue;
                }
                let bytes = unsafe { urbs[idx].packet_bytes(i, *actual) };
                payloads += 1;
                if let Some(frame) = assembler.push_payload(bytes) {
                    report_iso_frame(st, &frame, outdir);
                    if st.got >= args.frames {
                        break 'outer;
                    }
                }
            }
            // re-submit the reaped URB
            if let Err(e) = unsafe { urbs[idx].submit(fd) } {
                if let Some(hint) = pure::submiturb_hint(e.raw_os_error()) {
                    println!("      re-submit failed: {e}\n      {hint}");
                } else {
                    println!("      re-submit failed: {e}");
                }
                gone = Some(format!("resubmit: {e}"));
                break 'outer;
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

        println!();
        println!("payloads        : {payloads} ({} errors)", errs);
        if let Some(e) = gone {
            return AttemptOutcome::Gone(e);
        }
        AttemptOutcome::Done
    }

    /// Report one closed ISO frame: stats, delta to the previous PTS
    /// (in 125-ms frame periods - the device clock), PNGs, raw bytes.
    fn report_iso_frame(st: &mut RunState, frame: &VideoFrame, outdir: &Path) {
        let (a, b) = split_channels(&frame.data);
        let sa = plane_stats(&a);
        let sb = plane_stats(&b);
        let dt = match st.last_pts {
            Some(prev) => format!(
                " (dt {:4.1} fr)",
                frame.pts.wrapping_sub(prev) as f64 / idong::protocol::video::PTS_PER_FRAME as f64
            ),
            None => String::new(),
        };
        println!(
            "      frame {:3}: pts {}{dt} {} B  chA mean {:6.1} sd {:5.1}  chB mean {:6.1} sd {:5.1}",
            st.got, frame.pts, frame.data.len(), sa.mean, sa.stddev, sb.mean, sb.stddev
        );
        if st.got < 3 {
            let _ = fs::write(outdir.join(format!("frame{:04}.bin", st.got)), &frame.data);
        }
        let pix = pix_for();
        write_plane_png(&outdir.join(format!("frame{:04}_chA.png", st.got)), &a, &pix);
        write_plane_png(&outdir.join(format!("frame{:04}_chB.png", st.got)), &b, &pix);
        if let Some(prev) = st.last_pts {
            st.pts_deltas_fr.push(
                frame.pts.wrapping_sub(prev) as f64 / idong::protocol::video::PTS_PER_FRAME as f64,
            );
        }
        st.first_pts.get_or_insert(frame.pts);
        st.last_pts = Some(frame.pts);
        st.any_a = merge(st.any_a, sa);
        st.any_b = merge(st.any_b, sb);
        st.got += 1;
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
        let mut t = match idong::UsbTransport::open_bootloader() {
            Ok(t) => t,
            Err(e) => {
                println!("      recovery: open bootloader: {e}");
                return false;
            }
        };
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
