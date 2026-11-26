//! The firmware folder and the lookup rules for its files (turn 40).
//!
//! The device-side vendor files ship with the crate under
//! `firmware/<vendor version>/` (provenance, roles and hashes:
//! `firmware/README.md`). [`resolve_hex`] turns the `--hex` argument of the
//! examples into a path: no argument is the shipped `app_m8051_2611.hex`, a
//! bare file name is looked up in the folder, anything else is taken as a
//! path. [`load_set`] reads the three pinned images.
//!
//! Nothing here verifies bytes: the SHA-256 pins live in
//! [`FirmwareSet::verify`] / [`FirmwareSet::verify_hex`] and run after the
//! read, before any byte leaves the host.

use std::path::{Path, PathBuf};

use crate::error::{IdongError, Result};
use crate::FirmwareSet;

/// The vendor package the driver targets (the folder under `firmware/`).
pub const VENDOR_VERSION: &str = "v2.6.0.6";

/// The firmware folder, relative to the crate root (next to `Cargo.toml`).
pub const FIRMWARE_DIR: &str = "firmware";

/// `ens_code.ldr` - Blackfin BF527 loader image.
pub const LDR_FILE: &str = "ens_code.ldr";

/// `A_a_v4.4_BG0313.bit` - Xilinx FPGA bitstream.
pub const BIT_FILE: &str = "A_a_v4.4_BG0313.bit";

/// `app_m8051_2611.hex` - the 8051 image the flash path sends.
pub const HEX_FILE: &str = "app_m8051_2611.hex";

/// `firmware/<vendor version>`, relative to the crate root.
fn version_dir_rel() -> PathBuf {
    Path::new(FIRMWARE_DIR).join(VENDOR_VERSION)
}

/// Every place [`firmware_dir`] looks, in order: the crate's own folder
/// (its path is compiled in), then the folder above the running program
/// (`target/debug/examples/` is three levels below the crate), then the
/// current directory.
pub fn firmware_dir_candidates() -> Vec<PathBuf> {
    let rel = version_dir_rel();
    let mut out = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&rel)];
    if let Some(exe_dir) =
        std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        out.extend(exe_dir.ancestors().skip(1).take(4).map(|anc| anc.join(&rel)));
    }
    out.push(rel);
    out
}

/// The firmware folder, or `None` when no candidate exists (the program
/// was moved out of the source tree: pass explicit file paths then).
pub fn firmware_dir() -> Option<PathBuf> {
    firmware_dir_candidates().into_iter().find(|p| p.is_dir())
}

fn no_folder() -> IdongError {
    let looked: Vec<String> =
        firmware_dir_candidates().iter().map(|p| p.display().to_string()).collect();
    IdongError::FirmwareFile {
        path: version_dir_rel(),
        reason: format!("firmware folder not found (looked at {})", looked.join(", ")),
    }
}

fn is_bare_name(p: &Path) -> bool {
    p.components().count() == 1
}

/// Turn a `--hex`-style argument into a file path.
///
/// * `None` - the shipped [`HEX_FILE`] in the firmware folder
/// * a path that names an existing file - that path
/// * a bare file name the current directory does not have - the same
///   name in the firmware folder
///
/// Anything else is an error that lists the places looked at. The path is
/// not verified: read it, then run [`FirmwareSet::verify_hex`].
pub fn resolve_hex(arg: Option<&str>) -> Result<PathBuf> {
    let Some(a) = arg else {
        return Ok(firmware_dir().ok_or_else(no_folder)?.join(HEX_FILE));
    };
    let given = Path::new(a).to_path_buf();
    if given.is_file() {
        return Ok(given);
    }
    let mut tried = vec![given.clone()];
    if is_bare_name(&given) {
        if let Some(dir) = firmware_dir() {
            let in_folder = dir.join(&given);
            if in_folder.is_file() {
                return Ok(in_folder);
            }
            tried.push(in_folder);
        }
    }
    Err(IdongError::FirmwareFile {
        path: given,
        reason: format!(
            "file not found (looked at {})",
            tried.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
        ),
    })
}

fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| IdongError::FirmwareFile {
        path: path.to_path_buf(),
        reason: format!("cannot read: {e}"),
    })
}

/// [`resolve_hex`] and read the file. The bytes are not verified here:
/// `FirmwareSet::verify_hex` is the next step of every caller.
pub fn read_hex(arg: Option<&str>) -> Result<(PathBuf, Vec<u8>)> {
    let path = resolve_hex(arg)?;
    let bytes = read(&path)?;
    Ok((path, bytes))
}

/// Read the three pinned images from the firmware folder and verify them
/// ([`FirmwareSet::verify`]).
pub fn load_set() -> Result<FirmwareSet> {
    let dir = firmware_dir().ok_or_else(no_folder)?;
    let set = FirmwareSet {
        ldr: read(&dir.join(LDR_FILE))?,
        bit: read(&dir.join(BIT_FILE))?,
        hex: read(&dir.join(HEX_FILE))?,
    };
    set.verify()?;
    Ok(set)
}

/// The crate's SHA-256 (FIPS 180-4, the function behind the firmware
/// pins) as lower-case hex.
pub fn sha256_hex(data: &[u8]) -> String {
    FirmwareSet::sha256hex(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_folder_is_the_first_candidate() {
        let c = firmware_dir_candidates();
        assert_eq!(
            c[0],
            Path::new(env!("CARGO_MANIFEST_DIR")).join("firmware").join(VENDOR_VERSION)
        );
        // the working-directory fallback is always last
        assert_eq!(c.last().unwrap(), &version_dir_rel());
    }

    #[test]
    fn default_is_the_shipped_hex() {
        let p = resolve_hex(None).expect("the crate ships its firmware folder");
        assert_eq!(p, firmware_dir().unwrap().join(HEX_FILE));
        assert!(p.is_file());
    }

    #[test]
    fn bare_name_resolves_inside_the_folder() {
        let p = resolve_hex(Some("app_m8051_2500.hex")).expect("shipped next to the 2611 image");
        assert_eq!(p, firmware_dir().unwrap().join("app_m8051_2500.hex"));
    }

    #[test]
    fn missing_file_names_the_places_looked_at() {
        let err = resolve_hex(Some("no_such_image.hex")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.starts_with("firmware file no_such_image.hex: file not found"), "{msg}");
        // the places looked at: the argument, then the name inside the folder
        assert!(msg.contains("no_such_image.hex, "), "{msg}");
        assert!(msg.contains(&format!("{FIRMWARE_DIR}/{VENDOR_VERSION}")), "{msg}");
    }

    #[test]
    fn missing_folder_is_reported_not_panicked() {
        let err = no_folder();
        assert!(err.to_string().contains("firmware folder not found"), "{err}");
    }
}
