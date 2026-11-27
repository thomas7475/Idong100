//! P2.3 8051 firmware upload - the CONTROLLED hardware flash replay
//! (turn 39). THE write test of the flash sequence: the exact bytes the
//! vendor engine sends to every bootloader landing, finally run by this
//! driver on the bench.
//!
//! ```sh
//! # dry run (read-only: firmware pin + plan, no device touched) - the
//! # hex shipped with the crate is the default:
//! cargo run --features usb --example flash_mcu
//! # the controlled flash replay:
//! cargo run --features usb --example flash_mcu -- --confirm
//! # another copy of the image (a bare file name is looked up in
//! # software/firmware/v2.6.0.6):
//! cargo run --features usb --example flash_mcu -- --hex <path-or-name> --confirm
//! ```
//!
//! Flags:
//! * `--hex <path>`  the vendor `app_m8051_2611.hex`. Default: the file
//!                   shipped with the crate at
//!                   `software/firmware/v2.6.0.6/app_m8051_2611.hex`
//!                   (`idong::firmware` locates the folder)
//! * `--confirm`     the human key of the two-key gate - without it this
//!                   example is a READ-ONLY dry run (firmware pins only)
//! * `--wait <secs>` how long to wait for the bootloader window when the
//!                   sensor is currently in its runtime state (default
//!                   180; the sensor self-reboots to 0471:2600 every
//!                   31-157 s; unplugging/replugging forces it at once)
//!
//! Why flashing is SAFE here (and why the vendor does it constantly):
//! the sequence writes the app image to the 8051 RAM at 0x8000, reads
//! the WHOLE image back and only then sends the commit key + launch -
//! a corrupted download aborts BEFORE the commit, and the bootloader
//! mask ROM itself is never overwritten (the vendor engine re-runs this
//! exact sequence on EVERY bootloader landing, several times per hour).
//! The T24C16A EEPROM is never touched.
//!
//! The two-key gate (nothing here writes by accident):
//! 1. firmware pin - the raw hex must hash to the turn-12 manifest pin
//!    AND the flattened 21,411-B image must hash to the turn-32 capture
//!    pin (`FirmwareSet::mcu_image`);
//! 2. consent token - `FlashPermit` exists ONLY via the exact crate
//!    phrase `FLASH_CONSENT_PHRASE`, and this example builds it only
//!    after the human passed `--confirm`. `FLASH_CONFIRMED` is true
//!    since the turn-41 bench run (T8 PASS x2) - this example remains
//!    the RECOMMENDED path (the consent gate keeps a human in the loop
//!    even though the compile-time gate is open).

#[cfg(feature = "usb")]
fn main() {
    println!("=== idong P2.3 8051 firmware upload (controlled replay) ===");
    println!();

    let args = Args::parse();

    println!("[1/5] firmware verification (no device touched)");
    let image = match load_pinned_image(args.hex.as_deref()) {
        Ok(img) => img,
        Err(e) => {
            println!("      FAIL: {e}");
            println!("      the hex must be the vendor v2.6.0.6 file");
            println!("      (software/firmware/v2.6.0.6/app_m8051_2611.hex;");
            println!("      hashes in software/firmware/README.md)");
            std::process::exit(1);
        }
    };
    println!("      PASS: sha256 pins hold (raw hex + flattened image)");
    println!();

    if !args.confirm {
        println!("      dry run (no --confirm): nothing was sent, no device was");
        println!("      opened. With --confirm this example runs the EXACT cap3");
        println!("      vendor flash on the 0471:2600 bootloader:");
        println!("        0x0101 begin (size 0x53A3 = 21,411 B)");
        println!("        335 raw 64-B blocks (the last one 35 B)");
        println!("        0x0201 verify -> 00 04 + full-image readback compare");
        println!("        0x0301 key (COMMIT_KEY) + 0x0401 launch");
        println!("        -> re-enumerates as 0471:2500, version \"2619\"");
        println!("      a mismatched readback ABORTS before the key+launch (no commit).");
        println!();
        println!("PASS: dry run complete (firmware verified, consent gate closed).");
        println!("Rerun with --confirm to flash the sensor.");
        return;
    }

    println!("[2/5] consent gate (two keys)");
    let Some(permit) = idong::FlashPermit::from_consent(idong::FLASH_CONSENT_PHRASE) else {
        println!("      FAIL: consent token rejected - this is a driver bug,");
        println!("      the example passes the exact crate constant.");
        std::process::exit(1);
    };
    println!("      PASS: --confirm present, consent token accepted");
    println!("      (the exact vendor sequence runs next - key 0x0301 commits it)");
    println!();

    println!("[3/5] acquire the bootloader (0471:2600)");
    let mut t = match acquire_bootloader(args.wait) {
        Ok(t) => t,
        Err(e) => {
            println!("      FAIL: {e}");
            std::process::exit(1);
        }
    };
    println!("      PASS: bootloader open on interface 0, version \"2600\"");
    println!();

    println!("[4/5] flash sequence (writes - the exact cap3 bytes)");
    println!("      downloading 21,411 B (335 blocks) + readback compare...");
    match idong::protocol::runtime::RuntimeSession::new(&mut t).flash_mcu_permitted(&image, &permit)
    {
        Ok(_) => {
            println!("      PASS: begin/verify acked, readback matched 21,411 B,");
            println!("      key + launch accepted");
        }
        Err(e) => {
            println!("      FAIL: {e}");
            println!("      the readback compare aborts BEFORE the commit key - a");
            println!("      partial download is never launched. RERUN this example;");
            println!("      if it fails twice, send the output + `lsusb -v -d 0471:2600`.");
            std::process::exit(1);
        }
    }
    drop(t); // release the interface: the device re-enumerates now
    println!();

    println!("[5/5] re-enumeration + runtime proof");
    let dev = match idong::transport::reenum::wait_runtime(
        std::time::Duration::from_secs(30),
        std::time::Duration::from_millis(500),
    ) {
        Ok(d) => d,
        Err(e) => {
            println!("      FAIL: {e}");
            println!("      the launch ack was received - if the sensor landed on the");
            println!("      bootloader again it self-rebooted mid-transition: just rerun.");
            std::process::exit(1);
        }
    };
    println!("      PASS: re-enumerated as 0471:2500 (bus {}, addr {})", dev.bus, dev.address);

    let mut t = match idong::UsbTransport::open_runtime() {
        Ok(t) => t,
        Err(e) => {
            println!("      FAIL: cannot open MI_04: {e}");
            println!("      - udev rule installed? (src/transport/udev.rs)");
            std::process::exit(1);
        }
    };
    match idong::Session::probe(&mut t) {
        Ok(idong::Phase::Runtime) => {
            println!("      PASS: version \"2619\" (runtime, the image runs)")
        }
        Ok(other) => {
            println!("      FAIL: unexpected phase {other:?}");
            std::process::exit(1);
        }
        Err(e) => {
            println!("      FAIL: version probe: {e}");
            println!("      (the sensor self-reboots every 31-157 s - rerun if it");
            println!("      was talking seconds ago)");
            std::process::exit(1);
        }
    }
    match idong::Session::connect_readonly(&mut t) {
        Ok(info) => {
            println!("      PASS: identity reads (EEPROM intact)");
            println!("      device ID   : {}", info.device_id);
            println!("      edition tag : {} (code A)", info.code_a);
            println!("      version code: {} (code B)", info.code_b);
            match idong::calib::parse_calibration_payload(&info.calibration) {
                Ok(cal) => println!(
                    "      PASS: calibration parses: baseline {:.1} mm, focal {:.1} px",
                    cal.baseline_mm(),
                    cal.mean_focal_px()
                ),
                Err(e) => {
                    println!("      FAIL: calibration parse: {e}");
                    println!("      (send this output - the flash itself already passed)");
                }
            }
        }
        Err(e) => {
            println!("      FAIL: identity reads: {e}");
            println!("      (the flash itself already passed - this is likely the");
            println!("      31-157 s self-reboot; rerun `flash_mcu --confirm` or the");
            println!("      read-only `bringup` example)");
        }
    }

    println!();
    println!("PASS: the 8051 firmware upload (P2.3 controlled replay) is complete.");
    println!("(P2.3 acceptance reached on the turn-41 bench: FLASH_CONFIRMED = true");
    println!(" - the driver can bring the sensor up from ANY bootloader landing,)");
    println!(" through the ungated flash_mcu path or this consent-gated example.");
}

#[cfg(feature = "usb")]
struct Args {
    hex: Option<String>,
    confirm: bool,
    wait: u64,
}

#[cfg(feature = "usb")]
impl Args {
    fn parse() -> Self {
        let mut args = Args { hex: None, confirm: false, wait: 180 };
        let mut it = std::env::args().skip(1);
        while let Some(a) = it.next() {
            match a.as_str() {
                "--hex" => match it.next() {
                    Some(p) => args.hex = Some(p),
                    None => missing_value("--hex"),
                },
                "--wait" => match it.next() {
                    Some(v) => match v.parse::<u64>() {
                        Ok(n) if n > 0 => args.wait = n,
                        _ => {
                            println!("FAIL: --wait needs a positive number of seconds");
                            std::process::exit(2);
                        }
                    },
                    None => missing_value("--wait"),
                },
                "--confirm" => args.confirm = true,
                "--help" | "-h" => {
                    println!("usage: flash_mcu [--hex <file>] [--confirm] [--wait <secs>]");
                    println!("  --hex <file>    the 8051 image; default: the file shipped");
                    println!("                  with the crate,");
                    println!("                  firmware/v2.6.0.6/app_m8051_2611.hex");
                    println!("                  (a bare file name is looked up in that folder)");
                    println!("  --confirm       run the flash; without it: read-only dry run");
                    println!("  --wait <secs>   bootloader wait, default 180");
                    std::process::exit(0);
                }
                other => {
                    println!("FAIL: unknown argument {other:?}");
                    println!("usage: flash_mcu [--hex <file>] [--confirm] [--wait <secs>]");
                    std::process::exit(2);
                }
            }
        }
        args
    }
}

#[cfg(feature = "usb")]
fn missing_value(flag: &str) -> ! {
    println!("FAIL: {flag} needs a value");
    std::process::exit(2);
}

#[cfg(feature = "usb")]
/// Both SHA-256 pins (raw hex + flattened image) before the first byte
/// leaves the host - safety rule #3. `hex_arg` is the `--hex` value;
/// `None` is the image shipped with the crate.
fn load_pinned_image(hex_arg: Option<&str>) -> idong::Result<Vec<u8>> {
    let (hex_path, hex) = idong::firmware::read_hex(hex_arg)?;
    println!("      hex file    : {} ({} B)", hex_path.display(), hex.len());
    if let Err(e) = idong::FirmwareSet::verify_hex(&hex) {
        println!("      the file above is not the pinned image");
        return Err(e);
    }
    println!("      PASS: raw hex matches the turn-12 manifest pin");
    let fs = idong::FirmwareSet { ldr: Vec::new(), bit: Vec::new(), hex };
    let image = fs.mcu_image()?; // flattens + verifies the turn-32 capture pin
    println!("      PASS: flattened image {} B (0x{:X}), pin matches", image.len(), image.len());
    Ok(image)
}

#[cfg(feature = "usb")]
/// Wait for (or take) the 0471:2600 bootloader and open it. Handles the
/// two bench shapes: the sensor sits in the bootloader already (open at
/// once) or it runs the 0471:2500 runtime (wait out the 31-157 s
/// self-reboot window - or replug). The self-reboot transition can eat
/// the first open (the device vanishes mid-enumeration), so this retries
/// the whole wait+open+probe a bounded 3 times.
fn acquire_bootloader(wait_secs: u64) -> idong::Result<idong::UsbTransport> {
    use std::time::Duration;

    use idong::transport::reenum::wait_until;

    let timeout = Duration::from_secs(wait_secs);
    for attempt in 1..=3u32 {
        let current = idong::list_devices();
        let pids: Vec<u16> = current.iter().map(|d| d.pid).collect();
        match idong::bus_state(&pids) {
            idong::BusState::Bootloader => { /* open below */ }
            idong::BusState::Runtime => {
                println!("      runtime 0471:2500 present - waiting up to {wait_secs} s");
                println!("      for the bootloader window (self-reboot every 31-157 s);");
                println!("      unplug/replug the sensor to force it at once");
            }
            _ => println!("      no sensor on the bus yet - waiting up to {wait_secs} s"),
        }
        // wait_until returns the device; the internal waiter bounds the
        // wait and never spins forever (the kit-36 hang class).
        let dev =
            wait_until(timeout, Duration::from_secs(1), |st| st == idong::BusState::Bootloader)?;
        println!("      bootloader on the bus (bus {}, addr {})", dev.bus, dev.address);
        let mut t = idong::UsbTransport::open_bootloader()?;
        return match idong::Session::probe(&mut t) {
            Ok(idong::Phase::Bootloader) => Ok(t),
            Ok(other) => Err(idong::IdongError::Usb(format!(
                "unexpected phase {other:?} on the bootloader PID"
            ))),
            // the device vanished mid-open (self-reboot transition):
            Err(_) if attempt < 3 => {
                println!("      attempt {attempt}: the bootloader vanished mid-open;");
                println!("      retrying (this is the re-enumeration window)");
                continue;
            }
            Err(e) => Err(e),
        };
    }
    Err(idong::IdongError::Usb(
        "the bootloader never answered across 3 attempts (replug and rerun)".into(),
    ))
}

#[cfg(not(feature = "usb"))]
fn main() {
    // The firmware pins are pure core - honor the dry run even without
    // the usb feature (default hex: the file shipped with the crate),
    // then point at the bench build.
    let args: Vec<String> = std::env::args().collect();
    let hex_arg = args.iter().position(|a| a == "--hex").and_then(|i| args.get(i + 1));
    println!("=== idong P2.3 8051 firmware upload (dry run, pins only) ===");
    let (hex_path, hex) = match idong::firmware::read_hex(hex_arg.map(String::as_str)) {
        Ok(read) => read,
        Err(e) => {
            eprintln!("FAIL: {e}");
            eprintln!("usage: flash_mcu [--hex <file>] [--confirm]  (default hex:");
            eprintln!("       firmware/v2.6.0.6/app_m8051_2611.hex, shipped with the crate)");
            std::process::exit(1);
        }
    };
    println!("      hex file    : {} ({} B)", hex_path.display(), hex.len());
    if let Err(e) = idong::FirmwareSet::verify_hex(&hex) {
        eprintln!("FAIL: the file above is not the pinned image: {e}");
        std::process::exit(1);
    }
    let fs = idong::FirmwareSet { ldr: Vec::new(), bit: Vec::new(), hex };
    match fs.mcu_image() {
        Ok(image) => {
            println!("PASS: firmware pins hold (raw hex + {} B flattened image)", image.len());
            println!("build with the usb feature for the flash itself:");
            println!("  cargo run --features usb --example flash_mcu -- --confirm");
        }
        Err(e) => {
            eprintln!("FAIL: {e}");
            std::process::exit(1);
        }
    }
}
