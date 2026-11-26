//! V4L2 video-node discovery - the pure sysfs side of the P2.5 Linux
//! video backend (turn 38).
//!
//! The runtime composite (0471:2500) carries its UVC VideoStreaming
//! interface on MI_01; on Linux the uvcvideo kernel driver claims it and
//! exposes it as a /dev/videoN capture node. The vendor interface MI_04
//! stays free for the session transport (`transport::usb`) - the two
//! never conflict, which is exactly why the video test can run beside
//! the session init in ONE process.
//!
//! Which /dev/videoN is the sensor? Matching by node ORDER is fragile
//! (a laptop webcam grabs video0). The robust cross-match is sysfs:
//!
//! * `/sys/bus/usb/devices/<port>/idVendor|idProduct` - the sensor's
//!   USB device dir; `<port>` is the bus-hub-port chain, e.g. "2-1.3";
//! * `/sys/class/video4linux/videoN/device` - a symlink to the USB
//!   *interface* dir the node exposes, e.g. "2-1.3:1.1".
//!
//! A node belongs to the sensor when its interface dir sits under the
//! sensor's device dir (`iface == port` or `iface` starts with
//! `port + ":"`; the colon keeps "2-1" from matching "2-1.3:1.1" - a
//! sibling on the same hub must NOT match). This module is that
//! matcher plus the node scan - pure code, unit-tested. The ioctl
//! layer itself lives in src/examples/video.rs until the first bench run
//! promotes it into the crate (P2.5 acceptance).

use std::fs;
use std::path::{Path, PathBuf};

/// One /dev/videoN node with its sysfs identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoNode {
    /// Device node path, e.g. "/dev/video0".
    pub dev: PathBuf,
    /// Node index (`video12` -> 12).
    pub index: u32,
    /// Symlink-target basename of `/sys/class/video4linux/videoN/device`
    /// (e.g. "2-1.3:1.1" - the USB interface the node exposes). `None`
    /// when the link is missing (non-USB source or an unusual kernel):
    /// the node cannot be tied to the sensor, so the caller needs an
    /// explicit `--device` override.
    pub usb_iface: Option<String>,
}

/// The node number of a `videoN` basename ("video12" -> 12), `None`
/// for anything else.
fn node_index(name: &str) -> Option<u32> {
    let n = name.strip_prefix("video")?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    n.parse().ok()
}

/// Symlink basename of `/sys/class/video4linux/<name>/device`, if any.
fn sysfs_iface(name: &str) -> Option<String> {
    let link = Path::new("/sys/class/video4linux").join(name).join("device");
    let target = fs::read_link(link).ok()?;
    let base = target.file_name()?.to_str()?.to_string();
    if base.is_empty() {
        None
    } else {
        Some(base)
    }
}

/// All `/dev/video*` nodes, index-sorted (video0, video1, ..., video10).
/// A missing /dev entry or an unsortable name just skips that node.
pub fn video_nodes() -> Vec<VideoNode> {
    let mut nodes = Vec::new();
    let Ok(rd) = fs::read_dir("/dev") else { return nodes };
    for e in rd.flatten() {
        let name = e.file_name();
        let name = match name.to_str() {
            Some(s) => s,
            None => continue,
        };
        let Some(index) = node_index(name) else { continue };
        nodes.push(VideoNode {
            dev: PathBuf::from("/dev").join(name),
            index,
            usb_iface: sysfs_iface(name),
        });
    }
    nodes.sort_by_key(|n| n.index);
    nodes
}

/// USB port paths of every attached device with `vid:pid` (hex, e.g.
/// `0x0471`, `0x2500`), e.g. `["2-1.3"]`. Reads
/// `/sys/bus/usb/devices/*/idVendor` + `idProduct`; interface dirs
/// (`2-1.3:1.1`) and root hubs carry no idVendor and are skipped
/// naturally.
///
/// Turn-39 fix: sysfs idVendor/idProduct are HEX ASCII ("0471", "2500")
/// - the first bench run matched nothing because the values were parsed
/// as DECIMAL ("2500" = 2500, not 0x2500 = 9472), so every port list
/// came back empty and no /dev/video node ever matched.
pub fn sensor_usb_ports(vid: u16, pid: u16) -> Vec<String> {
    let mut ports = Vec::new();
    let Ok(rd) = fs::read_dir("/sys/bus/usb/devices") else { return ports };
    for e in rd.flatten() {
        let path = e.path();
        let read_hex =
            |f: &str| -> Option<u16> { parse_sysfs_hex16(&fs::read_to_string(path.join(f)).ok()?) };
        let (Some(v), Some(p)) = (read_hex("idVendor"), read_hex("idProduct")) else { continue };
        if v == vid && p == pid {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                ports.push(name.to_string());
            }
        }
    }
    ports.sort();
    ports.dedup();
    ports
}

/// One sysfs idVendor/idProduct value: plain hex ASCII without prefix
/// ("0471" -> 0x0471, "2500" -> 0x2500). Pure - unit-testable.
pub fn parse_sysfs_hex16(s: &str) -> Option<u16> {
    u16::from_str_radix(s.trim(), 16).ok()
}

/// Does a video node's USB interface dir (`usb_iface`, e.g. "2-1.3:1.1")
/// belong to the USB device at `port` (e.g. "2-1.3")? The colon matters:
/// "2-1" must NOT match "2-1.3:1.1" (a sibling behind the same hub).
pub fn iface_belongs_to_port(usb_iface: &str, port: &str) -> bool {
    usb_iface == port || usb_iface.starts_with(&format!("{port}:"))
}

/// The sensor's nodes among `nodes` (by the sysfs cross-match). Nodes
/// without a resolvable interface dir never match.
pub fn sensor_nodes<'a>(nodes: &'a [VideoNode], ports: &[String]) -> Vec<&'a VideoNode> {
    nodes
        .iter()
        .filter(|n| {
            n.usb_iface
                .as_deref()
                .map(|i| ports.iter().any(|p| iface_belongs_to_port(i, p)))
                .unwrap_or(false)
        })
        .collect()
}

/// A kernel fourcc as the classic 4 chars: `0x32315559` -> "YUYV"
/// (byte 0 is the first character).
pub fn fourcc_str(f: u32) -> String {
    let b = f.to_le_bytes();
    b.iter().map(|c| if c.is_ascii_graphic() || *c == b' ' { *c as char } else { '.' }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_index_parses_and_rejects() {
        assert_eq!(node_index("video0"), Some(0));
        assert_eq!(node_index("video12"), Some(12));
        assert_eq!(node_index("video"), None);
        assert_eq!(node_index("video0a"), None);
        assert_eq!(node_index("vide0"), None);
        assert_eq!(node_index("video-1"), None);
    }

    #[test]
    fn iface_matching_rules() {
        // exact (a node bound to the device dir itself)
        assert!(iface_belongs_to_port("2-1.3", "2-1.3"));
        // the video streaming interface of the same USB device
        assert!(iface_belongs_to_port("2-1.3:1.1", "2-1.3"));
        assert!(iface_belongs_to_port("2-1.3:1.0", "2-1.3"));
        // a sibling device behind the same hub must NOT match
        assert!(!iface_belongs_to_port("2-1.4:1.1", "2-1.3"));
        // a hub parent or an unrelated port must NOT match
        assert!(!iface_belongs_to_port("2-1:1.0", "2-1.3"));
        assert!(!iface_belongs_to_port("1-2:1.1", "2-1.3"));
        // prefix-without-colon traps: "2-1.30" is NOT "2-1.3"
        assert!(!iface_belongs_to_port("2-1.30:1.1", "2-1.3"));
    }

    #[test]
    fn sensor_nodes_filters_by_port() {
        let mk = |i: u32, iface: Option<&str>| VideoNode {
            dev: PathBuf::from(format!("/dev/video{i}")),
            index: i,
            usb_iface: iface.map(|s| s.to_string()),
        };
        let nodes = vec![
            mk(0, Some("1-2:1.0")),   // laptop webcam
            mk(1, Some("2-1.3:1.1")), // the sensor's VS interface
            mk(2, Some("2-1.3:1.0")), // the sensor's VC interface node
            mk(3, Some("2-1.4:1.1")), // sibling device
            mk(4, None),              // no sysfs link -> never matches
        ];
        let ports = vec!["2-1.3".to_string()];
        let hits = sensor_nodes(&nodes, &ports);
        assert_eq!(hits.iter().map(|n| n.index).collect::<Vec<_>>(), vec![1, 2]);
        // multiple sensors attach to multiple ports
        let ports2 = vec!["2-1.3".to_string(), "1-2".to_string()];
        let hits2 = sensor_nodes(&nodes, &ports2);
        assert_eq!(hits2.iter().map(|n| n.index).collect::<Vec<_>>(), vec![0, 1, 2]);
    }

    #[test]
    fn sysfs_hex_ids_parse() {
        // sysfs idVendor/idProduct are HEX ASCII: the sensor's ids parse
        // to the crate constants. This is the turn-39 bench fix: the
        // first video probe parsed them as DECIMAL ("2500" -> 2500),
        // matched nothing and every port list came back empty.
        assert_eq!(parse_sysfs_hex16("0471"), Some(0x0471));
        assert_eq!(parse_sysfs_hex16("2500"), Some(0x2500));
        assert_eq!(parse_sysfs_hex16("2600"), Some(0x2600));
        assert_eq!(parse_sysfs_hex16("1d6b"), Some(0x1d6b));
        // trailing newline (a real sysfs read) and a tidy value agree
        assert_eq!(parse_sysfs_hex16("0471\n"), Some(0x0471));
        // the decimal trap the old code fell into: 2500 != 0x2500
        assert_ne!(parse_sysfs_hex16("2500"), Some(2500));
        // garbage never matches
        assert_eq!(parse_sysfs_hex16("0x2500"), None);
        assert_eq!(parse_sysfs_hex16(""), None);
        assert_eq!(parse_sysfs_hex16("zz"), None);
    }

    #[test]
    fn fourcc_prints_graphic_chars() {
        // v4l2_fourcc(a,b,c,d) = a | b<<8 | c<<16 | d<<24: 'Y','U','Y','V'
        assert_eq!(fourcc_str(0x5659_5559), "YUYV");
        assert_eq!(fourcc_str(u32::from_le_bytes(*b"GREY")), "GREY");
        assert_eq!(fourcc_str(0x3231_5559), "YU12"); // 'Y','U','1','2'
                                                     // non-graphic bytes render as dots, never panic
        assert_eq!(fourcc_str(0x0001_4142), "BA..");
    }
}
