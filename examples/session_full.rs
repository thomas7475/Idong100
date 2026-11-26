//! P2.4 full-session bring-up - the hardware test that completes the
//! session layer (turn 38).
//!
//! `cargo run --features usb --example session_full`
//! `cargo run --features usb --example session_full -- --cmos`
//!
//! What this does on the REAL sensor, in the observed vendor order:
//!   1. enumerate - which sensor state is on the bus? (read-only)
//!   2. read-only identity - the four EEPROM reads of bringup (read-only)
//!   3. WITH `--cmos` only: the CMOS register init - the FIRST writes
//!      this driver ever sends to the sensor (the exact cap3 table
//!      bytes the vendor engine sends on every session: 2 register
//!      tables 56 B + 24 B, each written + read back, then 6 x 8-B
//!      register chunks each written + read back, then the 0x0303 end
//!      marker - 17 writes in total, all BG0313 image-sensor registers,
//!      all volatile: a power cycle clears them, the EEPROM is never
//!      touched)
//!   4. one steady-state poll cycle (the six observed session polls)
//!      and three 0x0205 stream polls (status reads; unanswered is
//!      EXPECTED while no video stream runs)
//!
//! Without `--cmos` this is exactly as safe as `bringup` (reads only)
//! and prints what the full run would send. With `--cmos` the only
//! writes are the vendor's own register tables - no flash, no EEPROM
//! writes, no stream start.

#[cfg(feature = "usb")]
fn main() {
    println!("=== idong P2.4 full-session bring-up ===");
    println!();

    let cmos = std::env::args().skip(1).any(|a| a == "--cmos");

    println!("[1/4] enumerate");
    let devices = idong::list_devices();
    if devices.is_empty() {
        println!("      no sensor found - plug the sensor in (0471:2600/2500)");
        std::process::exit(1);
    }
    for d in &devices {
        println!("      bus {:3} addr {:3} {} {:?}", d.bus, d.address, d.vid_pid(), d.state);
    }
    let Some(runtime) = devices.iter().find(|d| d.pid == idong::protocol::PID_RUNTIME) else {
        println!("      no runtime device 0471:2500 - run the vendor software once");
        println!("      (or examples/bringup) to flash the MCU, then rerun this");
        std::process::exit(1);
    };
    println!("      PASS: runtime device present (bus {}, addr {})", runtime.bus, runtime.address);

    let mut t = match idong::UsbTransport::open_runtime() {
        Ok(t) => t,
        Err(e) => {
            println!("      FAIL: cannot open MI_04: {e}");
            println!("      - udev rule installed? (src/transport/udev.rs)");
            std::process::exit(1);
        }
    };
    println!("      PASS: vendor interface MI_04 claimed");

    println!("[2/4] read-only identity (four EEPROM reads)");
    let info = match idong::Session::connect_readonly(&mut t) {
        Ok(i) => i,
        Err(e) => fail_reboot_note(e),
    };
    print_identity(&info);
    println!("      PASS: identity read (same bytes as the bringup test)");

    println!("[3/4] CMOS register init (the first writes - needs --cmos)");
    if !cmos {
        println!("      SKIPPED (no --cmos). With --cmos this sends the exact");
        println!("      cap3 register tables the vendor engine sends:");
        println!("        0x0104 table 56 B + 0x0204 readback   (CMOS_TABLE_56)");
        println!("        0x0104 table 24 B + 0x0204 readback   (CMOS_TABLE_24)");
        println!("        6 x 0x0103 chunk 8 B + 0x0203 readback (CMOS_REG8_CHUNKS)");
        println!("        0x0303 end marker");
        println!("      17 writes total, all volatile image-sensor registers");
        println!("      (a power cycle clears them; EEPROM is never written).");
        println!();
        println!("PASS: read-only part of the full-session bring-up complete.");
        println!("Rerun with --cmos for the register init + polls.");
        return;
    }

    // Session::connect on a runtime phase = the four EEPROM reads again
    // (vendor order) + the CMOS init tables. image/write_risk only
    // matter on the bootloader (flash) path; this device is runtime.
    let mut session = match idong::Session::connect(&mut t, idong::Phase::Runtime, &[], false) {
        Ok(s) => s,
        Err(e) => fail_reboot_note(e),
    };
    println!("      PASS: register init complete (tables + chunks + end marker)");

    println!("[4/4] session traffic");
    match session.poll_cycle() {
        Ok(()) => {
            println!("      PASS: poll cycle answered (6/6: 0003 0004 0005 000D 0404 040F)");
        }
        Err(e) => fail_reboot_note(e),
    }
    for i in 1..=3u32 {
        match session.poll_stream() {
            Ok(st) => {
                println!(
                    "      0x0205 #{}: new_frame={} frame_no={} (unanswered is expected while no video runs)",
                    i, st.new_frame, st.frame_no
                );
            }
            Err(_) => {
                println!("      0x0205 #{}: unanswered (~3 s) - expected without a stream", i);
            }
        }
    }

    println!();
    println!("PASS: full-session bring-up complete.");
    println!("next: cargo run --features usb --example video -- --probe");
    println!("      cargo run --features usb --example video -- --cmos --frames 10");
}

#[cfg(feature = "usb")]
fn print_identity(info: &idong::SessionInfo) {
    println!("      device ID   : {}", info.device_id);
    println!("      edition tag : {} (code A)", info.code_a);
    println!("      version code: {} (code B)", info.code_b);
    println!("      calibration : {} bytes", info.calibration.len());
}

#[cfg(feature = "usb")]
fn fail_reboot_note(e: idong::IdongError) -> ! {
    println!("      FAIL: {e}");
    println!("      note: the sensor self-reboots every 31-157 s - if this ran");
    println!("      longer than half a minute, the device simply went away;");
    println!("      RERUN the example (this is normal, not a driver bug).");
    println!("      (automatic re-enumeration recovery is P2.4b, transport/reenum.)");
    std::process::exit(1);
}

#[cfg(not(feature = "usb"))]
fn main() {
    eprintln!("build with the usb feature for live USB access:");
    eprintln!("  cargo run --features usb --example session_full [-- --cmos]");
}
