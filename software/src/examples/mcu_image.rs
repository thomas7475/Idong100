//! Build the MCU image from the shipped app_m8051_2611.hex and print its
//! SHA-256 (development check, not part of the shipped flow).
//!
//! Usage: cargo run --example mcu_image [path-to-hex]
//! No argument: `firmware/v2.6.0.6/app_m8051_2611.hex` of the crate; a
//! bare file name is looked up in that folder.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let hex_arg = args.get(1).map(String::as_str);
    let (path, hex) = match idong::firmware::read_hex(hex_arg) {
        Ok(read) => read,
        Err(e) => {
            eprintln!("FAILED: {e}");
            eprintln!("usage: mcu_image [path-to-app_m8051_2611.hex]");
            std::process::exit(2);
        }
    };
    let fs = idong::FirmwareSet { ldr: Vec::new(), bit: Vec::new(), hex };
    match fs.mcu_image() {
        Ok(image) => {
            println!("hex file   : {}", path.display());
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
