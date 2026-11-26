#!/usr/bin/env python3
"""make_scaffold.py - write the idong100-rs workspace scaffold exactly as
section 4 of the implementation draft specifies. Doc-comment stubs only:
every file names its duty and cites where the protocol facts come from, so
P1.1 can start with `cargo check` (memory limits: never full builds)."""
import os

ROOT = '/home/z/my-project/download/idong100-rs-scaffold'


def w(rel, content):
    path = os.path.join(ROOT, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w') as f:
        f.write(content)
    print(f'wrote {rel} ({len(content)} B)')


# ---------------------------------------------------------------- workspace
w('Cargo.toml', '''[workspace]
resolver = "2"
members = ["idong", "idong-transport", "idong-ros2"]

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "MIT"
repository = "https://github.com/thomas7475/Idong100"

[workspace.dependencies]
thiserror = "2"
byteorder = "1"
crossbeam-channel = "0.5"
log = "0.4"
rusb = "0.9"

[profile.release]
debug = true  # keep symbols for on-device diagnosis
''')

w('rustfmt.toml', '''# Project style: keep diffs small, imports grouped, 100 columns.
imports_granularity = "Module"
group_imports = "StdExternalCrate"
use_small_heuristics = "Max"
max_width = 100
''')

w('README.md', '''# idong100-rs - Rust driver for the Toltech i-Dong Sensor V2.6

Native, idiomatic Rust driver with ROS 2 output for the i-Dong stereo
camera (USB 0471:2600 bootloader / 0471:2500 runtime). Design contract:
`docs/IMPLEMENTATION_DRAFT.pdf` (turn 13) in the project downloads.

## Workspace layout (4 crates)

| Crate | Duty | Depends on |
|---|---|---|
| `idong` | pure protocol core: frame codec, boot + command state machines, xEye3Ddata parser, EEPROM calibration, stream | thiserror, byteorder, log, crossbeam-channel (NO rusb, NO rclrs) |
| `idong-transport` | USB I/O: discovery, bulk endpoints, re-enumeration wait | rusb, idong |
| `idong_msgs` | ROS message/service definitions (rosidl) | rosidl runtime |
| `idong-ros2` | ROS 2 lifecycle node: params, QoS, diagnostics | rclrs, idong, idong_msgs |

## Status

Scaffold only - doc-comment stubs. Phases P0..P4 in the draft roadmap.
Run `cargo check -p idong` to verify (sandbox note: never run full
`cargo build` of the whole workspace - not enough RAM; `cargo check`
and single-bin builds only).

## Safety rules (hard constraints)

1. Read-only until the USBPcap boot transcript is byte-confirmed.
2. No speculative writes to EP 0x02; only decoded vendor sequences.
3. Firmware files sha256-verified before upload; abort on CRC mismatch.
4. NEVER write the T24C16A EEPROM (factory calibration).
5. Write-capable examples require `--i-know-the-write-risks`.

## Firmware

NOT redistributed. Users supply vendor v2.6.0.6 files; the
`FirmwareSet` verifier pins the SHA-256 hashes from the turn-12
manifest (analysis/reports/all_extracted_sha256.tsv):

- ens_code.ldr        (Blackfin BF527, 54,420 B)
- A_a_v4.4_BG0313.bit (Spartan-3 XC3S400, 235,930 B)
- app_m8051_2611.hex  (8051, 21,362 B records)

## ROS 2 build

colcon with `colcon-cargo` + `colcon-ros-cargo` plugins; rclrs from
crates.io (0.8 line); `idong_msgs` generates Rust via
rosidl_generator_rs. Custom messages: MarkerArray, HandleEvent;
service: DeviceInfo.

## License

MIT for this code. Vendor firmware/licence constraints documented in
docs/PROTOCOL.md - private interoperability research only.
''')

w('.gitignore', '''/target
Cargo.lock
/firmware/*.ldr
/firmware/*.bit
/firmware/*.hex
''')

w('docs/PROTOCOL.md', '''# PROTOCOL.md - wire facts (P0.1 deliverable; to be filled from FINDINGS)

Every row must cite its source (DLL + RVA). Constants here are encoded
verbatim into `idong/src/protocol/mod.rs`.

## USB identity

| Item | Value | Source |
|---|---|---|
| bootloader | 0471:2600, iface 0, EP 0x02 OUT / 0x82 IN, maxpacket 64 | xeyez.inf |
| runtime | 0471:2500 composite, vendor MI_04 | xeyez.inf, DriverConfig.ini |

## Boot framing (PU/OD magic)

| Item | Value | Source |
|---|---|---|
| H2D magic | 0x7f7f5550 "PU" | xEyeUSB.dll |
| D2H magic | 0x7f7f444f "OD" | xEyeUSB.dll |
| packet size | 64 B type-3 | _InitBlobUpload |
| timeouts | 3 s ready / 6 s packet / 300 ms x 10 dev-ID / 1 s reset | xEyeUSB.dll |

## Runtime framing (7F 55)

| Item | Value | Source |
|---|---|---|
| layout | 7F 55; cmd@[2]; b2-b4@[3..5]; u16 arg@[6..7] byte-swapped; payload <= 56 B | xeyezHal builder @0x10003010 |
| cmd 3 | register write, 8 B chunks | SetCmosTable loop |
| cmd 5 | blob data read | GetBlobData |
| cmd 6 | GET_IMG_START (status 1) | GetImageBegin |
| cmd 15 | classifier/reg write | chunk loop @0x10003a50 |

## Marker record (xEye3Ddata)

0x3BC B: frame counter @+0 (overlays slot 0); 10 x 88 B slots;
quality i32 @ slot+0x24 (>= 50 tracked); gate f64 @ slot+0x18;
mode flag u32 @ +0x3B8; pipe form 0x208 B. Source: xEyeClass.dll
GetADataFromUSB @RVA 0x20f0.

## Open items (P2.3 USBPcap gate)

- multi-firmware staging order (ldr/bit/hex)
- CRC variant announced in count+CRC step
- exact count/CRC payload framing

## Firmware sha256 pins

From analysis/reports/all_extracted_sha256.tsv (turn 12). Fill in at
P0.3 from the manifest; verify with `idong/examples/verify_firmware`.
''')

# =============================== idong_msgs =================================
w('idong_msgs/package.xml', '''<?xml version="1.0"?>
<package format="3">
  <name>idong_msgs</name>
  <version>0.1.0</version>
  <description>Custom messages for the i-Dong sensor driver</description>
  <maintainer email="maintainer@example.org">idong100 project</maintainer>
  <license>MIT</license>

  <buildtool_depend>ament_cmake</buildtool_depend>
  <buildtool_depend>rosidl_default_generators</buildtool_depend>

  <depend>std_msgs</depend>
  <depend>sensor_msgs</depend>
  <depend>diagnostic_msgs</depend>

  <exec_depend>rosidl_default_runtime</exec_depend>

  <member_of_group>rosidl_interface_packages</member_of_group>

  <export>
    <build_type>ament_cmake</build_type>
  </export>
</package>
''')

w('idong_msgs/CMakeLists.txt', '''cmake_minimum_required(VERSION 3.8)
project(idong_msgs)

find_package(ament_cmake REQUIRED)
find_package(rosidl_default_generators REQUIRED)
# Rust generation: rosidl_generator_rs must be in the workspace (see
# docs in the draft, section 2.2); it registers itself as an extension.
find_package(std_msgs REQUIRED)
find_package(sensor_msgs REQUIRED)
find_package(diagnostic_msgs REQUIRED)

rosidl_generate_interfaces(${PROJECT_NAME}
  "msg/MarkerArray.msg"
  "msg/HandleEvent.msg"
  "srv/DeviceInfo.srv"
  DEPENDENCIES std_msgs sensor_msgs
)

ament_package()
''')

w('idong_msgs/msg/Marker.msg', '''# One tracked retro-reflector from the i-Dong sensor.
# Mirrors slot fields of the vendor xEye3Ddata record (xEyeClass.dll).
std_msgs/Header header   # stamp = host receive time; frame_id = sensor
uint32 seq              # device frame counter (continuity: delta <= 1)
uint8 slot              # 0..9 observed (chip spec: up to 15)
float32 x               # mm, device frame
float32 y               # mm
float32 z               # mm
int32 quality           # slot+0x24; >= 50 means tracked (vendor gate)
float64 age             # slot+0x18 gate value (timestamp/distance)
''')

w('idong_msgs/msg/MarkerArray.msg', '''# Up to 10-15 tracked markers per 20 ms frame (50 fps stream).
std_msgs/Header header
uint32 seq
Marker[] markers
uint8 link_quality      # 0 green / 1 yellow / 2 red (vendor thresholds)
uint8 mode              # record +0x3B8 mode flag
bool dropped            # frame-counter gap detected
''')

w('idong_msgs/msg/HandleEvent.msg', '''# RF handle (wand / airtouch wristband) event over the 2.4 GHz link.
std_msgs/Header header
uint8 event             # 0 FOUND, 1 NOT_FOUND, 2 TIMEOUT, 3 FOUND_OVER,
                        # 4 BUTTON_DOWN, 5 BUTTON_UP
uint8 handle_id
uint8 button            # bitmask when event is BUTTON_*
''')

w('idong_msgs/srv/DeviceInfo.srv', '''# Vendor handle-match analog: identity of the attached sensor.
string serial           # 32-char device serial (EEPROM)
string dsp_version      # device-ID exchange string at boot
string firmware_hash    # sha256 prefix of uploaded firmware set
float32 baseline_mm      # EEPROM T magnitude (100.00 expected)
float32 focal_px         # mean EEPROM focal length (432.33 expected)
---
bool ok
string message
''')

# ================================== idong ===================================
w('idong/Cargo.toml', '''[package]
name = "idong"
description = "Pure-Rust protocol core for the Toltech i-Dong Sensor V2.6"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
thiserror.workspace = true
byteorder.workspace = true
log.workspace = true
crossbeam-channel.workspace = true

# NOTE (CI dependency audit): this crate must NOT depend on rusb or rclrs.
''')

w('idong/src/lib.rs', '''//! # idong - pure protocol core for the i-Dong Sensor V2.6
//!
//! Zero USB and zero ROS dependencies: everything here is testable with
//! `cargo test` alone, against golden vectors under `tests/data/`.
//! The crate facade is [`Idong`]; see the implementation draft (turn 13)
//! section 4 for the file map and section 7 for the phase plan.
//!
//! Facts in this crate are cited to their decompiled source in
//! `docs/PROTOCOL.md` - the single source of truth for constants.

#![deny(unsafe_code)]

pub mod calib;
pub mod error;
pub mod handle;
pub mod marker;
pub mod mock;
pub mod protocol;
pub mod stream;
pub mod transport;

pub use calib::{DeviceIdentity, StereoCalib};
pub use error::{BootStep, IdongError};
pub use handle::HandleEvent;
pub use marker::{Marker, MarkerFrame, Quality};
pub use stream::{Idong, StreamStats};

/// Firmware set the user must supply (never bundled).
/// Hashes pinned from the turn-12 manifest.
pub struct FirmwareSet {
    pub ldr: Vec<u8>,  // ens_code.ldr       Blackfin BF527
    pub bit: Vec<u8>,  // A_a_v4.4_BG0313.bit  Spartan-3
    pub hex: Vec<u8>,  // app_m8051_2611.hex   8051
}

// TODO(P1): verify_firmware() pins sha256 before any upload (safety #3).
''')

w('idong/src/error.rs', '''//! Error model: one variant per failing state, no panics.
//!
//! `BootStep` mirrors the vendor `_InitBlobUpload` sequence one step per
//! variant so a failure pins the exact state (draft section 6, table 6).

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum IdongError {
    #[error("no i-Dong sensor found (looked for 0471:2600 or 0471:2500 MI_04)")]
    NotFound,
    #[error("device is in bootloader state; firmware upload required first")]
    BootloaderState,
    #[error("boot failed at {step:?}: {reason}")]
    Boot { step: BootStep, reason: String },
    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("bad frame magic {magic:#010x}, expected {expected:#010x}")]
    BadMagic { magic: u32, expected: u32 },
    #[error("CRC mismatch after upload: announced {announced:#010x}, device reported {device:#010x}")]
    CrcMismatch { announced: u32, device: u32 },
    #[error("USB error: {0}")]
    Usb(#[from] rusb::Error),
    #[error("firmware file {path} failed integrity check")]
    FirmwareIntegrity { path: PathBuf },
    #[error("write-capable operation refused: pass --i-know-the-write-risks")]
    WriteRiskGate,
}

/// Boot state-machine steps, in vendor sequence order.
#[derive(Debug, Clone, Copy)]
pub enum BootStep {
    DspStart,
    ReadyToLoad,
    CountAndCrc,
    PacketStream,
    CrcAck,
    DeviceId,
    DspReset,
    HandleConfig,
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("short buffer: need {need} bytes, have {have}")]
    Short { need: usize, have: usize },
    #[error("unexpected frame type {0:#06x}")]
    BadType(u16),
    #[error("status byte {0:#04x} not in expected set")]
    BadStatus(u8),
}
''')

w('idong/src/protocol/mod.rs', '''//! Wire constants - one home, every value cited.
//!
//! Sources: xeyez.inf, xEyeUSB.dll (_InitBlobUpload), xeyezHal.dll v2.6.0.6
//! (packet builder @VA 0x10003010, engine @0x100030a0), xEyeClass.dll
//! (GetADataFromUSB @RVA 0x20f0), xEyeUsb.ini, T24C16A dump decode.
//! Full table: docs/PROTOCOL.md.

pub mod boot;
pub mod cmd;
pub mod frames;
pub mod records;

use std::time::Duration;

// ---- USB identity (xeyez.inf) ----
pub const VID: u16 = 0x0471;
pub const PID_BOOTLOADER: u16 = 0x2600;
pub const PID_RUNTIME: u16 = 0x2500; // composite; vendor interface = 4
pub const IFACE_VENDOR: u8 = 4;

// ---- Boot framing (xEyeUSB.dll) ----
pub const MAGIC_H2D: u32 = 0x7f7f_5550; // bytes "PU" + 0x7f 0x7f (LE)
pub const MAGIC_D2H: u32 = 0x7f7f_444f; // bytes "OD" + 0x7f 0x7f (LE)
pub const BOOT_PACKET: usize = 64;
pub const TIMEOUT_READY: Duration = Duration::from_secs(3);
pub const TIMEOUT_PACKET: Duration = Duration::from_secs(6);
pub const TIMEOUT_RESET: Duration = Duration::from_secs(1);
pub const DEVICE_ID_POLL: Duration = Duration::from_millis(300);
pub const DEVICE_ID_TRIES: u8 = 10;

// ---- Runtime framing (xeyezHal.dll) ----
pub const RUNTIME_MAGIC: [u8; 2] = [0x7f, 0x55];
pub const RUNTIME_TX_TIMEOUT: Duration = Duration::from_secs(1);
pub const RUNTIME_RX_TIMEOUT: Duration = Duration::from_secs(5);
pub const RUNTIME_PAYLOAD_MAX: usize = 56;

// ---- Keep-alive / link (xEyeClass reader thread) ----
pub const KEEPALIVE_RX: u16 = 0x0fff;
pub const KEEPALIVE_TX: u16 = 0x0ffe;

// ---- Marker record (xEyeClass.dll GetADataFromUSB) ----
pub const MARKER_SLOTS: usize = 10;
pub const MARKER_SLOT_SIZE: usize = 88;
pub const MARKER_RECORD_MIN: usize = 0x3bc;
pub const QUALITY_GATE: i32 = 50; // slot+0x24 >= 50 -> tracked

// ---- Stream config defaults (xEyeUsb.ini) ----
pub const DEFAULT_FPS: u32 = 50; // xEyeFrame=50 (USBComFPS=35 transport)
pub const QUALITY_WINDOW: usize = 200; // RingSize
''')

w('idong/src/protocol/frames.rs', '''//! Frame codec + resynchronizing stream decoder.
//!
//! Boot framing: `[u32 magic LE][u16 type LE][payload]`; no length field,
//! so the decoder scans for magic on resync (draft section 6: decode must
//! be total - arbitrary bytes never panic; fuzz target in tests/).

use super::{MAGIC_D2H, MAGIC_H2D};

#[derive(Debug, Clone)]
pub struct Frame {
    pub magic: u32,
    pub kind: u16,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn h2d(kind: u16, payload: &[u8]) -> Self {
        Frame { magic: MAGIC_H2D, kind, payload: payload.to_vec() }
    }
}

/// Push bytes, get complete frames; skips garbage between magics.
// TODO(P1.2): implement + fuzz; golden vectors under tests/data/.
pub struct FrameDecoder { buf: Vec<u8> }
impl FrameDecoder {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Frame> { let _ = chunk; Vec::new() }
}
''')

w('idong/src/protocol/boot.rs', '''//! `_InitBlobUpload` state machine (decoded from xEyeUSB.dll).
//!
//! Sequence: TX DSP-start -> RX type 2 "Ready to load program!" ->
//! TX type 2 packet-count+CRC -> TX type-3 64 B packets (6 s each) ->
//! RX type 3 CRC result (abort on mismatch) -> device-ID exchange
//! (300 ms x 10) -> TX type 7 DSP reset. Staging order of the three
//! firmware images is open item #1 - resolved at the P2.3 USBPcap gate.
//!
//! Safety: refuses to run unless the risk gate is passed (error.rs
//! `WriteRiskGate`); firmware sha256-verified first.

use crate::error::BootStep;

pub struct BootUploader<'a> { pub blob: &'a [u8] }
impl<'a> BootUploader<'a> {
    /// Runs the full vendor sequence over any `Transport`.
    // TODO(P1.4): implement against MockTransport; replay test asserts
    // the exact byte sequence (tests/mock_replay.rs).
    pub fn run(&mut self) -> Result<crate::calib::DeviceIdentity, crate::IdongError> {
        Err(crate::IdongError::Boot { step: BootStep::DspStart, reason: "P1.4 not implemented".into() })
    }
}
''')

w('idong/src/protocol/cmd.rs', '''//! Runtime command engine - the "7F 55" framing (xeyezHal.dll v2.6.0.6).
//!
//! H2D: [0]=0x7F [1]=0x55 [2]=cmd [3..5]=b2-b4 [6..7]=u16 arg (BYTE
//! -SWAPPED on the wire) [8..]=payload (<= 56 B).  D2H: [4]=status.
//! Commands: 3 register write (8 B chunks), 5 blob read, 6 GET_IMG_START
//! (status 1), 15 classifier write.  Timeouts 1 s TX / 5 s RX; data
//! pipe 3 for GetImageData.
//!
//! Safety: only vendor-decoded commands are representable (error.rs
//! rejects everything else - rule "no speculative writes").

pub enum Cmd { RegWrite = 3, BlobRead = 5, ImgStart = 6, Classifier = 15 }

// TODO(P1.4): CmdEngine { execute(cmd, arg, payload) -> Result<Response> }
''')

w('idong/src/protocol/records.rs', '''//! xEye3Ddata record parser (xEyeClass.dll, GetADataFromUSB @RVA 0x20f0).
//!
//! Layout (verified structurally, field semantics partially open):
//! 0x3BC B minimum: frame counter u32 @ +0 (overlays slot 0's first u32);
//! 10 slots x 88 B at +0: gate f64 @ slot+0x18, quality i32 @ slot+0x24
//! (>= 50 tracked); 72 B trailer; mode flag u32 @ +0x3B8 (==1 skip).
//! Pipe form: 0x208 B after the RVA 0x25e0 transform.
//!
//! TODO(P1.2): total decode + golden-vector test; open item #4: which
//! slot bytes are x/y/z vs gate - resolve via xEyeClass 0x25e0 disasm.
''')

w('idong/src/calib.rs', '''//! T24C16A EEPROM layout (2 KiB, dumped and decoded in earlier turns).
//!
//! Tag 0xef0123456789abcd; records at 0x0100/0x0180/0x0192/0x0300 with
//! +0x400 mirror; record = [payload][checksum = sum&0xFF][0xFF]; the
//! calibration record is 28 x f32 in OpenCV convention (fx fy cx cy x2,
//! dist[5] x2, R[3x3], T[3]).  Known-good values: |T| = 100.00 mm
//! baseline, mean f = 432.33 px, |k1| < 0.05, cx ~312-313 px (640-px
//! frames).  Serial: AB460DC8BA12C6840B49CD5786C329DD.
//!
//! Read-only: EpromRead only; the write path is deliberately NOT
//! implemented (safety rule #4).

#[derive(Debug, Clone)]
pub struct StereoCalib { /* 28 x f32 LE, OpenCV convention */ }

#[derive(Debug, Clone)]
pub struct DeviceIdentity { pub serial: [u8; 32] }

// TODO(P1.3): parse_eeprom(dump: &[u8; 2048]) -> (StereoCalib, DeviceIdentity)
// with checksum verification; tests/eeprom_decode.rs asserts the
// known-good values above.
''')

w('idong/src/stream.rs', '''//! Reader thread + bounded channel + statistics.
//!
//! One owning reader thread per device (the xEyeClass analog): bulk reads
//! (4096 B chunks / 16 KiB accumulation), resync decode, keep-alive
//! answered inline (0xFFF -> 0xFFE within 1 s), MarkerFrame + Event into
//! a bounded crossbeam channel with drop-oldest backpressure; stats via
//! atomics (drops, resyncs, quality window of the last 200 frames).

/// Facade over a live (or mocked) device.
pub struct Idong { /* transport + reader handle + identity */ }

/// Snapshot of stream statistics (all atomics, lock-free read).
#[derive(Debug, Default)]
pub struct StreamStats {
    pub frames: u64,
    pub drops: u64,
    pub resyncs: u64,
    pub last_quality: u8, // 0 green / 1 yellow / 2 red
}

// TODO(P1.5): Idong::open_or_boot(fw) -> Self; frames()/events()/stats().
''')

w('idong/src/marker.rs', '''//! Public data model for marker frames.

#[derive(Debug, Clone)]
pub struct Marker {
    pub slot: u8,          // 0..=9 observed (chip: up to 15)
    pub x_mm: f32,
    pub y_mm: f32,
    pub z_mm: f32,
    pub quality: i32,      // slot+0x24; >= 50 = tracked
    pub age: f64,          // slot+0x18 gate value
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Quality { Green, Yellow, Red } // vendor thresholds 20/35/35

#[derive(Debug, Clone)]
pub struct MarkerFrame {
    pub frame_counter: u32, // record +0; continuity delta <= 1
    pub mode: u32,           // record +0x3B8
    pub markers: Vec<Marker>,
    pub link_quality: Quality,
    pub dropped: bool,
}
''')

w('idong/src/handle.rs', '''//! RF handle (wand / airtouch wristband) events.
//!
//! TSOL_DEVICE_HANDLE_FOUND / NOT_FOUND / TIMEOUT / FOUND_OVER ack
//! sequence over the 2.4 GHz link; NeedHandleMatch=1, MonitorList=10
//! in xEyeUsb.ini.  Surfaces as an Event queue on the stream channel.

#[derive(Debug, Clone, Copy)]
pub enum HandleEvent {
    Found,
    NotFound,
    Timeout,
    FoundOver,
    ButtonDown { button: u8 },
    ButtonUp { button: u8 },
}
''')

w('idong/src/transport.rs', '''//! Transport abstraction - the seam that makes CI possible.
//!
//! `idong-transport` implements this with rusb; `mock.rs` implements it
//! with recorded sessions.  The core never touches rusb directly.

pub trait Transport {
    /// Bulk write to EP 0x02 (commands/upload).  The ONLY write path.
    fn write(&mut self, buf: &[u8]) -> Result<(), crate::IdongError>;
    /// Bulk read from EP 0x82 (data).
    fn read(&mut self, max: usize) -> Result<Vec<u8>, crate::IdongError>;
}
''')

w('idong/src/mock.rs', '''//! MockTransport - record/replay for CI.
//!
//! Replays a recorded session (bytes + expected reads) and records
//! everything the state machines write, so tests/mock_replay.rs can
//! assert the exact vendor boot sequence byte-for-byte, including
//! failure injections (timeout at ready-to-load, CRC mismatch,
//! device-ID poll exhaustion).

// TODO(P1.4): implement Transport; session files under tests/data/.
''')

w('idong/examples/list.rs', '''//! Enumerate i-Dong devices, report lifecycle state. READ-ONLY.
//! `cargo run -p idong-transport --example list` after P2.1.

fn main() { println!("TODO(P2.1): enumerate 0471:2600 / 0471:2500 MI_04"); }
''')

w('idong/examples/probe_bootloader.rs', '''//! Read-only probe of EP 0x82 on the bootloader - the Rust port of
//! idong_probe.py.  NEVER writes (safety rule #1).  P2.1.
''')

w('idong/examples/upload_firmware.rs', '''//! Guarded full boot: firmware upload + re-enumeration.  P3.1.
//! Refuses to run without --i-know-the-write-risks (error::WriteRiskGate).
''')

w('idong/examples/dump_frames.rs', '''//! Runtime marker dump to JSONL.  P3.4.  Runs with --transport mock in CI.
''')

w('idong/tests/README.md', '''Test files (draft section 8, table 7):
- frames_golden.rs   codec vs golden vectors (tests/data/)
- eeprom_decode.rs   T24C16A checksums + known-good calibration values
- mock_replay.rs     boot + stream state machines over MockTransport
- hardware.rs        bench-only, every test #[ignore] by default
- data/              golden vectors: OD frames, EEPROM dump, scripts
''')

# ============================== idong-transport ==============================
w('idong-transport/Cargo.toml', '''[package]
name = "idong-transport"
description = "USB transport for the i-Dong sensor (rusb/libusb)"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
idong = { path = "../idong" }
rusb.workspace = true
log.workspace = true
''')

w('idong-transport/src/lib.rs', '''//! USB transport: discovery (0471:2600 / 0471:2500 MI_04), bulk I/O on
//! EP 0x02 / 0x82, re-enumeration wait after firmware upload.
//! Implements `idong::Transport`.  All hardware tests are #[ignore].

pub mod reenum;
pub mod udev;
pub mod usb;

pub use usb::{list_devices, DeviceState, UsbTransport};
''')

w('idong-transport/src/usb.rs', '''//! open bootloader (0471:2600 iface 0) / runtime (0471:2500 MI_04),
//! bulk read/write.  Bootloader maxpacket is 64 (quirk: full-speed
//! value at high speed - accepted).  Runtime path reads 4096 B chunks.

pub enum DeviceState { Bootloader, Runtime }
// TODO(P2.1): list_devices() -> Vec<UsbDeviceInfo>; UsbTransport impl
// Transport; detach kernel drivers defensively (Windows: WinUSB).
''')

w('idong-transport/src/reenum.rs', '''//! After firmware upload the device re-enumerates as 0471:2500.
//! Wait via rusb hotplug if available, else poll with backoff; then
//! open_runtime() claims MI_04 (vendor bulk interface).
// TODO(P3.2)
''')

w('idong-transport/src/udev.rs', '''//! udev rule for unprivileged access (no root requirement in the lib):
//! SUBSYSTEM=="usb", ATTRS{idVendor}=="0471", ATTRS{idProduct}=="2600",
//! MODE="0666"  (+ "2500" companion rule for the runtime composite).
// TODO(P2.2): rule text + install helper.
''')

# ================================ idong-ros2 ================================
w('idong-ros2/package.xml', '''<?xml version="1.0"?>
<package format="3">
  <name>idong_ros2</name>
  <version>0.1.0</version>
  <description>Rust ROS 2 driver node for the Toltech i-Dong sensor</description>
  <maintainer email="maintainer@example.org">idong100 project</maintainer>
  <license>MIT</license>

  <buildtool_depend>ament_cargo</buildtool_depend>
  <build_type>ament_cargo</build_type>

  <depend>rclrs</depend>
  <depend>idong_msgs</depend>
  <exec_depend>ros2launch</exec_depend>
</package>
''')

w('idong-ros2/Cargo.toml', '''[package]
name = "idong-ros2"
description = "ROS 2 (rclrs) node for the i-Dong sensor"
edition.workspace = true
version.workspace = true
license.workspace = true

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
idong = { path = "../idong" }
idong_msgs = { path = "../idong_msgs/msg" }  # generated: path set by colcon
rclrs = "0.8"
log.workspace = true
''')

w('idong-ros2/src/lib.rs', '''//! ROS 2 adapter crate - the ONLY crate that sees rclrs (draft 3.1).
//!
//! Node surface (draft table 4): /idong/markers (sensor-data QoS),
//! /idong/events (reliable), /idong/diagnostics, /idong/time_ref,
//! get_device_info service, ~/transition.  Parameters: draft table 5.
//! Threading: reader thread (core) -> bounded channel -> executor
//! spin loop (draft 5.2).

pub mod diagnostics;
pub mod lifecycle;
pub mod node;
pub mod params;
pub mod pubsub;

// rclrs_create_node! registration lands at P4.1; see draft section 5
// for the ~40-line entry-point skeleton.
''')

w('idong-ros2/src/node.rs', '''//! IdongNode: two threads, one bounded channel (draft 5.2).
//!
//! Executor thread spins the node, drains MarkerFrame/Event values,
//! stamps with receive time (device provides a frame counter, not a
//! clock - time_ref exposes the pairs), converts to idong_msgs, applies
//! the parameter snapshot (Arc<Mutex<TrackingConfig>>, snapshotted per
//! frame; no locks on the hot path).
// TODO(P4.1)
''')

w('idong-ros2/src/lifecycle.rs', '''//! Lifecycle semantics without framework lifecycle (draft 5.3).
//!
//! rclrs has no managed-lifecycle node type yet, so we implement the
//! same four states (unconfigured/inactive/active/finalized) with
//! standard transition names, exposed as a ~/transition service and
//! reflected on /idong/status.  Allocation at configure, USB discovery
//! at activate, teardown at shutdown (ros2_ouster order).  When rclrs
//! gains native lifecycle, this whole file swaps out - nothing else.
// TODO(P4.1)
''')

w('idong-ros2/src/params.rs', '''//! Parameter surface (draft table 5): serial_number, frame_id,
//! publish_hz_min (30), quality_threshold (50), link_quality_window
//! (200), buffer_depth (5), firmware_dir, enable_handle_events (true),
//! i_know_the_write_risks (false - must be set for any write-capable
//! interface).  All reconfigurable where the hardware allows; live
//! reapply via the config snapshot.
// TODO(P4.1)
''')

w('idong-ros2/src/pubsub.rs', '''//! Topics + QoS (draft table 4): markers best-effort keep-last 5 (a late
//! frame is worthless), events reliable keep-last 10 (a lost click is a
//! lost action), diagnostics + time_ref reliable.  Converters from core
//! data model to idong_msgs types live here - the only place message
//! types leak into our code.
// TODO(P4.1)
''')

w('idong-ros2/src/diagnostics.rs', '''//! StreamStats -> diagnostic_msgs/DiagnosticArray: link quality
//! (green/yellow/red), drops, resyncs, degraded-link flag when below
//! publish_hz_min.  Sanity-check timeout: if frames stop, say so
//! (sensor-package thread guidance).
// TODO(P4.3)
''')

w('idong-ros2/launch/idong.launch.py', '''"""Launch the i-Dong node with lifecycle-style bring-up (P4.2).

Starts the node, calls ~/transition configure+activate, and passes the
parameter file with sane defaults (see draft table 5).
"""
from launch import LaunchDescription
from launch_ros.actions import Node


def generate_launch_description():
    return LaunchDescription([
        Node(
            package="idong_ros2",
            executable="idong_node",
            name="idong",
            output="screen",
            parameters=["params.yaml"],
        ),
    ])
''')

w('idong-ros2/params.yaml', '''# Sane defaults (draft table 5). firmware_dir must be set by the user.
/idong:
  ros__parameters:
    serial_number: ""
    frame_id: idong
    publish_hz_min: 30
    quality_threshold: 50
    link_quality_window: 200
    buffer_depth: 5
    firmware_dir: ""
    enable_handle_events: true
    i_know_the_write_risks: false
''')

# ---------------------------------------------------------------- done
n = sum(len(files) for _, _, files in os.walk(ROOT))
print(f'\\nscaffold root: {ROOT}  ({n} files)')
