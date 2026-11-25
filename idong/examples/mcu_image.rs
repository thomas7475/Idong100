//! Build the MCU image from a real app_m8051_2611.hex and print its
//! SHA-256 (development check, not part of the shipped flow).
//!
//! Usage: cargo run -p idong --example mcu_image -- <path-to-hex>

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("usage: mcu_image <app_m8051_2611.hex>");
    let hex = std::fs::read(path).expect("read hex file");
    let fs = idong::FirmwareSet { ldr: Vec::new(), bit: Vec::new(), hex };
    match fs.mcu_image() {
        Ok(image) => {
            println!("image: {} bytes (0x{:X}), sha256 verified", image.len(), image.len());
            println!(
                "first 16 B: {}",
                image[..16].iter().map(|b| format!("{b:02x}")).collect::<String>()
            );
        }
        Err(e) => {
            eprintln!("FAILED: {e}");
            std::process::exit(1);
        }
    }
}
