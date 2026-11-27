//! P2.5 video bring-up on Linux - the UVC plane of the 0471:2500
//! composite (turn 38). THE hardware test of the video backend.
//!
//! ```sh
//! cargo run --features usb --example video -- --probe
//! cargo run --features usb --example video -- --cmos --frames 10
//! cargo run --features usb --example video -- --iso --cmos --frames 10
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
//! * `--iso`          the P2.5b raw usbfs-ISO backend (turn 42): take
//!                    the VS interface from uvcvideo, run the observed
//!                    probe/commit, stream EP 0x85 directly. Use this
//!                    when V4L2 STREAMON fails (T7 bench, EIO).
//! * `--device PATH`  use this /dev/videoN instead of the sysfs
//!                    auto-match (src/transport/v4l2.rs). Ignored with
//!                    --iso.
//! * `--frames N`    stop after N frames (default 10).
//! * `--outdir PATH` output folder (default: video_runN - the first
//!                    nonexistent or EMPTY one, the kit-37 runN rule).
//!
//! Why V4L2: on Linux the uvcvideo kernel driver claims the sensor's UVC
//! VideoStreaming interface (MI_01) and does the probe/commit negotiation
//! itself; the vendor interface MI_04 stays free for the session transport
//! - so ONE process can run the session (rusb) and the video (V4L2)
//! side by side, exactly like the vendor engine on Windows.
//!
//! Why --iso (P2.5b): the turn-41 bench run showed uvcvideo cannot
//! STREAMON this firmware (EIO at VIDIOC_STREAMON, both formats and
//! formats fine, node + 640x480 YUYV negotiated). The vendor app itself
//! never issues SET_INTERFACE alt>0 - video flows on EP 0x85 with the
//! interface untouched (turn-35 capture) - so the hypothesis is the
//! firmware streams whenever ISO URBs are posted. The --iso path opens
//! /dev/bus/usb/<bus>/<addr>, DISCONNECT_CLAIMs the VS interface from
//! uvcvideo, runs the observed probe/commit SET_CURs, and submits ISO
//! URBs on EP 0x85 with ASAP scheduling - no kernel UVC driver in the
//! way. If THAT streams, P2.5 is accepted and the backend is promoted
//! into the crate; if it fails, dmesg + the errno decide the next move.
//!
//! What PASS looks like (the turn-35 capture predicts): 640x480 frames,
//! 614400 B each, ~8 fps, a two-channel byte-interleaved raster - channel
//! A (even bytes) the scene image with real contrast, channel B (odd
//! bytes) the second, low-contrast channel. Both are saved as grayscale
//! PNGs plus the raw interleaved bytes for analysis.
//!
//! What a FAIL means (and what it decides):
//! * no /dev/video* node for the sensor at all -> uvcvideo did not bind
//!   (dmesg); the raw usbfs-ISO backend (P2.5b, EP 0x85 directly) is the
//!   fallback path.
//! * ENUM_FMT empty / S_FMT rejects 640x480 -> the descriptors expose no
//!   usable format; same fallback.
//! * node + format fine but no frames -> the firmware may stream only
//!   under the full session (rerun with --cmos) or only while the 0x0205
//!   poll runs (this test sends it at 10 Hz while streaming).
//!
//! The V4L2 ioctl layer is a UAPI mirror verified against
//! /usr/include/linux/videodev2.h (kernel 5.10+ layouts; every struct
//! size and ioctl number is const-asserted - see `mod uapi`), kept here
//! in the example until the first successful bench run promotes it
//! into the crate behind a feature (P2.5 acceptance). The crate itself
//! stays 100% safe code.

#[cfg(all(feature = "usb", target_os = "linux"))]
mod bench {
    use std::fs::{self, File, OpenOptions};
    use std::io;
    use std::os::fd::AsRawFd;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use idong::protocol::video::{plane_stats, PlaneStats};
    use idong::protocol::{PID_BOOTLOADER, PID_RUNTIME, VID};
    use idong::transport::v4l2::{
        fourcc_str, iface_belongs_to_port, sensor_nodes, sensor_usb_ports, video_nodes,
    };
    use idong::{Session, UsbTransport};

    // ---- V4L2 UAPI mirror (layout-verified, see module docs) ----
    mod uapi {
        use core::mem::{offset_of, size_of};
        use std::io;

        pub const IOC_WRITE: u32 = 1;
        pub const IOC_READ: u32 = 2;
        pub const IOC_RW: u32 = IOC_READ | IOC_WRITE;

        /// The Linux `_IOC(dir, type, nr, size)` request encoder.
        pub const fn ioc(dir: u32, ty: u8, nr: u8, size: usize) -> u32 {
            (dir << 30) | ((size as u32) << 16) | ((ty as u32) << 8) | nr as u32
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct Capability {
            pub driver: [u8; 16],
            pub card: [u8; 32],
            pub bus_info: [u8; 32],
            pub version: u32,
            pub capabilities: u32,
            pub device_caps: u32,
            pub reserved: [u32; 3],
        }

        /// 12 x u32 = 48 B (kernel 4.11+ layout).
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct PixFormat {
            pub width: u32,
            pub height: u32,
            pub pixelformat: u32,
            pub field: u32,
            pub bytesperline: u32,
            pub sizeimage: u32,
            pub colorspace: u32,
            pub priv_: u32,
            pub flags: u32,
            pub ycbcr_enc: u32,
            pub quantization: u32,
            pub xfer_func: u32,
        }

        /// Kernel union: 200 B, alignment 8 (v4l2_window carries a
        /// pointer) - the `[u64; 25]` member pins both facts.
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub union FormatFmt {
            pub pix: PixFormat,
            pub raw: [u8; 200],
            #[allow(dead_code)]
            align: [u64; 25],
        }

        /// type (4) + pad (4) + union (200) = 208 B.
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct Format {
            pub type_: u32,
            pub fmt: FormatFmt,
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct FmtDesc {
            pub index: u32,
            pub type_: u32,
            pub flags: u32,
            pub description: [u8; 32],
            pub pixelformat: u32,
            pub mbus_code: u32,
            pub reserved: [u32; 3],
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        pub union FrmSize {
            pub discrete: FrmsizeDiscrete,
            pub stepwise: FrmsizeStepwise,
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct FrmsizeDiscrete {
            pub width: u32,
            pub height: u32,
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct FrmsizeStepwise {
            pub min_width: u32,
            pub max_width: u32,
            pub step_width: u32,
            pub min_height: u32,
            pub max_height: u32,
            pub step_height: u32,
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct FrmSizeEnum {
            pub index: u32,
            pub pixel_format: u32,
            pub type_: u32,
            pub size: FrmSize,
            pub reserved: [u32; 2],
        }

        /// count,type,memory,capabilities (16) + flags u8 + reserved[3]
        /// = 20 B (kernel 5.x layout).
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct RequestBuffers {
            pub count: u32,
            pub type_: u32,
            pub memory: u32,
            pub capabilities: u32,
            pub flags: u8,
            pub reserved: [u8; 3],
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        pub union BufferM {
            pub offset: u32,
            #[allow(dead_code)]
            userptr: u64,
            #[allow(dead_code)]
            planes: *mut u8,
            #[allow(dead_code)]
            fd: i32,
        }

        /// 88 B on LP64 (timestamp at 24, m at 64, length at 72) -
        /// const-asserted below against the UAPI.
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct Buffer {
            pub index: u32,
            pub type_: u32,
            pub bytesused: u32,
            pub flags: u32,
            pub field: u32,
            pub timestamp: libc::timeval,
            pub timecode: Timecode,
            pub sequence: u32,
            pub memory: u32,
            pub m: BufferM,
            pub length: u32,
            pub reserved2: u32,
            pub reserved: u32,
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct Timecode {
            pub type_: u32,
            pub flags: u32,
            pub frames: u8,
            pub seconds: u8,
            pub minutes: u8,
            pub hours: u8,
            pub userbits: [u8; 4],
        }

        // ---- ioctl numbers ----
        pub const VIDIOC_QUERYCAP: u32 = ioc(IOC_READ, b'V', 0, size_of::<Capability>());
        pub const VIDIOC_ENUM_FMT: u32 = ioc(IOC_RW, b'V', 2, size_of::<FmtDesc>());
        pub const VIDIOC_G_FMT: u32 = ioc(IOC_RW, b'V', 4, size_of::<Format>());
        pub const VIDIOC_S_FMT: u32 = ioc(IOC_RW, b'V', 5, size_of::<Format>());
        pub const VIDIOC_REQBUFS: u32 = ioc(IOC_RW, b'V', 8, size_of::<RequestBuffers>());
        pub const VIDIOC_QUERYBUF: u32 = ioc(IOC_RW, b'V', 9, size_of::<Buffer>());
        pub const VIDIOC_QBUF: u32 = ioc(IOC_RW, b'V', 15, size_of::<Buffer>());
        pub const VIDIOC_DQBUF: u32 = ioc(IOC_RW, b'V', 17, size_of::<Buffer>());
        pub const VIDIOC_STREAMON: u32 = ioc(IOC_WRITE, b'V', 18, size_of::<libc::c_int>());
        pub const VIDIOC_STREAMOFF: u32 = ioc(IOC_WRITE, b'V', 19, size_of::<libc::c_int>());
        pub const VIDIOC_ENUM_FRAMESIZES: u32 = ioc(IOC_RW, b'V', 74, size_of::<FrmSizeEnum>());

        // ABI pins (gcc-verified against /usr/include/linux/videodev2.h,
        // turn 38): any compiler/arch drift fails the BUILD, not the bench.
        const _: () = assert!(size_of::<Capability>() == 104);
        const _: () = assert!(size_of::<PixFormat>() == 48);
        const _: () = assert!(size_of::<Format>() == 208);
        const _: () = assert!(offset_of!(Format, fmt) == 8);
        const _: () = assert!(size_of::<FmtDesc>() == 64);
        const _: () = assert!(size_of::<FrmSizeEnum>() == 44);
        const _: () = assert!(size_of::<RequestBuffers>() == 20);
        const _: () = assert!(size_of::<Buffer>() == 88);
        const _: () = assert!(offset_of!(Buffer, timestamp) == 24);
        const _: () = assert!(offset_of!(Buffer, sequence) == 56);
        const _: () = assert!(offset_of!(Buffer, m) == 64);
        const _: () = assert!(offset_of!(Buffer, length) == 72);
        const _: () = assert!(VIDIOC_QUERYCAP == 0x8068_5600);
        const _: () = assert!(VIDIOC_ENUM_FMT == 0xc040_5602);
        const _: () = assert!(VIDIOC_G_FMT == 0xc0d0_5604);
        const _: () = assert!(VIDIOC_S_FMT == 0xc0d0_5605);
        const _: () = assert!(VIDIOC_REQBUFS == 0xc014_5608);
        const _: () = assert!(VIDIOC_QUERYBUF == 0xc058_5609);
        const _: () = assert!(VIDIOC_QBUF == 0xc058_560f);
        const _: () = assert!(VIDIOC_DQBUF == 0xc058_5611);
        const _: () = assert!(VIDIOC_STREAMON == 0x4004_5612);
        const _: () = assert!(VIDIOC_STREAMOFF == 0x4004_5613);
        const _: () = assert!(VIDIOC_ENUM_FRAMESIZES == 0xc02c_564a);

        // ---- capability / type constants ----
        pub const CAP_VIDEO_CAPTURE: u32 = 0x0000_0001;
        pub const CAP_STREAMING: u32 = 0x0400_0000;
        pub const CAP_DEVICE_CAPS: u32 = 0x8000_0000;
        pub const BUF_TYPE_VIDEO_CAPTURE: u32 = 1;
        pub const MEMORY_MMAP: u32 = 1;
        pub const FRMSIZE_TYPE_DISCRETE: u32 = 1;
        // (FRMSIZE_TYPE_STEPWISE = 3: the non-discrete branch below)

        /// One ioctl call; errno on failure.
        pub unsafe fn ioctl<T>(fd: i32, req: u32, arg: *mut T) -> io::Result<i32> {
            let r = libc::ioctl(fd, req as libc::c_ulong, arg.cast::<libc::c_void>());
            if r < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(r)
            }
        }
    }

    pub struct Args {
        probe: bool,
        cmos: bool,
        iso: bool,
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
                    "--frames" | "--device" | "--outdir" => {
                        let val = a.get(i + 1).cloned();
                        match (a[i].as_str(), val) {
                            ("--frames", Some(v)) => {
                                args.frames = v.parse().unwrap_or(10).clamp(1, 300)
                            }
                            ("--device", Some(v)) => args.device = Some(v),
                            ("--outdir", Some(v)) => args.outdir = Some(v),
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

    /// One open /dev/videoN device with its mmap'd buffers.
    struct V4l2Dev {
        file: File,
        pub caps: uapi::Capability,
        maps: Vec<(usize, *mut u8)>, // (length, ptr)
        streaming: bool,
    }

    impl V4l2Dev {
        fn open(path: &Path) -> io::Result<V4l2Dev> {
            let file = OpenOptions::new().read(true).write(true).open(path)?;
            let mut caps = unsafe { std::mem::zeroed::<uapi::Capability>() };
            unsafe { uapi::ioctl(file.as_raw_fd(), uapi::VIDIOC_QUERYCAP, &mut caps)? };
            Ok(V4l2Dev { file, caps, maps: Vec::new(), streaming: false })
        }

        fn get_format(&self) -> io::Result<uapi::PixFormat> {
            let mut f = unsafe { std::mem::zeroed::<uapi::Format>() };
            f.type_ = uapi::BUF_TYPE_VIDEO_CAPTURE;
            unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_G_FMT, &mut f)? };
            Ok(unsafe { f.fmt.pix })
        }

        fn set_format(&self, want: uapi::PixFormat) -> io::Result<uapi::PixFormat> {
            let mut f = uapi::Format {
                type_: uapi::BUF_TYPE_VIDEO_CAPTURE,
                fmt: uapi::FormatFmt { pix: want },
            };
            unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_S_FMT, &mut f)? };
            Ok(unsafe { f.fmt.pix })
        }

        fn enum_formats(&self) -> Vec<(String, u32)> {
            let mut out = Vec::new();
            for idx in 0..64 {
                let mut d = unsafe { std::mem::zeroed::<uapi::FmtDesc>() };
                d.index = idx;
                d.type_ = uapi::BUF_TYPE_VIDEO_CAPTURE;
                match unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_ENUM_FMT, &mut d) } {
                    Ok(_) => out.push((cstr(&d.description), d.pixelformat)),
                    Err(_) => break,
                }
            }
            out
        }

        fn enum_framesizes(&self, fourcc: u32) -> Option<Vec<(u32, u32)>> {
            let mut out = Vec::new();
            for idx in 0..64 {
                let mut e = unsafe { std::mem::zeroed::<uapi::FrmSizeEnum>() };
                e.index = idx;
                e.pixel_format = fourcc;
                match unsafe {
                    uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_ENUM_FRAMESIZES, &mut e)
                } {
                    Ok(_) => {
                        if e.type_ == uapi::FRMSIZE_TYPE_DISCRETE {
                            let d = unsafe { e.size.discrete };
                            out.push((d.width, d.height));
                        } else {
                            // stepwise (=3) / continuous: report the max once
                            let s = unsafe { e.size.stepwise };
                            out.push((s.max_width, s.max_height));
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            Some(out)
        }

        fn request_buffers(&mut self, count: u32) -> io::Result<u32> {
            let mut rb = unsafe { std::mem::zeroed::<uapi::RequestBuffers>() };
            rb.count = count;
            rb.type_ = uapi::BUF_TYPE_VIDEO_CAPTURE;
            rb.memory = uapi::MEMORY_MMAP;
            unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_REQBUFS, &mut rb)? };
            Ok(rb.count)
        }

        fn map_buffer(&mut self, index: u32) -> io::Result<usize> {
            let mut b = unsafe { std::mem::zeroed::<uapi::Buffer>() };
            b.index = index;
            b.type_ = uapi::BUF_TYPE_VIDEO_CAPTURE;
            b.memory = uapi::MEMORY_MMAP;
            unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_QUERYBUF, &mut b)? };
            let len = b.length as usize;
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    len,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    self.file.as_raw_fd(),
                    b.m.offset as libc::off_t,
                )
            };
            if ptr == libc::MAP_FAILED {
                return Err(io::Error::last_os_error());
            }
            self.maps.push((len, ptr as *mut u8));
            Ok(len)
        }

        fn queue(&self, index: u32) -> io::Result<()> {
            let mut b = unsafe { std::mem::zeroed::<uapi::Buffer>() };
            b.index = index;
            b.type_ = uapi::BUF_TYPE_VIDEO_CAPTURE;
            b.memory = uapi::MEMORY_MMAP;
            unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_QBUF, &mut b)? };
            Ok(())
        }

        fn stream_on(&mut self) -> io::Result<()> {
            let mut t = uapi::BUF_TYPE_VIDEO_CAPTURE as libc::c_int;
            unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_STREAMON, &mut t)? };
            self.streaming = true;
            Ok(())
        }

        fn stream_off(&mut self) {
            if self.streaming {
                let mut t = uapi::BUF_TYPE_VIDEO_CAPTURE as libc::c_int;
                let _ =
                    unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_STREAMOFF, &mut t) };
                self.streaming = false;
            }
        }

        /// Poll for `timeout_ms`, then DQBUF. `Ok(Some((index, bytesused,
        /// sequence, timestamp_us)))`; `Ok(None)` = no frame in time.
        fn dq(&self, timeout_ms: i32) -> io::Result<Option<(u32, u32, u32, i64)>> {
            let mut pfd =
                libc::pollfd { fd: self.file.as_raw_fd(), events: libc::POLLIN, revents: 0 };
            let r = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
            if r < 0 {
                return Err(io::Error::last_os_error());
            }
            if r == 0 {
                return Ok(None);
            }
            let mut b = unsafe { std::mem::zeroed::<uapi::Buffer>() };
            b.type_ = uapi::BUF_TYPE_VIDEO_CAPTURE;
            b.memory = uapi::MEMORY_MMAP;
            unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_DQBUF, &mut b)? };
            let ts = b.timestamp.tv_sec as i64 * 1_000_000 + b.timestamp.tv_usec as i64;
            Ok(Some((b.index, b.bytesused, b.sequence, ts)))
        }

        /// The frame bytes of a dequeued buffer (clamped to the map len).
        fn buffer_bytes(&self, index: u32, bytesused: u32) -> &[u8] {
            let i = index as usize;
            if i >= self.maps.len() {
                return &[];
            }
            let (len, ptr) = self.maps[i];
            let n = (bytesused as usize).min(len);
            unsafe { std::slice::from_raw_parts(ptr, n) }
        }
    }

    impl Drop for V4l2Dev {
        fn drop(&mut self) {
            self.stream_off();
            for (len, ptr) in self.maps.drain(..) {
                unsafe { libc::munmap(ptr.cast(), len) };
            }
        }
    }

    // ---------------- the two modes ----------------

    pub fn run() {
        let args = Args::parse();
        if args.probe {
            probe(&args);
        } else if args.iso {
            iso_stream_test(&args);
        } else {
            stream_test(&args);
        }
    }

    /// `--probe`: pure diagnostic, no streaming, no USB writes.
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
            let c = &dev.caps;
            println!(
                "      driver {} card {:?} bus {:?}",
                cstr(&c.driver),
                cstr(&c.card),
                cstr(&c.bus_info)
            );
            let caps = if c.capabilities & uapi::CAP_DEVICE_CAPS != 0 {
                c.device_caps
            } else {
                c.capabilities
            };
            let cap_capture = caps & uapi::CAP_VIDEO_CAPTURE != 0;
            let cap_stream = caps & uapi::CAP_STREAMING != 0;
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

    /// The stream test (default mode).
    fn stream_test(args: &Args) {
        println!("=== idong P2.5 video bring-up ===");
        println!();

        // [1/4] session init on MI_04 (rusb), only with --cmos
        // The transport lives in a fixed Option slot; the Session borrows
        // that slot for the whole stream (drop order frees it correctly).
        let mut transport: Option<UsbTransport> = None;
        let mut session: Option<Session> = None;
        if args.cmos {
            println!("[1/4] session init on MI_04 (--cmos: EEPROM reads + CMOS registers)");
            let t = match UsbTransport::open_runtime() {
                Ok(t) => t,
                Err(e) => {
                    println!("      FAIL: cannot open MI_04: {e}");
                    println!("      - sensor plugged in? udev rule installed? not in bootloader?");
                    std::process::exit(1);
                }
            };
            transport = Some(t);
            let t = transport.as_mut().unwrap();
            match Session::probe(&mut *t) {
                Ok(idong::Phase::Runtime) => {}
                Ok(_) => {
                    println!("      FAIL: device not in runtime phase - run the vendor software");
                    println!("      once (or src/examples/flash_mcu.rs), then rerun");
                    std::process::exit(1);
                }
                Err(e) => {
                    println!("      FAIL: version probe: {e}");
                    std::process::exit(1);
                }
            }
            let s = match Session::connect(&mut *t, idong::Phase::Runtime, &[], false) {
                Ok(s) => s,
                Err(e) => {
                    println!("      FAIL: session init: {e}");
                    println!("      (the sensor self-reboots every 31-157 s - just rerun)");
                    std::process::exit(1);
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
                        println!("      FAIL: no /dev/video* node matched the sensor.");
                        println!("      nodes found:");
                        for n in &nodes {
                            println!("        {}  iface {:?}", n.dev.display(), n.usb_iface);
                        }
                        println!("      sensor usb ports: {ports:?}");
                        println!("      rerun with an explicit node: --device /dev/videoN");
                        std::process::exit(1);
                    }
                    _ => {
                        // Turn-39: a composite sensor exposes SEVERAL nodes
                        // on the same interface (the bench run showed
                        // video0 capture + video1 metadata, both "1-2:1.0")
                        // - pick the one that can actually capture. The
                        // query is the same read-only QUERYCAP --probe
                        // does; no streaming, no USB writes.
                        println!(
                            "      several nodes matched the sensor (iface {:?}):",
                            hits[0].usb_iface
                        );
                        let mut pick: Option<&idong::transport::v4l2::VideoNode> = None;
                        for n in &hits {
                            let caps = V4l2Dev::open(&n.dev).ok().map(|d| {
                                if d.caps.capabilities & uapi::CAP_DEVICE_CAPS != 0 {
                                    d.caps.device_caps
                                } else {
                                    d.caps.capabilities
                                }
                            });
                            let is_capture =
                                caps.map(|c| c & uapi::CAP_VIDEO_CAPTURE != 0).unwrap_or(false);
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
                println!("      FAIL: cannot open {}: {e}", node.display());
                if e.kind() == io::ErrorKind::PermissionDenied {
                    println!("      permissions: sudo usermod -aG video $USER, then re-login");
                }
                std::process::exit(1);
            }
        };
        let c = &dev.caps;
        println!(
            "      driver {} card {:?} bus {:?}",
            cstr(&c.driver),
            cstr(&c.card),
            cstr(&c.bus_info)
        );
        let caps = if c.capabilities & uapi::CAP_DEVICE_CAPS != 0 {
            c.device_caps
        } else {
            c.capabilities
        };
        if caps & uapi::CAP_VIDEO_CAPTURE == 0 {
            println!("      FAIL: node is not a video capture device");
            std::process::exit(1);
        }
        if caps & uapi::CAP_STREAMING == 0 {
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
        let outdir = match &args.outdir {
            Some(d) => PathBuf::from(d),
            None => next_run_dir(),
        };
        fs::create_dir_all(&outdir).expect("create output dir");
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
        let mut got = 0u32;
        let mut poll_errors = 0u32;
        let mut polls_live = true;
        let mut first_ts: Option<i64> = None;
        let mut last_ts: Option<i64> = None;
        let mut any_a = None;
        let mut any_b = None;
        let mut frame_log: Vec<(u32, u32, u32, i64)> = Vec::new();

        while got < args.frames && t_start.elapsed() < Duration::from_secs(30) {
            // keep the 0x0205 stream poll alive (~10 Hz) while streaming,
            // exactly the vendor pattern (30 Hz on the wire); stop after
            // three consecutive unanswered polls (it is not required for
            // V4L2 streaming - the vendor engine sends it, we observe).
            if polls_live
                && session.is_some()
                && t_last_poll.elapsed() >= Duration::from_millis(100)
            {
                match session.as_mut().unwrap().poll_stream() {
                    Ok(st) => {
                        poll_errors = 0;
                        if st.new_frame {
                            println!(
                                "      0x0205: new frame #{} (frame counter advanced)",
                                st.frame_no
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
                    let (a, b) = idong::protocol::video::split_channels(bytes);
                    let sa = plane_stats(&a);
                    let sb = plane_stats(&b);
                    println!(
                        "      frame {:3}: seq {:3} {} B  chA mean {:6.1} sd {:5.1} [{:3},{:3}]  chB mean {:6.1} sd {:5.1} [{:3},{:3}]",
                        got, seq, bytesused, sa.mean, sa.stddev, sa.min, sa.max, sb.mean, sb.stddev, sb.min, sb.max
                    );
                    if got < 3 {
                        let raw = outdir.join(format!("frame{got:04}.bin"));
                        let _ = fs::write(&raw, bytes);
                    }
                    write_plane_png(&outdir.join(format!("frame{got:04}_chA.png")), &a, &pix);
                    write_plane_png(&outdir.join(format!("frame{got:04}_chB.png")), &b, &pix);
                    any_a = merge(any_a, sa);
                    any_b = merge(any_b, sb);
                    first_ts.get_or_insert(ts);
                    last_ts = Some(ts);
                    frame_log.push((got, seq, bytesused, ts));
                    got += 1;
                    if let Err(e) = dev.queue(idx) {
                        println!("      FAIL: requeue: {e}");
                        break;
                    }
                }
                Ok(None) => continue,
                Err(e) => {
                    println!("      DQBUF error: {e}");
                    break;
                }
            }
        }
        dev.stream_off();

        println!();
        if got == 0 {
            println!("FAIL: no frames within 30 s.");
            println!("      checks: rerun WITH --cmos; is another app using the node;");
            println!("      dmesg | tail -50; formats: run --probe first and send both outputs.");
            println!("      If uvcvideo cannot drive this interface, the fallback is the");
            println!("      raw usbfs-ISO backend on EP 0x85 (P2.5b, no V4L2).");
            std::process::exit(1);
        }
        let span_us = last_ts.zip(first_ts).map(|(l, f)| l - f).unwrap_or(0).max(1);
        let fps = (got as f64 - 1.0) * 1_000_000.0 / span_us as f64;
        println!("frames captured : {}", got);
        println!("measured rate   : {fps:.1} fps (capture timestamps)");
        println!(
            "session         : {}",
            if transport.is_some() {
                "held (MI_04 + 0x0205 polls)"
            } else {
                "none (--cmos absent)"
            }
        );
        println!("0x0205 polls    : {} unanswered (off after 3 if so)", poll_errors);
        println!("bytes/frame     : {}", frame_log[0].2);
        println!("channel A stats : {:?}", any_a);
        println!("channel B stats : {:?}", any_b);
        if let Some(a) = any_a {
            if a.has_contrast() {
                println!("PASS: channel A carries a real image (contrast present).");
            } else {
                println!("WARN: channel A is flat - registers not initialized? (use --cmos)");
            }
        }
        let expect = pix.sizeimage;
        if frame_log[0].2 == expect {
            println!("PASS: frame size matches the negotiated format ({} B).", expect);
        } else {
            println!(
                "NOTE: frame size {} != negotiated {} - send the .bin files.",
                frame_log[0].2, expect
            );
        }
        println!();
        println!("output: {} (PNGs per channel + raw .bin of the first frames)", outdir.display());
        println!("next steps:");
        println!("  - send the PNGs + this console output back");
        println!("  - if channel B decodes as an image too, the marker/3D plane is next");
    }

    fn merge(prev: Option<PlaneStats>, s: PlaneStats) -> Option<PlaneStats> {
        Some(match prev {
            None => s,
            Some(p) => PlaneStats {
                mean: (p.mean + s.mean) / 2.0,
                min: p.min.min(s.min),
                max: p.max.max(s.max),
                stddev: (p.stddev + s.stddev) / 2.0,
            },
        })
    }

    // ---------------- the --iso (P2.5b) engine ----------------

    /// The usbfs UAPI mirror (mirrors the V4L2 one: ABI-pinned against
    /// /usr/include/linux/usbdevice_fs.h, gcc-verified turn 42).
    mod usbfs_api {
        use core::mem::{offset_of, size_of};
        use std::io;

        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct IsoPacketDesc {
            pub length: u32,
            pub actual_length: u32,
            pub status: u32,
        }

        #[repr(C)]
        pub struct CtrlTransfer {
            pub bm_request_type: u8,
            pub b_request: u8,
            pub w_value: u16,
            pub w_index: u16,
            pub w_length: u16,
            pub timeout: u32, // ms
            pub data: *mut u8,
        }

        #[repr(C)]
        pub struct SetInterface {
            pub interface: u32,
            pub altsetting: u32,
        }

        #[repr(C)]
        pub struct DisconnectClaim {
            pub interface: u32,
            pub flags: u32,
            pub driver: [u8; 256],
        }

        /// The URB header (the kernel's flexible-array member starts at
        /// offset 56 - the header size; the packet descs follow).
        #[repr(C)]
        pub struct UrbHeader {
            pub urb_type: u8,
            pub endpoint: u8,
            pub status: i32,
            pub flags: u32,
            pub buffer: *mut u8,
            pub buffer_length: i32,
            pub actual_length: i32,
            pub start_frame: i32,
            /// number_of_packets (ISO) | stream_id (bulk) - the union.
            pub number_of_packets: i32,
            pub error_count: i32,
            pub signr: u32,
            pub usercontext: *mut u8,
        }

        // The full request-number set is pinned for documentation; the
        // --iso path uses CONTROL / DISCONNECT_CLAIM / SUBMITURB /
        // REAPURBNDELAY (and RELEASEINTERFACE on drop - wired below).
        #[allow(dead_code)]
        pub const USBDEVFS_CONTROL: u32 = 0xc018_5500;
        #[allow(dead_code)]
        pub const USBDEVFS_SETINTERFACE: u32 = 0x8008_5504;
        #[allow(dead_code)]
        pub const USBDEVFS_CLAIMINTERFACE: u32 = 0x8004_550f;
        pub const USBDEVFS_RELEASEINTERFACE: u32 = 0x8004_5510;
        pub const USBDEVFS_SUBMITURB: u32 = 0x8038_550a;
        pub const USBDEVFS_REAPURBNDELAY: u32 = 0x4008_550d;
        #[allow(dead_code)]
        pub const USBDEVFS_DISCARDURB: u32 = 0x0000_550b;
        pub const USBDEVFS_DISCONNECT_CLAIM: u32 = 0x8108_551b;

        pub const URB_TYPE_ISO: u8 = 0;
        pub const URB_ISO_ASAP: u32 = 0x02;
        /// `USBDEVFS_DISCONNECT_CLAIM_IF_DRIVER` - disconnect only if the
        /// named driver is bound.
        pub const DISC_CLAIM_IF_DRIVER: u32 = 0x01;

        // ---- ABI pins (gcc-verified against the kernel header) ----
        const _: () = assert!(size_of::<IsoPacketDesc>() == 12);
        const _: () = assert!(size_of::<CtrlTransfer>() == 24);
        const _: () = assert!(size_of::<SetInterface>() == 8);
        const _: () = assert!(size_of::<DisconnectClaim>() == 264);
        const _: () = assert!(size_of::<UrbHeader>() == 56);
        const _: () = assert!(offset_of!(UrbHeader, buffer) == 16);
        const _: () = assert!(offset_of!(UrbHeader, buffer_length) == 24);
        const _: () = assert!(offset_of!(UrbHeader, actual_length) == 28);
        const _: () = assert!(offset_of!(UrbHeader, number_of_packets) == 36);
        const _: () = assert!(offset_of!(UrbHeader, signr) == 44);
        const _: () = assert!(USBDEVFS_CONTROL == 0xc018_5500);
        const _: () = assert!(USBDEVFS_SUBMITURB == 0x8038_550a);
        const _: () = assert!(USBDEVFS_REAPURBNDELAY == 0x4008_550d);
        const _: () = assert!(USBDEVFS_DISCARDURB == 0x550b);
        const _: () = assert!(USBDEVFS_DISCONNECT_CLAIM == 0x8108_551b);

        /// One ioctl call; errno on failure.
        pub unsafe fn ioctl(fd: i32, req: u32, arg: *mut u8) -> io::Result<i32> {
            let r = libc::ioctl(fd, req as libc::c_ulong, arg.cast::<libc::c_void>());
            if r < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(r)
            }
        }
    }

    /// One in-flight ISO URB: header + packet descs + data buffer, as one
    /// malloc'd blob; re-submitted until the ring drains.
    struct IsoUrb {
        ptr: *mut usbfs_api::UrbHeader,
        layout: std::alloc::Layout,
        buf: *mut u8,
        buf_layout: std::alloc::Layout,
        packet_size: usize,
    }

    // SAFETY: the URB blob is owned exclusively by this struct; the ioctl
    // kernel calls read/write it while submitted, but only this thread
    // touches it between submit and reap.
    unsafe impl Send for IsoUrb {}

    impl IsoUrb {
        /// Allocate one URB of `npackets` slots x `packet_size` B.
        fn new(ep: u8, npackets: usize, packet_size: usize) -> io::Result<IsoUrb> {
            use std::alloc::{alloc, Layout};
            let desc_off = size_of::<usbfs_api::UrbHeader>();
            let total = desc_off + npackets * size_of::<usbfs_api::IsoPacketDesc>();
            let layout = Layout::from_size_align(total, 8).map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidInput, e.to_string())
            })?;
            let ptr = unsafe { alloc(layout) } as *mut usbfs_api::UrbHeader;
            let buf_layout =
                Layout::from_size_align(npackets * packet_size, 8).map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidInput, e.to_string())
                })?;
            let buf = unsafe { alloc(buf_layout) };
            if ptr.is_null() || buf.is_null() {
                return Err(io::Error::new(io::ErrorKind::OutOfMemory, "URB alloc"));
            }
            // zero the descs and fill the static fields once
            unsafe {
                std::ptr::write_bytes(ptr as *mut u8, 0, total);
                let h = &mut *ptr;
                h.urb_type = usbfs_api::URB_TYPE_ISO;
                h.endpoint = ep;
                h.flags = usbfs_api::URB_ISO_ASAP;
                h.buffer = buf;
                h.buffer_length = (npackets * packet_size) as i32;
                h.number_of_packets = npackets as i32;
                let descs = std::slice::from_raw_parts_mut(
                    (ptr as *mut u8).add(desc_off) as *mut usbfs_api::IsoPacketDesc,
                    npackets,
                );
                for d in descs.iter_mut() {
                    d.length = packet_size as u32;
                }
            }
            Ok(IsoUrb { ptr, layout, buf, buf_layout, packet_size })
        }

        /// (Re-)submit this URB.
        unsafe fn submit(&mut self, fd: i32) -> io::Result<()> {
            usbfs_api::ioctl(fd, usbfs_api::USBDEVFS_SUBMITURB, self.ptr as *mut u8).map(|_| ())
        }

        /// The per-packet results of a reaped URB: (actual_length, status).
        unsafe fn packet_results(&self) -> Vec<(u32, i32)> {
            let h = &*self.ptr;
            let n = h.number_of_packets.max(0) as usize;
            let descs = std::slice::from_raw_parts(
                (self.ptr as *const u8).add(size_of::<usbfs_api::UrbHeader>())
                    as *const usbfs_api::IsoPacketDesc,
                n,
            );
            descs.iter().map(|d| (d.actual_length, d.status as i32)).collect()
        }

        /// One packet's received bytes: packet `i` lives at
        /// `i * packet_size` in the data buffer, length `actual`.
        unsafe fn packet_bytes(&self, i: usize, actual: u32) -> &[u8] {
            let off = i * self.packet_size + 0;
            let n = (actual as usize).min(self.packet_size);
            std::slice::from_raw_parts(self.buf.add(off), n)
        }
    }

    impl Drop for IsoUrb {
        fn drop(&mut self) {
            unsafe { std::alloc::dealloc(self.ptr as *mut u8, self.layout) };
            unsafe { std::alloc::dealloc(self.buf, self.buf_layout) };
        }
    }

    /// The --iso stream test (P2.5b raw usbfs ISO on EP 0x85).
    fn iso_stream_test(args: &Args) {
        use idong::protocol::video::{ProbeCommit, VideoAssembler};
        use idong::transport::usbfs as pure;

        println!("=== idong P2.5b raw usbfs-ISO bring-up ===");
        println!();

        // [1/4] session init on MI_04 (same as the V4L2 path, --cmos)
        // the transport slot is kept alive to the end: dropping it frees
        // MI_04 AFTER the stream stops (the session owns no resource)
        #[allow(unused_assignments)]
        let mut transport: Option<UsbTransport> = None;
        let mut session: Option<Session> = None;
        if args.cmos {
            println!("[1/4] session init on MI_04 (--cmos: EEPROM reads + CMOS registers)");
            let t = match UsbTransport::open_runtime() {
                Ok(t) => t,
                Err(e) => {
                    println!("      FAIL: cannot open MI_04: {e}");
                    std::process::exit(1);
                }
            };
            transport = Some(t);
            let t = transport.as_mut().unwrap();
            match Session::probe(&mut *t) {
                Ok(idong::Phase::Runtime) => {}
                Ok(_) => {
                    println!("      FAIL: device not in runtime phase");
                    std::process::exit(1);
                }
                Err(e) => {
                    println!("      FAIL: version probe: {e}");
                    std::process::exit(1);
                }
            }
            let s = match Session::connect(&mut *t, idong::Phase::Runtime, &[], false) {
                Ok(s) => s,
                Err(e) => {
                    println!("      FAIL: session init: {e}");
                    println!("      (the sensor self-reboots every 31-157 s - just rerun)");
                    std::process::exit(1);
                }
            };
            println!("      device ID {}", s.info().device_id);
            println!("      PASS: session init complete");
            session = Some(s);
        } else {
            println!("[1/4] session init SKIPPED (no --cmos)");
            println!("      the vendor flow always inits the sensor registers before");
            println!("      video; without it the stream may stay empty (still a valid");
            println!("      experiment - a FAIL here does not reject the backend).");
        }

        // [2/4] the raw device node + descriptors
        println!("[2/4] raw device node (/dev/bus/usb)");
        // rusb discovery finds bus/addr; the session transport holds MI_04
        // on the SAME node - the usbfs fd must be a SECOND open of it.
        let devices = idong::transport::usb::list_devices();
        let runtime = devices.iter().find(|d| d.state == idong::DeviceState::Runtime);
        let Some(dev) = runtime else {
            println!("      FAIL: no 0471:2500 runtime device on the bus");
            std::process::exit(1);
        };
        let node_path = format!("/dev/bus/usb/{:03}/{:03}", dev.bus, dev.address);
        println!("      opening {node_path} (runtime, bus {} addr {})", dev.bus, dev.address);
        let node = match fs::OpenOptions::new().read(true).write(true).open(&node_path) {
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
        let cfg_blob = match read_config_descriptor(node.as_raw_fd()) {
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
        let best = pure::best_streaming_alt(&vs).expect("an ISO alt exists");
        let packet_size = best.iso_in().map(|e| e.wmax_total).unwrap_or(0) as usize;
        println!("      streaming alt {} -> {} B per microframe", best.alt, packet_size);

        // [3/4] claim the VS interface + UVC negotiation
        println!("[3/4] DISCONNECT_CLAIM MI_{:02} + UVC probe/commit", iface);
        let fd = node.as_raw_fd();
        let mut dc = usbfs_api::DisconnectClaim {
            interface: iface as u32,
            flags: usbfs_api::DISC_CLAIM_IF_DRIVER,
            driver: [0u8; 256],
        };
        dc.driver[.."uvcvideo".len()].copy_from_slice(b"uvcvideo");
        if let Err(e) = unsafe {
            usbfs_api::ioctl(
                fd,
                usbfs_api::USBDEVFS_DISCONNECT_CLAIM,
                &mut dc as *mut usbfs_api::DisconnectClaim as *mut u8,
            )
        } {
            println!("      FAIL: DISCONNECT_CLAIM: {e}");
            println!("      (is uvcvideo bound? try: ls /sys/class/video4linux)");
            std::process::exit(1);
        }
        println!("      PASS: MI_{:02} claimed away from uvcvideo (reversible on exit)", iface);
        let golden = ProbeCommit::GOLDEN;
        let probe_body = pure::probe_commit_body(&golden);
        for (cs, label) in [(pure::UVC_CS_PROBE, "probe"), (pure::UVC_CS_COMMIT, "commit")] {
            let req = pure::uvc_set_cur(iface, cs, &probe_body);
            let mut ctrl = usbfs_api::CtrlTransfer {
                bm_request_type: req.request_type,
                b_request: req.request,
                w_value: req.value,
                w_index: req.index,
                w_length: req.length,
                timeout: 5000,
                data: req.data.as_ptr() as *mut u8,
            };
            let r = unsafe {
                usbfs_api::ioctl(
                    fd,
                    usbfs_api::USBDEVFS_CONTROL,
                    &mut ctrl as *mut usbfs_api::CtrlTransfer as *mut u8,
                )
            };
            match r {
                Ok(_) => println!("      PASS: {label} SET_CUR sent ({} B)", req.length),
                Err(e) => {
                    println!("      FAIL: {label} SET_CUR: {e}");
                    println!("      (the firmware may reject the negotiation without a");
                    println!("      session - rerun with --cmos)");
                    std::process::exit(1);
                }
            }
        }
        // restore alt 0 then raise to the streaming alt (alt 1) - the
        // observed vendor state (they never left alt 0 on the wire, but
        // the ISO endpoint only exists in alt >= 1 - the kernel needs to
        // route ISO packets to the endpoint... no: usbfs ISO URBs are
        // endpoint-addressed, not interface-routed. The capture's truth:
        // video flowed with NO alt change. We mirror the capture: no
        // SET_INTERFACE at all.)
        println!("      (no SET_INTERFACE - mirroring the vendor capture: video");
        println!("      flowed on EP 0x85 with the interface at alt 0)");

        // [4/4] the ISO URB ring
        println!("[4/4] ISO URB ring on EP 0x85 ({} frames)", args.frames);
        let npackets = 8usize; // 8 microframes per URB = 1 ms per re-submission
        let ring = 4usize; // four URBs in flight
        let mut urbs: Vec<IsoUrb> = Vec::new();
        for _ in 0..ring {
            match IsoUrb::new(0x85, npackets, packet_size) {
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
                println!("      collect dmesg | tail -50 - this decides P2.5b");
                std::process::exit(1);
            }
        }
        println!("      PASS: {ring} URBs x {npackets} microframes submitted (ASAP)");

        let outdir = match &args.outdir {
            Some(d) => PathBuf::from(d),
            None => next_run_dir(),
        };
        fs::create_dir_all(&outdir).expect("create output dir");
        println!("      output dir: {}", outdir.display());

        let mut assembler = VideoAssembler::new();
        let mut got = 0u32;
        let mut payloads = 0u64;
        let mut errs = 0u64;
        let mut any_a = None;
        let mut any_b = None;
        let t_start = Instant::now();
        let mut t_last_poll = Instant::now() - Duration::from_secs(1);
        let mut poll_errors = 0u32;
        let mut polls_live = true;
        'outer: while got < args.frames && t_start.elapsed() < Duration::from_secs(30) {
            // keep the 0x0205 stream poll alive (~10 Hz) as in the V4L2 run
            if polls_live
                && session.is_some()
                && t_last_poll.elapsed() >= Duration::from_millis(100)
            {
                match session.as_mut().unwrap().poll_stream() {
                    Ok(st) => {
                        poll_errors = 0;
                        let _ = st;
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
            // reap one URB (non-blocking) + poll on the fd
            let mut pfd = libc::pollfd { fd, events: libc::POLLOUT, revents: 0 };
            // usbfs signals a reaped URB as POLLOUT (writable)
            let r = unsafe { libc::poll(&mut pfd, 1, 100) };
            if r < 0 {
                println!("      poll: {}", io::Error::last_os_error());
                break;
            }
            if r == 0 {
                continue; // nothing reaped yet
            }
            // REAPURBNDELAY
            let mut reaped: *mut usbfs_api::UrbHeader = std::ptr::null_mut();
            let rr = unsafe {
                usbfs_api::ioctl(
                    fd,
                    usbfs_api::USBDEVFS_REAPURBNDELAY,
                    &mut reaped as *mut *mut usbfs_api::UrbHeader as *mut u8,
                )
            };
            match rr {
                Ok(_) => {}
                Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => continue,
                Err(e) => {
                    println!("      REAPURB: {e}");
                    break;
                }
            }
            // find which of our URBs was reaped (the returned pointer)
            let idx = urbs
                .iter()
                .position(|u| std::ptr::eq(u.ptr as *mut u8, reaped as *mut u8))
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
                    let (a, b) = idong::protocol::video::split_channels(&frame.data);
                    let sa = plane_stats(&a);
                    let sb = plane_stats(&b);
                    println!(
                        "      frame {:3}: pts {} {} B  chA mean {:6.1} sd {:5.1}  chB mean {:6.1} sd {:5.1}",
                        got, frame.pts, frame.data.len(), sa.mean, sa.stddev, sb.mean, sb.stddev
                    );
                    if got < 3 {
                        let _ = fs::write(outdir.join(format!("frame{got:04}.bin")), &frame.data);
                    }
                    write_plane_png(&outdir.join(format!("frame{got:04}_chA.png")), &a, &pix_for(&frame));
                    write_plane_png(&outdir.join(format!("frame{got:04}_chB.png")), &b, &pix_for(&frame));
                    any_a = merge(any_a, sa);
                    any_b = merge(any_b, sb);
                    got += 1;
                    if got >= args.frames {
                        break 'outer;
                    }
                }
            }
            // re-submit the reaped URB
            if let Err(e) = unsafe { urbs[idx].submit(fd) } {
                println!("      re-submit failed: {e}");
                break;
            }
        }

        // release the VS claim so uvcvideo can rebind (the kernel
        // re-probes on the next open of /dev/video*)
        let mut ifn = iface as u32;
        let _ = unsafe {
            usbfs_api::ioctl(
                fd,
                usbfs_api::USBDEVFS_RELEASEINTERFACE,
                &mut ifn as *mut u32 as *mut u8,
            )
        };

        println!();
        println!("payloads        : {payloads} ({} errors)", errs);
        println!("frames          : {got}");
        if got == 0 {
            println!("FAIL: no frames within 30 s via the raw ISO path.");
            println!("      collect: dmesg | tail -50, lsusb -v -d 0471:2500,");
            println!("      rerun with --cmos if it was absent above.");
            std::process::exit(1);
        }
        if let Some(a) = any_a {
            if a.has_contrast() {
                println!("PASS: channel A carries a real image (contrast present).");
            } else {
                println!("WARN: channel A flat - registers not initialized? (--cmos)");
            }
        }
        println!("channel A stats : {:?}", any_a);
        println!("channel B stats : {:?}", any_b);
        println!("output: {}", outdir.display());
    }

    /// Read the device's raw configuration descriptor over usbfs:
    /// GET_DESCRIPTOR(config) on ep 0 - no interface claim needed.
    /// Two steps (the classic USB dance): read 8 B for wTotalLength,
    /// then the whole descriptor.
    fn read_config_descriptor(fd: i32) -> io::Result<Vec<u8>> {
        let mut total_len = 0usize;
        for max in [8usize, 512usize] {
            let mut buf = vec![0u8; max];
            let mut ctrl = usbfs_api::CtrlTransfer {
                bm_request_type: 0x80,
                b_request: 0x06, // GET_DESCRIPTOR
                w_value: 0x0200, // config descriptor
                w_index: 0,
                w_length: max as u16,
                timeout: 5000,
                data: buf.as_mut_ptr(),
            };
            unsafe {
                usbfs_api::ioctl(
                    fd,
                    usbfs_api::USBDEVFS_CONTROL,
                    &mut ctrl as *mut usbfs_api::CtrlTransfer as *mut u8,
                )?
            };
            if max == 8 {
                if buf[0] != 9 || buf[1] != 2 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "not a config descriptor",
                    ));
                }
                total_len = u16::from_le_bytes([buf[2], buf[3]]) as usize;
                if total_len < 9 || total_len > 4096 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("implausible wTotalLength {total_len}"),
                    ));
                }
                buf.truncate(8);
            } else {
                buf.truncate(total_len.min(max));
                return Ok(buf);
            }
        }
        unreachable!("loop runs the second pass")
    }

    /// A minimal PixFormat for the PNG writer (640x480, stride 1280).
    fn pix_for(_frame: &idong::protocol::video::VideoFrame) -> uapi::PixFormat {
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
}
