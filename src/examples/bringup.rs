//! P2.2 live read-only bring-up - THE hardware test of the idong crate.
//!
//! `cargo run --features usb --example bringup`
//!
//! What this does on the REAL sensor (everything read-only, every byte
//! also sent by the vendor engine):
//!   1. enumerate - which sensor state is on the bus?
//!   2. runtime (0471:2500): open MI_04, probe the version ("2619"),
//!      read the four EEPROM values of the session init
//!      (device ID, edition tag, code B, factory calibration)
//!   3. bootloader (0471:2600): version probe only ("2600") + what to do
//!   4. print PASS/FAIL per step, and the exact bytes sent (compare with
//!      the captures: every write is an 0x0107 EEPROM read request)
//!
//! What this never does: CMOS register writes (0x0104/0x0203/0x0103),
//! polls, the stream poll, flash, EEPROM writes. The only failure mode
//! against real hardware is a failed read.

#[cfg(feature = "usb")]
fn main() {
    println!("=== idong P2.2 read-only bring-up ===");
    println!();

    println!("[1/3] enumerate");
    let devices = idong::list_devices();
    if devices.is_empty() {
        println!("      no sensor found - PASS criteria not met");
        println!("      looked for 0471:2600 (bootloader) and 0471:2500 (runtime)");
        println!("      - plug the sensor in; in a VM also attach it to the VM");
        std::process::exit(1);
    }
    for d in &devices {
        println!("      bus {:3} addr {:3} {} {:?}", d.bus, d.address, d.vid_pid(), d.state);
    }
    println!("      PASS: {} sensor device(s) found", devices.len());
    println!();

    let runtime = devices.iter().find(|d| d.pid == idong::protocol::PID_RUNTIME);
    let boot = devices.iter().find(|d| d.pid == idong::protocol::PID_BOOTLOADER);

    if let Some(d) = runtime {
        println!(
            "[2/3] runtime device 0471:2500, opening MI_04 read-only (bus {}, addr {})",
            d.bus, d.address
        );
        let mut t = match idong::UsbTransport::open_runtime() {
            Ok(t) => t,
            Err(e) => {
                println!("      FAIL: cannot open: {e}");
                println!("      - permissions? install the udev rule (src/transport/udev.rs)");
                println!("      - is the vendor engine / another process holding it?");
                std::process::exit(1);
            }
        };
        println!("      PASS: vendor interface MI_04 claimed (UVC/UAC untouched)");

        println!("[3/3] read-only session (version + four EEPROM reads)");
        let phase = match idong::Session::probe(&mut t) {
            Ok(p) => p,
            Err(e) => {
                println!("      FAIL: version probe: {e}");
                std::process::exit(1);
            }
        };
        match phase {
            idong::Phase::Runtime => println!("      PASS: version \"2619\" (runtime)"),
            other => {
                println!("      FAIL: unexpected phase {other:?} on the runtime PID");
                std::process::exit(1);
            }
        }
        let info = match idong::Session::connect_readonly(&mut t) {
            Ok(i) => i,
            Err(e) => {
                println!("      FAIL: EEPROM reads: {e}");
                println!("      (send this output + a run_capture.bat capture of this run)");
                std::process::exit(1);
            }
        };
        println!("      device ID   : {}", info.device_id);
        println!("      edition tag : {} (code A)", info.code_a);
        println!("      version code: {} (code B)", info.code_b);
        println!("      calibration : {} bytes", info.calibration.len());
        // format checks (values differ per unit; shapes do not)
        let id_ok =
            info.device_id.len() == 32 && info.device_id.bytes().all(|b| b.is_ascii_hexdigit());
        let a_ok = info.code_a.len() == 4;
        let b_ok = info.code_b.len() == 4;
        let c_ok = info.calibration.len() == 120;
        println!(
            "      {} device ID is 32 hex chars / {} codes are 4 chars / {} calibration is 120 B",
            verdict(id_ok),
            verdict(a_ok && b_ok),
            verdict(c_ok)
        );
        // the calibration payload is the REC_CALIB blob of the 24c16
        // dump; the runtime layer already verified the wire checksum,
        // so parse the 120-B PAYLOAD (turn-39 fix: this used to feed the
        // 121-B wire-record parser and always failed with
        // "need 121 bytes, have 120")
        match idong::calib::parse_calibration_payload(&info.calibration) {
            Ok(cal) => {
                println!(
                    "      {} calibration parses: baseline {:.1} mm, focal {:.1} px",
                    verdict(true),
                    cal.baseline_mm(),
                    cal.mean_focal_px()
                );
            }
            Err(e) => {
                println!("      {}: calibration parse: {e}", verdict(false));
                println!("      (send this output + the sensor_states.txt of this run)");
            }
        }
        println!();
        println!("PASS: read-only bring-up complete.");
        println!("next steps:");
        println!("  - the 8051 firmware upload is the P2.3 controlled replay:");
        println!("    cargo run --features usb --example flash_mcu -- \\");
        println!("      --hex <app_m8051_2611.hex>          (dry run, pins only)");
        println!("    ... --confirm                        (the flash itself)");
        println!("  - the video backend is the next code phase (P2.5): V4L2 or");
        println!("    raw usbfs ISO on EP 0x85 - VideoSource is the seam");
        return;
    }

    if let Some(d) = boot {
        println!(
            "[2/3] no runtime device; bootloader 0471:2600 (bus {}, addr {})",
            d.bus, d.address
        );
        println!("[3/3] read-only bootloader probe");
        let mut t = match idong::UsbTransport::open_bootloader() {
            Ok(t) => t,
            Err(e) => {
                println!("      FAIL: cannot open: {e}");
                std::process::exit(1);
            }
        };
        match idong::Session::probe(&mut t) {
            Ok(idong::Phase::Bootloader) => {
                println!("      PASS: bootloader answered, version \"2600\"");
                println!();
                println!("PARTIAL: the sensor is in its 8051 bootloader. For the full");
                println!("read-only bring-up you need the runtime device 0471:2500:");
                println!("  1. run the vendor software once (it flashes the MCU), or");
                println!("  2. wait: the sensor self-reboots to the bootloader every");
                println!("     31-157 s, and the vendor engine re-flashes it");
                println!("then run this example again.");
                return;
            }
            Ok(other) => {
                println!("      FAIL: unexpected phase {other:?}");
                std::process::exit(1);
            }
            Err(e) => {
                println!("      FAIL: probe: {e}");
                std::process::exit(1);
            }
        }
    }

    println!("FAIL: only older-generation devices (2100/0668/0666) found.");
    std::process::exit(1);
}

#[cfg(feature = "usb")]
fn verdict(ok: bool) -> &'static str {
    if ok {
        "PASS"
    } else {
        "FAIL"
    }
}

#[cfg(not(feature = "usb"))]
fn main() {
    eprintln!("build with the usb feature for live USB access:");
    eprintln!("  cargo run --features usb --example bringup");
}
