//! P3a/P3b marker-record-plane bench - T12/T13/T14 (turns 46/49/50).
//!
//! `cargo run --features usb --example records`
//! `cargo run --features usb --example records -- --cmos`
//! `cargo run --features usb --example records -- --cmos --frames 10`
//! `cargo run --features usb --example records -- --cmos --frames 10 --out recdir`
//! `cargo run --features usb --example records -- --cmos --no-fpga --no-ldr   (T12: probe only)`
//!
//! What this does on the REAL sensor:
//!   1. enumerate - runtime device 0471:2500 must be present
//!   2. read-only identity (the four EEPROM reads, like bringup)
//!   3. WITH `--cmos` only: the full session init (register tables +
//!      poll cycle, exactly session_full --cmos)
//!   4. WITH `--cmos` only (turn 50): the FPGA BITSTREAM download -
//!      A_a_v4.4_BG0313.bit, SHA-pinned, bit-reversed and streamed in
//!      64 KiB chunks over the 7F 55 session plane (the vendor
//!      `xeyezHal_StartFlow` step 3 / `DownLoadFpga` @0x10040; the
//!      XC3S400A config is VOLATILE - the vendor re-downloads it on
//!      every device open). `--no-fpga` skips it.
//!   5. the ENS PROBE (read-only): read 64 B from EP 0x83 ten times
//!      (100 ms each) and check for an OD frame of type 0xF0 - the
//!      vendor usbCom worker's startup probe (xEyeUSB.dll @RVA 0x10640)
//!   6a. probe ALIVE -> the HOT START (0x10640 + 0x10900): three
//!       record cycles, two type-7 ACKs, then the lens (PU/OD 0x10)
//!       and device-ID (PU/OD 0x9001) exchange
//!   6b. probe SILENT -> the COLD START (0xf790, turn 49): the
//!       8-state machine that uploads the shipped, SHA-256-pinned
//!       `ens_code.ldr` to the DSP (PU type 2 {size, CRC32} -> OD 2 ->
//!       raw 64-B chunks -> OD 3), then the pairing select, the ID &
//!       version exchange, the pairing loop and the lens parameters.
//!       `--no-ldr` keeps the T12 behavior (report and stop).
//!   7. the RECORD LOOP: status frame -> 640-B record -> type-7 ACK,
//!      `--frames N` times, raw dumps + per-record stats.
//!
//! The FPGA download and the DSP loader upload are both VOLATILE RAM
//! loads (the vendor's own startup behavior). The 8051 flash and the
//! EEPROM are never touched.

#[cfg(feature = "usb")]
fn main() {
    use idong::protocol::ens::{ColdStart, HotStart};
    use idong::protocol::{ENS_PROBE_TIMEOUT, ENS_PROBE_TRIES, EP_RECORD_IN, RECORD_FRAME_LEN};

    println!("=== idong P3b record-plane bench (T12/T13, turns 46/49) ===");
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
    let no_ldr = args.iter().any(|a| a == "--no-ldr");
    let no_match = args.iter().any(|a| a == "--no-match");
    let no_fpga = args.iter().any(|a| a == "--no-fpga");
    let fpga_only = args.iter().any(|a| a == "--fpga-only");

    println!("[1/7] enumerate");
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

    println!("[2/7] read-only identity");
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
        println!("[3/7] session init SKIPPED (no --cmos)");
        println!("      The vendor engine completes the register init and the");
        println!("      FPGA download before the record plane can run. Rerun with");
        println!("      --cmos to send the vendor tables + the bitstream first");
        println!("      (both volatile - RAM state, EEPROM untouched).");
    } else {
        println!("[3/7] full session init (--cmos: register tables + polls)");
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

    // ---- [4/7]: the FPGA bitstream download (turn 50) ----
    if cmos && !no_fpga {
        println!("[4/7] FPGA bring-up: download A_a_v4.4_BG0313.bit (StartFlow step 3)");
        let set = match idong::firmware::load_set() {
            Ok(s) => s,
            Err(e) => {
                println!("      FAIL: firmware set: {e}");
                std::process::exit(2);
            }
        };
        println!(
            "      bitstream {} B (XC3S400A work_top.ncd, volatile config) - bit-reversed, {} chunks of 64 KiB",
            set.bit.len(),
            set.bit.len().div_ceil(idong::protocol::fpga::FPGA_CHUNK)
        );
        let mut dl = idong::protocol::fpga::FpgaDownload::new(&mut t);
        match dl.run(&set.bit) {
            Ok(rep) => {
                for m in &dl.milestones {
                    println!("      milestone: {m}");
                }
                println!("      QUERY_CFG reply: {}", hex_head(&rep.query_reply));
                println!(
                    "      PASS: the FPGA is configured ({} B in {} chunks)",
                    rep.size, rep.chunks
                );
            }
            Err(e) => {
                println!("      FAIL: {e}");
                println!("      (the FPGA config is volatile and the vendor re-downloads");
                println!("      it on every open - power-cycle the sensor and rerun)");
                std::process::exit(2);
            }
        }
        if fpga_only {
            println!();
            println!("PASS: fpga-only run (--fpga-only). The DSP loader (ens_code.ldr)");
            println!("was NOT uploaded - rerun without the flag for the full plane.");
            return;
        }
    } else {
        println!("[4/7] FPGA bring-up SKIPPED (--no-fpga or no --cmos)");
    }

    println!("[5/7] ENS probe: EP 0x83, 64 B x {ENS_PROBE_TRIES} @ 100 ms, expecting OD type 0xF0");
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
        // ---- VERDICT B: silence -> the cold start (turn 49) ----
        println!();
        println!("      VERDICT B: no type-0xF0 record stream on EP 0x83 - the DSP");
        println!("      needs the runtime ens_code.ldr upload first (the vendor state");
        println!("      machine @0xf790, turn 49: decoded from xEyeUSB.dll).");
        if no_ldr || probe_only {
            println!("      --no-ldr/--probe-only: upload refused, T12 behavior kept.");
            println!("FAIL: probe exhausted (--no-ldr)");
            std::process::exit(2);
        }
        println!();
        println!("[6/7] cold start: the 8-state ens_code.ldr upload (sha256-pinned)");
        let set = match idong::firmware::load_set() {
            Ok(s) => s,
            Err(e) => {
                println!("      FAIL: firmware set: {e}");
                std::process::exit(2);
            }
        };
        println!(
            "      loader image: {} B (crc32_ens 0x{:08X}, {} chunks of 64)",
            set.ldr.len(),
            idong::protocol::ens::crc32_ens(&set.ldr),
            set.ldr.len().div_ceil(64)
        );
        let mut cs = ColdStart::new(&mut t, &set.ldr);
        match cs.run(!no_match) {
            Ok(out) => {
                for m in &cs.milestones {
                    println!("      milestone: {m}");
                }
                println!(
                    "      DSP version {} device ID {}",
                    String::from_utf8_lossy(&out.dsp_version),
                    String::from_utf8_lossy(&out.device_id)
                );
                println!(
                    "      lens params: {} B (first: {})",
                    out.lens_params.len(),
                    hex_head(&out.lens_params[..8.min(out.lens_params.len())])
                );
                println!("      PASS: the cold start completed (the DSP runs)");
            }
            Err(e) => {
                println!("      FAIL: {e}");
                println!("      (the vendor machine retries each state once and restarts");
                println!("      on double failure - rerun the bench the same way)");
                std::process::exit(2);
            }
        }
        startup_done(&mut t, frames, outdir, true);
        return;
    };

    // ---- VERDICT A: the stream is alive -> the hot start ----
    println!("      PASS: the record plane is ALIVE (type-0xF0 frames flow)");
    if probe_only {
        println!();
        println!("PASS: probe-only run (--probe-only).");
        return;
    }
    println!();
    println!("[6/7] hot start: 3 record cycles + lens/DID exchange (0x10640 + 0x10900)");
    let mut hs = HotStart::new(&mut t);
    match hs.run() {
        Ok(Some(out)) => {
            for m in &hs.milestones {
                println!("      milestone: {m}");
            }
            println!(
                "      DSP version {} device ID {}",
                String::from_utf8_lossy(&out.dsp_version),
                String::from_utf8_lossy(&out.device_id)
            );
            println!("      PASS: the hot start completed");
        }
        Ok(None) => {
            println!("      FAIL: the stream vanished mid-probe - rerun");
            std::process::exit(2);
        }
        Err(e) => {
            println!("      FAIL: {e}");
            std::process::exit(2);
        }
    }
    startup_done(&mut t, frames, outdir, false);
}

#[cfg(feature = "usb")]
fn startup_done(t: &mut idong::UsbTransport, frames: usize, outdir: String, cold: bool) {
    use std::time::Duration;

    use idong::protocol::{EP_RECORD_IN, EP_RECORD_OUT, RECORD_FRAME_LEN, RECORD_WIRE_LEN};

    let how = if cold { "cold start" } else { "hot start" };
    println!();
    println!(
        "[7/7] handshake loop after {how}: {frames} records (status -> 640-B record -> type-7 ACK)"
    );
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
        println!("PASS: {ok}/{frames} records exchanged after the {how}, raw dumps in {outdir}/.");
        println!("The marker plane is LIVE. The blob layout (classifier input) is");
        println!("the next decode: pair these dumps with a running game capture.");
    } else if ok > 0 {
        println!("PARTIAL: {ok}/{frames} records, then the stream stopped - the");
        println!("dumps in {outdir}/ still decide the blob layout. Check the");
        println!("self-reboot window (31-157 s) if this ran long.");
    } else {
        println!("FAIL: no complete record exchanged after the {how} - the DSP may");
        println!("need a full power cycle (the loader upload is volatile).");
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
