//! usbfs backend - the pure half of the P2.5b ISO streaming path (turn 42).
//!
//! uvcvideo cannot STREAMON this firmware (T7 bench FAIL: `EIO` at
//! VIDIOC_STREAMON, turn 41). The capture (turn 35) shows the vendor app
//! never issued a SET_INTERFACE alt>0 either - video flowed on EP 0x85
//! with the interface untouched. The fallback the docs promised is the
//! raw usbfs-ISO backend: open `/dev/bus/usb/<bus>/<addr>`, take the UVC
//! VideoStreaming interface away from uvcvideo
//! (USBDEVFS_DISCONNECT_CLAIM), run the observed probe/commit control
//! requests, submit ISO URBs on EP 0x85, and parse each packet with the
//! existing [`crate::protocol::video::VideoAssembler`].
//!
//! This module is the PURE, hardware-free part: the configuration
//! descriptor walker, the wMaxPacketSize decode and the UVC control
//! builders. The ioctl layer (the UAPI mirror with ABI pins) lives in
//! `src/examples/video.rs` behind the `--iso` mode, exactly like the
//! V4L2 ioctl mirror before its first bench PASS promotes it into the
//! crate - `libc` is a dev-dependency, so the lib itself cannot call
//! ioctls.
//!
//! # The wMaxPacketSize decode (a doc correction, turn 42)
//!
//! The sensor's config descriptor advertises for EP 0x85: alt 1
//! `wMaxPacketSize` 0x032C and alt 2 `0x1324`. The earlier docs called
//! alt 2 "4900 B" - that is the DECIMAL misread of the raw 16-bit value
//! 0x1324. The USB 2.0 encoding is an 11-bit base (0x324 = 804) plus a
//! 2-bit count of additional transactions per microframe (2): the real
//! capacity is 804 x (2+1) = **2412 B per microframe** - exactly the
//! 2412-B packet stride the turn-35 capture shows (27 payloads per ISO
//! record, 1236-B tail). [`wmax_total`] performs that decode; alt 1's
//! 0x032C has no multiplier bits and decodes to plain 812 B.
//!
//! # The ISO URB shape
//!
//! One URB carries `npackets` microframes of `packet_size` bytes each
//! (alt 2: 2412). The kernel writes each received packet's data at the
//! buffer offset of the *requested* lengths (contiguous areas), and
//! reports `actual_length` + per-packet `status` in the URB's ISO
//! descriptor array. Each received packet = one UVC payload (12-B header
//! + image data) - exactly what [`VideoAssembler::push_payload`]
//! consumes.

use crate::protocol::video::ProbeCommit;

/// Decode a USB 2.0 high-speed ISO `wMaxPacketSize`: bits 10..0 = base
/// size, bits 12..11 = additional transactions per microframe.
///
/// `0x032C` (alt 1) -> 812; `0x1324` (alt 2) -> base 0x324 = 804 with 2
/// additional transactions -> 3 x 804 = **2412 B per microframe** - the
/// exact packet stride the turn-35 capture shows (the earlier docs'
/// "alt 2: 4900 B" was the raw 16-bit field read as decimal: 0x1324 =
/// 4900).
pub fn wmax_total(w: u16) -> u32 {
    let base = (w & 0x07FF) as u32;
    let additionals = ((w >> 11) & 0x3) as u32;
    base * (1 + additionals)
}

/// A parsed endpoint descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointDesc {
    /// bEndpointAddress (0x85 = the ISO IN).
    pub address: u8,
    /// bmAttributes (bits 0..1: 1 = isochronous).
    pub attributes: u8,
    /// wMaxPacketSize total, decoded by [`wmax_total`].
    pub wmax_total: u32,
}

impl EndpointDesc {
    /// Is this an isochronous IN endpoint?
    pub fn is_iso_in(&self) -> bool {
        self.attributes & 0x03 == 0x01 && self.address & 0x80 != 0
    }
}

/// A parsed interface alt-setting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceAlt {
    pub number: u8,
    pub alt: u8,
    /// bInterfaceClass (0x0E = video).
    pub class: u8,
    /// bInterfaceSubClass (0x02 = VideoStreaming).
    pub subclass: u8,
    pub endpoints: Vec<EndpointDesc>,
}

impl InterfaceAlt {
    /// The ISO IN endpoint of this alt (the video plane: EP 0x85).
    pub fn iso_in(&self) -> Option<&EndpointDesc> {
        self.endpoints.iter().find(|e| e.is_iso_in())
    }
}

/// Walk a raw configuration descriptor blob: every interface alt-setting
/// with its endpoint descriptors. Standard layout: `[len][type]...`;
/// interface = type 0x04 (9 B: number@2, alt@3, class@5, subclass@6),
/// endpoint = type 0x05 (7 B: address@2, attributes@3, wMaxPacketSize@4).
/// A malformed tail stops the walk, keeping what was parsed. Pure.
pub fn walk_config(blob: &[u8]) -> Vec<InterfaceAlt> {
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut cur: Option<InterfaceAlt> = None;
    while i + 2 <= blob.len() {
        let len = blob[i] as usize;
        let ty = blob[i + 1];
        if len < 2 || i + len > blob.len() {
            break;
        }
        match ty {
            0x04 if len >= 9 => {
                if let Some(prev) = cur.take() {
                    out.push(prev);
                }
                cur = Some(InterfaceAlt {
                    number: blob[i + 2],
                    alt: blob[i + 3],
                    class: blob[i + 5],
                    subclass: blob[i + 6],
                    endpoints: Vec::new(),
                });
            }
            0x05 if len >= 7 => {
                if let Some(c) = cur.as_mut() {
                    c.endpoints.push(EndpointDesc {
                        address: blob[i + 2],
                        attributes: blob[i + 3],
                        wmax_total: wmax_total(u16::from_le_bytes([blob[i + 4], blob[i + 5]])),
                    });
                }
            }
            _ => {}
        }
        i += len;
    }
    if let Some(last) = cur.take() {
        out.push(last);
    }
    out
}

/// The VideoStreaming interface (class 0x0E, subclass 0x02) among the
/// walked alts, with its alt-settings sorted by number. `None` when the
/// blob has no VS interface.
pub fn video_streaming_iface(alts: &[InterfaceAlt]) -> Option<Vec<&InterfaceAlt>> {
    let mut vs: Vec<&InterfaceAlt> =
        alts.iter().filter(|a| a.class == 0x0E && a.subclass == 0x02).collect();
    if vs.is_empty() {
        return None;
    }
    vs.sort_by_key(|a| a.alt);
    Some(vs)
}

/// The streaming alt-setting to use: the VS alt with the largest ISO IN
/// bandwidth (the vendor capture streams 2412-B packets = alt 2).
pub fn best_streaming_alt<'a>(vs: &[&'a InterfaceAlt]) -> Option<&'a InterfaceAlt> {
    vs.iter()
        .copied()
        .filter(|a| a.iso_in().is_some())
        .max_by_key(|a| a.iso_in().map(|e| e.wmax_total).unwrap_or(0))
}

/// One prepared control transfer (mirrors `usbdevfs_ctrltransfer`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbControl {
    pub request_type: u8,
    pub request: u8,
    pub value: u16,
    pub index: u16,
    /// wLength (the data length).
    pub length: u16,
    /// The OUT payload (empty for IN).
    pub data: Vec<u8>,
}

/// UVC class request numbers (UVC 1.0, on the VS interface).
pub const UVC_SET_CUR: u8 = 0x01;
pub const UVC_GET_CUR: u8 = 0x81;
/// wValue high byte: probe control selector.
pub const UVC_CS_PROBE: u16 = 0x01;
/// wValue high byte: commit control selector.
pub const UVC_CS_COMMIT: u16 = 0x02;

/// Build a UVC SET_CUR (host-to-device) request for interface `iface`
/// (bmRequestType 0x21, wValue = `cs << 8`). Pure.
pub fn uvc_set_cur(iface: u8, cs: u16, body: &[u8]) -> UsbControl {
    UsbControl {
        request_type: 0x21,
        request: UVC_SET_CUR,
        value: cs << 8,
        index: iface as u16,
        length: body.len() as u16,
        data: body.to_vec(),
    }
}

/// Build a UVC GET_CUR (device-to-host) request for `length` bytes.
pub fn uvc_get_cur(iface: u8, cs: u16, length: u16) -> UsbControl {
    UsbControl {
        request_type: 0xA1,
        request: UVC_GET_CUR,
        value: cs << 8,
        index: iface as u16,
        length,
        data: Vec::new(),
    }
}

/// Build the standard GET_DESCRIPTOR(config) request (bmRequestType
/// 0x80, wValue 0x0200, wLength `max`).
pub fn get_descriptor_config(max: u16) -> UsbControl {
    UsbControl {
        request_type: 0x80,
        request: 0x06,
        value: 0x0200,
        index: 0,
        length: max,
        data: Vec::new(),
    }
}

/// Encode the 26-B UVC probe/commit body from a [`ProbeCommit`]
/// (little-endian fields, the UVC 1.0 layout the capture shows).
/// `probe_commit_body(&ProbeCommit::GOLDEN)` equals the golden
/// `DEVICE_COMMIT` body.
pub fn probe_commit_body(pc: &ProbeCommit) -> [u8; 26] {
    let mut b = [0u8; 26];
    b[0..2].copy_from_slice(&pc.bm_hint.to_le_bytes());
    b[2] = pc.format_index;
    b[3] = pc.frame_index;
    b[4..8].copy_from_slice(&pc.frame_interval.to_le_bytes());
    b[18..22].copy_from_slice(&pc.max_video_frame_size.to_le_bytes());
    b[22..26].copy_from_slice(&pc.max_payload_transfer_size.to_le_bytes());
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sensor composite, modeled on the cap7/turn-35 descriptors:
    /// if0 VideoControl (no ep), if1 VideoStreaming alt 0 (no ep),
    /// alt 1 (ep 0x85, 812 B), alt 2 (ep 0x85, raw 0x1324), if2/if3
    /// audio, if4 vendor with bulk ep 0x02/0x82.
    fn sensor_config() -> Vec<u8> {
        fn iface(num: u8, alt: u8, class: u8, sub: u8) -> Vec<u8> {
            let mut v = vec![9, 0x04, num, alt, 0, class, sub, 1, 0];
            v[7] = 0; // iInterface
            v
        }
        fn ep(addr: u8, attrs: u8, wmax: u16) -> Vec<u8> {
            // bLength 7: header(2) + address + attributes + wMaxPacketSize
            // + bInterval - exactly 7 bytes, no padding
            vec![
                7, 0x05, addr, attrs, (wmax & 0xFF) as u8, (wmax >> 8) as u8, 1,
            ]
        }
        let mut blob = vec![9, 0x02, 0x69, 0x01]; // wTotalLength 361, 1 config
        blob.extend_from_slice(&[0x64, 0x00, 0x32, 0x01, 0x01]); // attrs, maxpower, iCfg
        // if0 VC (alt 0, no endpoints)
        blob.extend(iface(0, 0, 0x0E, 0x01));
        // if1 VS alt 0/1/2
        blob.extend(iface(1, 0, 0x0E, 0x02));
        blob.extend(iface(1, 1, 0x0E, 0x02));
        blob.extend(ep(0x85, 0x01, 0x032C));
        blob.extend(iface(1, 2, 0x0E, 0x02));
        blob.extend(ep(0x85, 0x01, 0x1324));
        // if4 vendor with bulk OUT/IN
        blob.extend(iface(4, 0, 0xFF, 0x00));
        blob.extend(ep(0x02, 0x02, 0x0200));
        blob.extend(ep(0x82, 0x02, 0x0200));
        blob
    }

    #[test]
    fn wmax_decode_matches_capture_stride() {
        // alt 1: no multiplier bits -> plain 812
        assert_eq!(wmax_total(0x032C), 812);
        // alt 2: 0x1324 = base 0x324 (804) x (2+1) = 2412 - the exact
        // capture packet stride (the old docs' "4900" was the decimal
        // misread of the raw word)
        assert_eq!(wmax_total(0x1324), 2412);
        // multiplier encoding limits
        assert_eq!(wmax_total(0x0400), 1024);
        assert_eq!(wmax_total(0x1400), 1024 * 3);
        assert_eq!(wmax_total(0x0000), 0);
    }    #[test]
    fn walk_finds_vs_alts_and_vendor_bulk() {
        let alts = walk_config(&sensor_config());
        // if0 VC, if1 alt0/1/2, if4 vendor = 5 interface descriptors
        assert_eq!(alts.len(), 5, "{alts:#?}");
        let vs = video_streaming_iface(&alts).expect("VS iface present");
        assert_eq!(vs.iter().map(|a| a.alt).collect::<Vec<_>>(), vec![0, 1, 2]);
        // the video plane endpoint per alt
        assert!(vs[0].iso_in().is_none(), "alt 0 has no endpoint");
        assert_eq!(vs[1].iso_in().unwrap().address, 0x85);
        assert_eq!(vs[1].iso_in().unwrap().wmax_total, 812);
        assert_eq!(vs[2].iso_in().unwrap().wmax_total, 2412);
        // best streaming alt = alt 2 (2412 B/microframe)
        let best = best_streaming_alt(&vs).expect("an ISO alt");
        assert_eq!(best.alt, 2);
        assert_eq!(best.iso_in().unwrap().wmax_total, 2412);
        // the vendor interface bulk pair
        let vendor = alts.iter().find(|a| a.number == 4).unwrap();
        assert!(!vendor.endpoints.iter().any(|e| e.is_iso_in()));
        assert!(vendor.endpoints.iter().all(|e| !e.is_iso_in()));
    }

    #[test]
    fn walk_stops_cleanly_at_malformed_tail() {
        let mut blob = sensor_config();
        blob.extend([3, 0x04, 0x99]); // garbage after a truncated header
        let alts = walk_config(&blob);
        assert_eq!(alts.len(), 5, "everything before the garbage parses");
        let empty = walk_config(&[]);
        assert!(empty.is_empty());
    }

    #[test]
    fn uvc_control_builders() {
        let set = uvc_set_cur(1, UVC_CS_PROBE, &[0u8; 26]);
        assert_eq!(set.request_type, 0x21);
        assert_eq!(set.request, 0x01);
        assert_eq!(set.value, 0x0100); // cs << 8, matching wire35's decode
        assert_eq!(set.index, 1);
        assert_eq!(set.length, 26);
        let get = uvc_get_cur(1, UVC_CS_COMMIT, 26);
        assert_eq!(get.request_type, 0xA1);
        assert_eq!(get.request, 0x81);
        assert_eq!(get.value, 0x0200);
        let cfg = get_descriptor_config(512);
        assert_eq!((cfg.request_type, cfg.request, cfg.value, cfg.length), (0x80, 6, 0x0200, 512));
    }

    #[test]
    fn probe_commit_body_matches_golden_commit() {
        let body = probe_commit_body(&ProbeCommit::GOLDEN);
        assert_eq!(body, ProbeCommit::DEVICE_COMMIT);
        // the host probe differs only in dwMaxPayloadTransferSize = 0
        let host = uvc_set_cur(1, UVC_CS_PROBE, &body);
        assert_eq!(host.data[22..26], [0xc2, 0x03, 0x00, 0x00]);
    }
}
