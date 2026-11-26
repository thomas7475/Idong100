//! Enumerate i-Dong devices, report lifecycle state. READ-ONLY.
//!
//! `cargo run --features usb --example list`
//!
//! Discovers every 0471 device (bootloader 2600, runtime 2500, legacy
//! 2100/0668/0666) with descriptors only - no interface is claimed,
//! nothing is written.

#[cfg(feature = "usb")]
fn main() {
    let devices = idong::list_devices();
    if devices.is_empty() {
        println!("no i-Dong sensor on the bus");
        println!("  looked for 0471:2600 (bootloader), 0471:2500 (runtime),");
        println!("  0471:2100 / 0668 / 0666 (older generations)");
        println!("  - plug the sensor in; on a VM also attach it to the VM");
        return;
    }
    for d in &devices {
        println!(
            "bus {:3}  addr {:3}  {}  {:?}{}",
            d.bus,
            d.address,
            d.vid_pid(),
            d.state,
            d.serial.as_deref().map(|s| format!("  serial {s}")).unwrap_or_default()
        );
    }
    match idong::bus_state(&devices.iter().map(|d| d.pid).collect::<Vec<_>>()) {
        idong::BusState::Runtime => {
            println!();
            println!("runtime device present - the next step is the read-only bring-up:");
            println!("  cargo run --features usb --example bringup");
        }
        idong::BusState::Bootloader => {
            println!();
            println!("bootloader present - run examples/bringup.rs for the read-only probe");
        }
        _ => {}
    }
}

#[cfg(not(feature = "usb"))]
fn main() {
    eprintln!("build with the usb feature for live USB access:");
    eprintln!("  cargo run --features usb --example list");
}
