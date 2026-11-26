//! Read-only probe of the bootloader (0471:2600) - the Rust port of
//! idong_probe.py. NEVER writes (safety rule #1).
//!
//! `cargo run --features usb --example probe_bootloader`
//!
//! Opens the bootloader interface 0, sends the one `0x0008` version
//! command the vendor also sends, expects the version string "2600",
//! closes. Everything the vendor engine sends to a bootloader besides
//! this is the flash sequence - gated behind `FLASH_CONFIRMED`.

#[cfg(feature = "usb")]
fn main() {
    println!("opening 0471:2600 (bootloader, interface 0)...");
    let mut t = match idong::UsbTransport::open_bootloader() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("open failed: {e}");
            eprintln!();
            eprintln!(
                "  - is the sensor plugged in? (run: cargo run --features usb --example list)"
            );
            eprintln!("  - permissions? install the udev rule (src/transport/udev.rs)");
            std::process::exit(1);
        }
    };
    match idong::Session::probe(&mut t) {
        Ok(idong::Phase::Bootloader) => {
            println!("PASS: bootloader answered, version \"2600\"");
            println!();
            println!("the device is in its 8051 bootloader. The vendor engine flashes");
            println!("app_m8051_2611.hex from here; this driver's flash sequence is");
            println!("byte-identical on the wire but gated until one controlled");
            println!("hardware replay confirms it (FLASH_CONFIRMED, P2.3).");
            println!("Flashing is NOT needed for the read-only bring-up: run the vendor");
            println!("software once, then use the runtime device:");
            println!("  cargo run --features usb --example bringup");
        }
        Ok(other) => {
            eprintln!("FAIL: unexpected phase {other:?} on the bootloader PID");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("FAIL: probe error: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(feature = "usb"))]
fn main() {
    eprintln!("build with the usb feature for live USB access:");
    eprintln!("  cargo run --features usb --example probe_bootloader");
}
