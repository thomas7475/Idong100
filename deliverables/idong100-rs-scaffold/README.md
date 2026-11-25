# idong100-rs - Rust driver for the Toltech i-Dong Sensor V2.6

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
