//! P3a marker/record plane bench - the T12 one-command decision (turn 46).
//!
//! `cargo run --features usb --example records`
//! `cargo run --features usb --example records -- --cmos`
//! `cargo run --features usb --example records -- --cmos --frames 10`
//! `cargo run --features usb --example records -- --cmos --frames 10 --out recdir`
//!
//! What this does on the REAL sensor:
//!   1. enumerate - runtime device 0471:2500 must be present
//!   2. read-only identity (the four EEPROM reads, like bringup)
//!   3. WITH `--cmos` only: the full session init (register tables +
//!      poll cycle, exactly session_full --cmos)
//!   4. the ENS PROBE - the turn-46 decision point, read-only on the
//!      record plane: read 64 B from EP 0x83 ten times (100 ms each)
//!      and check for an OD frame of type 0xF0 - the vendor usbCom
//!      worker's startup probe (xEyeUSB.dll @RVA 0x10640)
//!   5. if a type-0xF0 frame answers: the vendor handshake loop -
//!      read one 640-B record, ACK with a 64-B type-7 command on
//!      EP 0x03 (zero payload), dump + report per record
//!
//! VERDICT A (stream alive): records arrive -> the raw blob data lands
//! in --out; the marker plane is ON. VERDICT B (no frame in 10 tries):
//! the device needs the runtime `ens_code.ldr` upload first (the
//! vendor state machine @RVA 0xf790 - the never-captured PU/OD traffic
//! on EP 0x03/0x83). One capture with the kit-37 unlocked app in a
//! running game pins its wire bytes; this probe is the decision.
//!
//! This example never writes flash or EEPROM. The only writes (with
//! --cmos) are the vendor's own volatile CMOS register tables and the
//! 64-B record ACKs.

#[cfg(feature = "usb")]
fn main() {
    use std::time::Duration;

    use idong::protocol::{
        ENS_PROBE_TIMEOUT, ENS_PROBE_TRIES, EP_RECORD_IN, EP_RECORD_OUT, RECORD_FRAME_LEN,
        RECORD_WIRE_LEN,
    };

    println!("=== idong P3a record-plane bench (T12, turn 46) ===");
    println!();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmos = args.iter().any(|a| a == "--cmos");
    let frames = flag_value(&args, "--frames").parse::<usize>().unwrap_or(10);
    let outdir = {
        let v = flag_value(&args, "--out");
        if v.is_empty() {
            "records-out".to_string()
        } else {
            v
        }
    };
    let probe_only = args.iter().any(|a| a == "--probe-only");

    println!("[1/5] enumerate");
    let devices = idong::list_devices();
    if devices.is_empty() {
        println!("      no sensor found - plug the sensor in (0471:2600/2500)");
        std::process::exit(1);
    }
    for d in &devices {
        println!("      bus {:3} addr {:3} {} {:?}", d.bus, d.address, d.vid_pid(), d.state);
    }
    if !devices.iter().any(|d| d.pid == idong::protocol::PID_RUNTIME) {
        println!("      no runtime device 0471:2500 - flash it first (flash_mcu --confirm)");
        std::process::exit(1);
    }
    println!("      PASS: runtime device present");

    let mut t = match idong::UsbTransport::open_runtime() {
        Ok(t) => t,
        Err(e) => {
            println!("      FAIL: cannot open MI_04: {e}");
            println!("      - a 'Resource busy' right after a renum clears on rerun");
            std::process::exit(1);
        }
    };
    println!("      PASS: vendor interface MI_04 claimed (bulk pairs 0x02/0x82 + 0x03/0x83)");

    println!("[2/5] read-only identity");
    match idong::Session::connect_readonly(&mut t) {
        Ok(info) => {
            match idong::calib::parse_calibration_payload(&info.calibration) {
                Ok(cal) => println!(
                    "      device {} calibration |T| {:.1} mm fx {:.1} px",
                    info.device_id,
                    cal.baseline_mm(),
                    cal.mean_focal_px()
                ),
                Err(e) => println!("      device {} (calibration parse: {e})", info.device_id),
            }
            println!("      PASS: 7F 55 session layer answers (EEPROM reads)");
        }
        Err(e) => {
            println!("      FAIL: {e}");
            println!("      (the sensor self-reboots every 31-157 s - rerun on a fresh landing)");
            std::process::exit(1);
        }
    }

    if !cmos {
        println!("[3/5] session init SKIPPED (no --cmos)");
        println!("      The vendor engine completes the register init before the");
        println!("      record plane can run. Rerun with --cmos to send the exact");
        println!("      vendor tables first (volatile registers, EEPROM untouched).");
    } else {
        println!("[3/5] full session init (--cmos: register tables + polls)");
        match idong::Session::connect(&mut t, idong::Phase::Runtime, &[], false) {
            Ok(mut session) => {
                if let Err(e) = session.poll_cycle() {
                    println!("      WARN: poll cycle: {e} (non-fatal for the probe)");
                } else {
                    println!("      PASS: register init + poll cycle (6/6)");
                }
            }
            Err(e) => {
                println!("      WARN: {e} (continuing with the probe - read-only)");
            }
        }
    }

    println!("[4/5] ENS probe: EP 0x83, 64 B x {ENS_PROBE_TRIES} @ 100 ms, expecting OD type 0xF0");
    let mut probe_saw = None;
    for try_no in 1..=ENS_PROBE_TRIES {
        match t.read_bulk_ep(EP_RECORD_IN, RECORD_FRAME_LEN, ENS_PROBE_TIMEOUT) {
            Ok(buf) if buf.len() == RECORD_FRAME_LEN => {
                match idong::protocol::ens::StatusFrame::parse(&buf) {
                    Some(f) => {
                        println!(
                            "      frame after {try_no} tries: OD type 0x{:04X} (payload head: {})",
                            f.frame_type(),
                            hex_head(&f.bytes[6..16])
                        );
                        probe_saw = Some(f);
                        break;
                    }
                    None => {
                        println!(
                            "      try {try_no}: 64 B arrived but not a type-0xF0 OD frame: {}",
                            hex_head(&buf[..16])
                        );
                    }
                }
            }
            Ok(buf) if buf.is_empty() => { /* timeout - quiet, the probe budget runs */ }
            Ok(buf) => println!("      try {try_no}: short read {} B (expected 64)", buf.len()),
            Err(e) => println!("      try {try_no}: read error: {e}"),
        }
    }

    let Some(_first) = probe_saw else {
        println!();
        println!("      VERDICT B: no type-0xF0 record stream on EP 0x83.");
        println!("      The vendor stack would now run the ens_code.ldr upload state");
        println!("      machine (xEyeUSB.dll @RVA 0xf790) on EP 0x03/0x83 - the PU/OD");
        println!("      traffic never seen in any capture (the license gates kept the");
        println!("      app out of the game state). ONE capture with the kit-37");
        println!("      unlocked app in a running game pins its wire bytes, and the");
        println!("      driver replays them the same way flash_mcu replays the hex.");
        println!();
        println!("FAIL: probe exhausted (expected outcome until the ldr machine is pinned)");
        std::process::exit(2);
    };
    println!("      PASS: the record plane is ALIVE (type-0xF0 frames flow)");

    if probe_only {
        println!();
        println!("PASS: probe-only run (--probe-only). Rerun with --frames N for the");
        println!("handshake loop (status -> 640-B record -> type-7 ACK).");
        return;
    }

    println!("[5/5] handshake loop: {frames} records (status -> 640-B record -> type-7 ACK)");
    let _ = std::fs::create_dir_all(&outdir);
    let mut ack = idong::protocol::ens::Type7Command::ack_zeros();
    let mut ok = 0usize;
    for i in 1..=frames {
        // 1. the 64-B type-0xF0 announcement
        let status = match t.read_bulk_ep(EP_RECORD_IN, RECORD_FRAME_LEN, Duration::from_secs(2)) {
            Ok(buf) if buf.len() == RECORD_FRAME_LEN => buf,
            Ok(buf) if buf.is_empty() => {
                println!("      record {i}: no announcement within 2 s - stream stopped");
                break;
            }
            Ok(buf) => {
                println!("      record {i}: short announcement ({} B) - stop", buf.len());
                break;
            }
            Err(e) => {
                println!("      record {i}: announcement read error: {e} - stop");
                break;
            }
        };
        let status = match idong::protocol::ens::StatusFrame::parse(&status) {
            Some(f) => f,
            None => {
                println!("      record {i}: announcement is not OD type 0xF0 - stop");
                break;
            }
        };
        // 2. the 640-B raw record
        let wire = match t.read_bulk_ep(EP_RECORD_IN, RECORD_WIRE_LEN, Duration::from_secs(2)) {
            Ok(buf) if buf.len() == RECORD_WIRE_LEN => buf,
            Ok(buf) => {
                println!("      record {i}: record read {} B (expected 640) - stop", buf.len());
                break;
            }
            Err(e) => {
                println!("      record {i}: record read error: {e} - stop");
                break;
            }
        };
        let record = match idong::protocol::ens::WireRecord::parse(&wire) {
            Some(r) => r,
            None => {
                println!("      record {i}: OD trailer magic missing - stop");
                break;
            }
        };
        // 3. the 64-B type-7 ACK on EP 0x03 (zero payload: we have no
        //    classifier - the vendor fills this from the PREVIOUS
        //    record's classification; zeros keep the loop benign)
        if let Err(e) = t.write_bulk_ep(EP_RECORD_OUT, &ack.bytes, Duration::from_secs(1)) {
            println!("      record {i}: ACK write failed: {e} - stop");
            break;
        }
        // 4. report + dump
        let s = record.status();
        println!(
            "      record {i:3}: announce {} | blob {nn:4} nonzero B | status 0x{raw:02X} (mode {m}, flags {fa}{fb}{fc})",
            hex_head(&status.bytes[6..16]),
            raw = s.raw,
            m = s.mode,
            nn = record.blob_nonzero(),
            fa = if s.flag_a { 'A' } else { '-' },
            fb = if s.flag_b { 'B' } else { '-' },
            fc = if s.flag_c { 'C' } else { '-' },
        );
        let _ = std::fs::write(
            std::path::Path::new(&outdir).join(format!("record-{i:03}.bin")),
            &record.bytes,
        );
        // next ACK stays zero-payload (see note above)
        ack = idong::protocol::ens::Type7Command::ack_zeros();
        ok += 1;
    }

    println!();
    if ok == frames {
        println!("PASS: {ok}/{frames} records exchanged, raw dumps in {outdir}/.");
        println!("The marker plane is LIVE. The blob layout (classifier input) is");
        println!("the next decode: pair these dumps with a running game capture.");
    } else if ok > 0 {
        println!("PARTIAL: {ok}/{frames} records, then the stream stopped - the");
        println!("dumps in {outdir}/ still decide the blob layout. Check the");
        println!("self-reboot window (31-157 s) if this ran long.");
    } else {
        println!("FAIL: no complete record exchanged (the announcement answered");
        println!("but the 640-B read or the ACK failed) - rerun on a fresh landing.");
    }
}

#[cfg(feature = "usb")]
fn flag_value(args: &[String], flag: &str) -> String {
    args.iter().position(|a| a == flag).and_then(|p| args.get(p + 1)).cloned().unwrap_or_default()
}

#[cfg(feature = "usb")]
fn hex_head(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(" ")
}

#[cfg(not(feature = "usb"))]
fn main() {
    println!("this example needs the usb feature: cargo run --features usb --example records");
}
