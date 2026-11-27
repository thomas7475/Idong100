//! The usbfs ISO engine - the ioctl half of the P2.5b raw video
//! backend, promoted into the crate on the turn-44 bench PASS (T9:
//! the SET_INTERFACE-before-the-ring flow captured 10 frames, 1281
//! payloads, 0 errors - see `docs/LINUX_TESTS.md`).
//!
//! Until turn 44 this lived in `src/examples/video.rs` (`--iso`); the
//! pure helpers (config walk, wMaxPacketSize decode, UVC control
//! builders) have been in [`super::usbfs`] since turn 42. The split is
//! now: [`super::usbfs`] = pure, this module = the raw `/dev/bus/usb`
//! ioctls, behind `usb` + `target_os = "linux"`.
//!
//! # What this module is
//!
//! * `usbfs_api` - the layout-verified `usbdevice_fs` UAPI mirror
//!   (ISO packet descs, ctrl transfers, SET_INTERFACE,
//!   DISCONNECT_CLAIM, the URB header); every size, offset and ioctl
//!   number const-asserted against the kernel header (gcc-verified
//!   turn 42).
//! * [`IsoUrb`] - one in-flight ISO URB (header + packet descs + data
//!   buffer as one allocation), re-submitted until the ring drains.
//! * The free functions - one control transfer OUT, the two-step
//!   GET_DESCRIPTOR(config) read, DISCONNECT_CLAIM / SET_INTERFACE /
//!   RELEASE_INTERFACE, and the reap/discard ring steps.
//!
//! The crate is `#![deny(unsafe_code)]`; the ioctl layer is the
//! sanctioned exception, scoped to this module (raw kernel ABIs
//! cannot be called from safe code).

#![allow(unsafe_code)] // the raw-ioctl engine (see module docs)

use std::io;

use super::usbfs::UsbControl;

/// The usbfs UAPI mirror (mirrors the V4L2 one: ABI-pinned against
/// /usr/include/linux/usbdevice_fs.h, gcc-verified turn 42).
pub mod usbfs_api {
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
    // streaming path uses CONTROL / DISCONNECT_CLAIM / SETINTERFACE /
    // SUBMITURB / REAPURBNDELAY (and DISCARDURB + SETINTERFACE alt 0
    // + RELEASEINTERFACE on teardown - wired below).
    #[allow(dead_code)]
    pub const USBDEVFS_CONTROL: u32 = 0xc018_5500;
    pub const USBDEVFS_SETINTERFACE: u32 = 0x8008_5504;
    #[allow(dead_code)]
    pub const USBDEVFS_CLAIMINTERFACE: u32 = 0x8004_550f;
    pub const USBDEVFS_RELEASEINTERFACE: u32 = 0x8004_5510;
    pub const USBDEVFS_SUBMITURB: u32 = 0x8038_550a;
    pub const USBDEVFS_REAPURBNDELAY: u32 = 0x4008_550d;
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
pub struct IsoUrb {
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
    pub fn new(ep: u8, npackets: usize, packet_size: usize) -> io::Result<IsoUrb> {
        use std::alloc::{alloc, Layout};
        let desc_off = size_of::<usbfs_api::UrbHeader>();
        let total = desc_off + npackets * size_of::<usbfs_api::IsoPacketDesc>();
        let layout = Layout::from_size_align(total, 8)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        let ptr = unsafe { alloc(layout) } as *mut usbfs_api::UrbHeader;
        let buf_layout = Layout::from_size_align(npackets * packet_size, 8)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
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

    /// The URB header pointer (the token REAPURBNDELAY returns;
    /// callers match it against the ring).
    pub fn ptr(&self) -> *mut usbfs_api::UrbHeader {
        self.ptr
    }

    /// (Re-)submit this URB.
    pub unsafe fn submit(&mut self, fd: i32) -> io::Result<()> {
        usbfs_api::ioctl(fd, usbfs_api::USBDEVFS_SUBMITURB, self.ptr as *mut u8).map(|_| ())
    }

    /// The per-packet results of a reaped URB: (actual_length, status).
    pub unsafe fn packet_results(&self) -> Vec<(u32, i32)> {
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
    pub unsafe fn packet_bytes(&self, i: usize, actual: u32) -> &[u8] {
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

/// One host-to-device control transfer on the usbfs fd (SET_CUR /
/// vendor OUT requests). `req` comes from the pure builders in
/// [`super::usbfs`] (`uvc_set_cur`, ...).
pub fn control_out(fd: i32, req: &UsbControl, timeout_ms: u32) -> io::Result<()> {
    let mut ctrl = usbfs_api::CtrlTransfer {
        bm_request_type: req.request_type,
        b_request: req.request,
        w_value: req.value,
        w_index: req.index,
        w_length: req.length,
        timeout: timeout_ms,
        data: req.data.as_ptr() as *mut u8,
    };
    unsafe {
        usbfs_api::ioctl(
            fd,
            usbfs_api::USBDEVFS_CONTROL,
            &mut ctrl as *mut usbfs_api::CtrlTransfer as *mut u8,
        )
    }
    .map(|_| ())
}

/// Read the device's raw configuration descriptor over usbfs:
/// GET_DESCRIPTOR(config) on ep 0 - no interface claim needed.
/// Two steps (the classic USB dance): read 8 B for wTotalLength,
/// then the whole descriptor.
pub fn read_config_descriptor(fd: i32) -> io::Result<Vec<u8>> {
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
                return Err(io::Error::new(io::ErrorKind::InvalidData, "not a config descriptor"));
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

/// Take an interface away from a kernel driver and claim it for usbfs
/// (`USBDEVFS_DISCONNECT_CLAIM` with `IF_DRIVER`: disconnects ONLY if
/// `driver` is bound - reversible, the driver rebinds on release).
pub fn disconnect_claim(fd: i32, iface: u8, driver: &str) -> io::Result<()> {
    let mut dc = usbfs_api::DisconnectClaim {
        interface: iface as u32,
        flags: usbfs_api::DISC_CLAIM_IF_DRIVER,
        driver: [0u8; 256],
    };
    let n = driver.len().min(dc.driver.len());
    dc.driver[..n].copy_from_slice(&driver.as_bytes()[..n]);
    unsafe {
        usbfs_api::ioctl(
            fd,
            usbfs_api::USBDEVFS_DISCONNECT_CLAIM,
            &mut dc as *mut usbfs_api::DisconnectClaim as *mut u8,
        )
    }
    .map(|_| ())
}

/// `USBDEVFS_SETINTERFACE`: select an alt setting. For an ISO endpoint
/// this ARMS the host-controller transfer ring (an alt-0 submit fails
/// with ENOENT - the turn-43 lesson, see [`super::usbfs`]).
pub fn set_interface(fd: i32, iface: u8, alt: u8) -> io::Result<()> {
    let mut si = usbfs_api::SetInterface { interface: iface as u32, altsetting: alt as u32 };
    unsafe {
        usbfs_api::ioctl(
            fd,
            usbfs_api::USBDEVFS_SETINTERFACE,
            &mut si as *mut usbfs_api::SetInterface as *mut u8,
        )
    }
    .map(|_| ())
}

/// `USBDEVFS_RELEASEINTERFACE`: give the interface back (uvcvideo
/// rebinds on the next open of /dev/video*).
pub fn release_interface(fd: i32, iface: u8) -> io::Result<()> {
    let mut ifn = iface as u32;
    unsafe {
        usbfs_api::ioctl(fd, usbfs_api::USBDEVFS_RELEASEINTERFACE, &mut ifn as *mut u32 as *mut u8)
    }
    .map(|_| ())
}

/// `USBDEVFS_REAPURBNDELAY`: take one completed URB. `EAGAIN` = no URB
/// ready (the caller's signal to keep polling); the returned pointer
/// is the `IsoUrb::ptr()` of the completed URB.
pub fn reap_urb(fd: i32) -> io::Result<*mut usbfs_api::UrbHeader> {
    let mut reaped: *mut usbfs_api::UrbHeader = std::ptr::null_mut();
    unsafe {
        usbfs_api::ioctl(
            fd,
            usbfs_api::USBDEVFS_REAPURBNDELAY,
            &mut reaped as *mut *mut usbfs_api::UrbHeader as *mut u8,
        )
    }
    .map(|_| reaped)
}

/// Poll the fd for a reaped URB (`POLLOUT` is how usbfs signals it),
/// then `REAPURBNDELAY`. `Ok(None)` = nothing reaped within
/// `timeout_ms`; `Ok(Some(ptr))` = the completed URB's header pointer.
pub fn wait_reap(fd: i32, timeout_ms: i32) -> io::Result<Option<*mut usbfs_api::UrbHeader>> {
    let mut pfd = libc::pollfd { fd, events: libc::POLLOUT, revents: 0 };
    // usbfs signals a reaped URB as POLLOUT (writable)
    let r = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    if r < 0 {
        return Err(io::Error::last_os_error());
    }
    if r == 0 {
        return Ok(None); // nothing reaped yet
    }
    match reap_urb(fd) {
        Ok(p) => Ok(Some(p)),
        // EAGAIN between the poll and the reap: nothing after all
        Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => Ok(None),
        Err(e) => Err(e),
    }
}

/// The ring teardown: DISCARDURB every still-submitted URB, then reap
/// the discards out (unlink is asynchronous) until the ring drains or
/// `drain_ms` elapses. Call BEFORE SET_INTERFACE alt 0 / RELEASE.
pub fn discard_and_drain(fd: i32, urbs: &mut [IsoUrb], drain_ms: u64) {
    for u in urbs.iter_mut() {
        let _ = unsafe { usbfs_api::ioctl(fd, usbfs_api::USBDEVFS_DISCARDURB, u.ptr() as *mut u8) };
    }
    let drain_end = std::time::Instant::now() + std::time::Duration::from_millis(drain_ms.max(1));
    while std::time::Instant::now() < drain_end {
        match reap_urb(fd) {
            Ok(_) => continue, // reaped one more discarded URB
            _ => break,        // EAGAIN: the ring is drained
        }
    }
}
