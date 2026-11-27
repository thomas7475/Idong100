//! The V4L2 ioctl streaming engine - promoted into the crate on the
//! turn-44 bench PASS (P2.5 acceptance: the T7 stream run captured 10
//! frames through uvcvideo after the T8 flash, and the T9 raw-ISO run
//! passed the same day; see `docs/LINUX_TESTS.md`).
//!
//! Until turn 44 this UAPI mirror + the mmap/DQBUF engine lived in
//! `src/examples/video.rs`; the plan was to promote it behind a feature
//! after the first hardware PASS. It now sits here behind
//! `usb` + `target_os = "linux"`; the pure sysfs matcher stays in
//! [`super::v4l2`].
//!
//! # What this module is
//!
//! * `uapi` - a layout-verified mirror of the Linux V4L2 UAPI subset
//!   the driver needs (QUERYCAP/ENUM_FMT/G_FMT/S_FMT/
//!   ENUM_FRAMESIZES/REQBUFS/QUERYBUF/QBUF/DQBUF/STREAMON/STREAMOFF).
//!   Every struct size, field offset and ioctl number is
//!   const-asserted against the kernel headers (gcc-verified turn 38;
//!   any ABI drift fails the BUILD, not the bench).
//! * [`V4l2Dev`] - one open `/dev/videoN` node with its mmap'd
//!   buffers: the mmap-streaming state machine the video example and
//!   (later) the ROS 2 node drive.
//!
//! The crate is `#![deny(unsafe_code)]`; the ioctl/mmap layer is the
//! sanctioned exception, scoped to this module (raw kernel ABIs cannot
//! be called from safe code). `libc` is an optional dependency gated
//! behind the same `usb` feature - plain `cargo test` never builds
//! this module.

#![allow(unsafe_code)] // the raw-ioctl engine (see module docs)

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::Path;

// ---- V4L2 UAPI mirror (layout-verified, see module docs) ----
pub mod uapi {
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

/// One open /dev/videoN device with its mmap'd buffers (the P2.5
/// mmap-streaming engine, promoted turn 44).
pub struct V4l2Dev {
    file: File,
    caps: uapi::Capability,
    maps: Vec<(usize, *mut u8)>, // (length, ptr)
    streaming: bool,
}

impl V4l2Dev {
    /// Open the node and QUERYCAP it.
    pub fn open(path: &Path) -> io::Result<V4l2Dev> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let mut caps = unsafe { std::mem::zeroed::<uapi::Capability>() };
        unsafe { uapi::ioctl(file.as_raw_fd(), uapi::VIDIOC_QUERYCAP, &mut caps)? };
        Ok(V4l2Dev { file, caps, maps: Vec::new(), streaming: false })
    }

    /// The QUERYCAP result (driver/card/bus raw bytes).
    pub fn caps(&self) -> &uapi::Capability {
        &self.caps
    }

    /// The effective capabilities: `device_caps` when the kernel
    /// reports per-device caps, else `capabilities` (the V4L2 rule).
    pub fn device_caps(&self) -> u32 {
        if self.caps.capabilities & uapi::CAP_DEVICE_CAPS != 0 {
            self.caps.device_caps
        } else {
            self.caps.capabilities
        }
    }

    /// A video-capture node?
    pub fn is_capture(&self) -> bool {
        self.device_caps() & uapi::CAP_VIDEO_CAPTURE != 0
    }

    pub fn get_format(&self) -> io::Result<uapi::PixFormat> {
        let mut f = unsafe { std::mem::zeroed::<uapi::Format>() };
        f.type_ = uapi::BUF_TYPE_VIDEO_CAPTURE;
        unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_G_FMT, &mut f)? };
        Ok(unsafe { f.fmt.pix })
    }

    pub fn set_format(&self, want: uapi::PixFormat) -> io::Result<uapi::PixFormat> {
        let mut f = uapi::Format {
            type_: uapi::BUF_TYPE_VIDEO_CAPTURE,
            fmt: uapi::FormatFmt { pix: want },
        };
        unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_S_FMT, &mut f)? };
        Ok(unsafe { f.fmt.pix })
    }

    pub fn enum_formats(&self) -> Vec<(String, u32)> {
        let mut out = Vec::new();
        for idx in 0..64 {
            let mut d = unsafe { std::mem::zeroed::<uapi::FmtDesc>() };
            d.index = idx;
            d.type_ = uapi::BUF_TYPE_VIDEO_CAPTURE;
            match unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_ENUM_FMT, &mut d) } {
                Ok(_) => {
                    let end = d.description.iter().position(|b| *b == 0).unwrap_or(32);
                    out.push((
                        String::from_utf8_lossy(&d.description[..end]).to_string(),
                        d.pixelformat,
                    ));
                }
                Err(_) => break,
            }
        }
        out
    }

    pub fn enum_framesizes(&self, fourcc: u32) -> Option<Vec<(u32, u32)>> {
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

    pub fn request_buffers(&mut self, count: u32) -> io::Result<u32> {
        let mut rb = unsafe { std::mem::zeroed::<uapi::RequestBuffers>() };
        rb.count = count;
        rb.type_ = uapi::BUF_TYPE_VIDEO_CAPTURE;
        rb.memory = uapi::MEMORY_MMAP;
        unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_REQBUFS, &mut rb)? };
        Ok(rb.count)
    }

    pub fn map_buffer(&mut self, index: u32) -> io::Result<usize> {
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

    pub fn queue(&self, index: u32) -> io::Result<()> {
        let mut b = unsafe { std::mem::zeroed::<uapi::Buffer>() };
        b.index = index;
        b.type_ = uapi::BUF_TYPE_VIDEO_CAPTURE;
        b.memory = uapi::MEMORY_MMAP;
        unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_QBUF, &mut b)? };
        Ok(())
    }

    pub fn stream_on(&mut self) -> io::Result<()> {
        let mut t = uapi::BUF_TYPE_VIDEO_CAPTURE as libc::c_int;
        unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_STREAMON, &mut t)? };
        self.streaming = true;
        Ok(())
    }

    pub fn stream_off(&mut self) {
        if self.streaming {
            let mut t = uapi::BUF_TYPE_VIDEO_CAPTURE as libc::c_int;
            let _ = unsafe { uapi::ioctl(self.file.as_raw_fd(), uapi::VIDIOC_STREAMOFF, &mut t) };
            self.streaming = false;
        }
    }

    /// Poll for `timeout_ms`, then DQBUF. `Ok(Some((index, bytesused,
    /// sequence, timestamp_us)))`; `Ok(None)` = no frame in time.
    pub fn dq(&self, timeout_ms: i32) -> io::Result<Option<(u32, u32, u32, i64)>> {
        let mut pfd = libc::pollfd { fd: self.file.as_raw_fd(), events: libc::POLLIN, revents: 0 };
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
    pub fn buffer_bytes(&self, index: u32, bytesused: u32) -> &[u8] {
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
