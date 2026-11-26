//! The vendor files shipped in `software/firmware/` (turn 40) - the test
//! that keeps that folder trustworthy.
//!
//! Oracle: the turn-12 extraction manifest
//! (`research/analysis/reports/all_extracted_sha256.tsv`) and the pins in
//! `idong::FirmwareSet`. Checked here: every image against the manifest,
//! the three driver pins on the loaded set, that the flash gate refuses
//! every other image, every line of `SHA256SUMS` (and that no file of the
//! folder is missing from it), the vendor's own file map
//! (`xeyezHal.ini`, `xEyeUsb.ini`) against the folder, the register-table
//! format, and the folder lookup of `idong::firmware`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use idong::firmware::{self, sha256_hex, FIRMWARE_DIR, HEX_FILE, VENDOR_VERSION};
use idong::{FirmwareSet, IdongError};

/// The images of the folder with size and SHA-256 from the turn-12
/// manifest (xeyez_alone flavour; xeyez_Lobby is byte-identical).
const MANIFEST: &[(&str, usize, &str)] = &[
    (
        "v2.6.0.6/ens_code.ldr",
        54_420,
        "3a19c68a33352cda9b4c9e8a476c4978e19a5b983e567cff946fe543f51a60dd",
    ),
    (
        "v2.6.0.6/A_a_v4.4_BG0313.bit",
        235_930,
        "75df0a38037e1983d270f1003c2303c25a61dcdb554d455b1f963cd4a70b4abd",
    ),
    (
        "v2.6.0.6/app_m8051_2611.hex",
        61_756,
        "1fad00c1dca7f995e8b58d4ed6c5c080da4005f63682ad7af6288abd4d118f35",
    ),
    (
        "v2.6.0.6/app_m8051_2500.hex",
        61_031,
        "ffb8b96274c3b4c4d6400b75917f5a270431a21af6952d67967c1867e3a003e4",
    ),
    (
        "v2.6.0.6/xeyez_v21.hex",
        26_304,
        "0298a4d8a6501bcdef22965946f2d5dad11d211673f8915c804f65af9dea6854",
    ),
    (
        "v2.6.0.6/xeyez_v20.hex",
        25_913,
        "3f82b8ccd5a80aef7a476d4de0cb7c03f580eb777521d97aa76f12feadfce61d",
    ),
    (
        "v2.5.0.7/app_m8051.hex",
        57_683,
        "09df3d856775bbb0046b9a5cb515a78a5d3a0be3c8feffaa75ebbc897106b47e",
    ),
];

/// Files the vendor's ini names without the `#` comment marker that are
/// not in the vendor package at all (checked against the installer
/// listing in turn 40), so they cannot be shipped.
const ABSENT_IN_VENDOR_PACKAGE: &[&str] =
    &["Rolling_0316_Cmos_v26.txt", "Rolling_Img_0316_Fpga_v26.txt"];

/// `software/firmware/` of this source tree.
fn firmware_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIRMWARE_DIR)
}

/// Every regular file under `dir`, keyed by its path relative to `dir`
/// with `/` separators.
fn tree(dir: &Path) -> BTreeMap<String, PathBuf> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let rel = path.strip_prefix(base).expect("under base").to_string_lossy();
                out.insert(rel.replace('\\', "/"), path);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn read(rel: &str) -> Vec<u8> {
    std::fs::read(firmware_root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// A vendor text file as `String` (the vendor's GBK comment lines lose
/// their non-ASCII bytes; the keys, values and table lines are ASCII).
fn vendor_text(rel: &str) -> String {
    String::from_utf8_lossy(&read(rel)).into_owned()
}

/// The `(section, key, value)` pairs of a vendor .ini: blank lines and
/// `#` comment lines skipped, keys and values trimmed.
fn ini_pairs(text: &str) -> Vec<(String, String, String)> {
    let mut section = String::new();
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            section = name.trim().to_string();
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            out.push((section.clone(), key.trim().to_string(), value.trim().to_string()));
        }
    }
    out
}

/// Where a file named by the ini of `version` lives in the folder: that
/// version's own folder first, then the other one (the other vendor
/// package ships byte-identical copies of everything not in `v2.5.0.7/`).
fn shipped(version: &str, rel: &str) -> Option<PathBuf> {
    let twin = if version == VENDOR_VERSION { "v2.5.0.7" } else { VENDOR_VERSION };
    [version, twin].iter().find_map(|v| {
        let p = firmware_root().join(v).join(rel);
        p.is_file().then_some(p)
    })
}

#[test]
fn shipped_images_match_the_turn12_manifest() {
    for (rel, size, want) in MANIFEST {
        let bytes = read(rel);
        assert_eq!(bytes.len(), *size, "{rel}: size");
        assert_eq!(sha256_hex(&bytes), *want, "{rel}: sha256");
    }
}

#[test]
fn driver_pins_hold_on_the_loaded_set() {
    // the crate constants ARE the manifest values for the three pinned files
    let pinned =
        ["v2.6.0.6/ens_code.ldr", "v2.6.0.6/A_a_v4.4_BG0313.bit", "v2.6.0.6/app_m8051_2611.hex"];
    for (slot, rel) in pinned.iter().enumerate() {
        let (_, _, want) = MANIFEST.iter().find(|m| m.0 == *rel).expect("manifest entry");
        assert_eq!(FirmwareSet::SHA256[slot], *want, "{rel}");
    }
    let set = firmware::load_set().expect("the shipped set verifies against the pins");
    let image = set.mcu_image().expect("flattened image pin");
    assert_eq!(image.len(), 21_411);
    assert_eq!(sha256_hex(&image), FirmwareSet::MCU_IMAGE_SHA256);
}

#[test]
fn flash_gate_refuses_every_other_image() {
    // only app_m8051_2611.hex is a pinned MCU image; the gate rejects the
    // rest (and the non-hex images) before any byte leaves the host
    for rel in [
        "v2.6.0.6/app_m8051_2500.hex",
        "v2.6.0.6/xeyez_v21.hex",
        "v2.6.0.6/xeyez_v20.hex",
        "v2.5.0.7/app_m8051.hex",
        "v2.6.0.6/ens_code.ldr",
        "v2.6.0.6/A_a_v4.4_BG0313.bit",
    ] {
        assert!(
            matches!(
                FirmwareSet::verify_hex(&read(rel)),
                Err(IdongError::FirmwareIntegrity { .. })
            ),
            "{rel} must not pass the flash gate"
        );
    }
    assert!(FirmwareSet::verify_hex(&read("v2.6.0.6/app_m8051_2611.hex")).is_ok());
}

#[test]
fn sha256sums_lists_every_file_with_the_right_hash() {
    let sums = read("SHA256SUMS");
    let text = std::str::from_utf8(&sums).expect("SHA256SUMS is ASCII");
    let mut listed: Vec<String> = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let (digest, rel) = line.split_once("  ").unwrap_or_else(|| panic!("bad line {line:?}"));
        assert_eq!(digest.len(), 64, "sha256 hex in {line:?}");
        let bytes = read(rel);
        assert_eq!(sha256_hex(&bytes), digest, "{rel}");
        listed.push(rel.to_string());
    }
    // sorted, and exactly the files of the folder (the two generated
    // documents excluded)
    let mut want: Vec<String> = tree(&firmware_root())
        .into_keys()
        .filter(|k| k != "README.md" && k != "SHA256SUMS")
        .collect();
    want.sort();
    listed.sort();
    assert_eq!(listed, want);
    assert_eq!(listed.len(), 24);
}

#[test]
fn vendor_file_map_resolves_against_the_folder() {
    let mut seen_absent: Vec<String> = Vec::new();
    for (version, hex_by_section) in [
        (
            "v2.6.0.6",
            vec![
                ("Version2.0", "xeyez_v20.hex"),
                ("Version2.1", "xeyez_v21.hex"),
                ("Version2.5", "app_m8051_2500.hex"),
                ("Version2.6", HEX_FILE),
            ],
        ),
        (
            "v2.5.0.7",
            vec![
                ("Version2.0", "xeyez_v20.hex"),
                ("Version2.1", "xeyez_v21.hex"),
                ("Version2.5", "app_m8051.hex"),
            ],
        ),
    ] {
        let pairs = ini_pairs(&vendor_text(&format!("{version}/config/xeyezHal.ini")));

        // which MCU image belongs to which sensor generation
        let hexes: Vec<(&str, &str)> = pairs
            .iter()
            .filter(|(_, key, _)| key == "HexFileName")
            .map(|(section, _, value)| (section.as_str(), value.as_str()))
            .collect();
        assert_eq!(hexes, hex_by_section, "{version}: HexFileName per section");
        for (_, name) in &hexes {
            assert!(shipped(version, name).is_some(), "{version}: {name} not shipped");
        }

        // every active table entry names a shipped file
        for (section, key, value) in &pairs {
            if key == "HexFileName" || value.starts_with('#') || value.is_empty() {
                continue;
            }
            if ["Update", "LastVersion", "RunGlobal"].contains(&key.as_str()) {
                continue;
            }
            // the payload root of the installer is config/ here; the
            // 30fps folders keep their name
            let rel = value.trim_start_matches(".\\").replace('\\', "/");
            let name = Path::new(&rel).file_name().map(|n| n.to_string_lossy().into_owned());
            match shipped(version, &format!("config/{rel}")) {
                Some(_) => {}
                None if name.as_deref().is_some_and(|n| ABSENT_IN_VENDOR_PACKAGE.contains(&n)) => {
                    seen_absent.push(name.expect("file name"))
                }
                None => panic!("{version} [{section}] {key} names {value}: not shipped"),
            }
        }
    }
    // the absent list is not stale: each name is still named by the vendor
    let mut want = ABSENT_IN_VENDOR_PACKAGE.to_vec();
    want.sort();
    seen_absent.sort();
    seen_absent.dedup();
    assert_eq!(seen_absent, want);
}

#[test]
fn vendor_bit_file_entry_names_the_pinned_bitstream() {
    let bit: Vec<_> = ini_pairs(&vendor_text("v2.6.0.6/config/xEyeUsb.ini"))
        .into_iter()
        .filter(|(_, key, _)| key == "BitFile")
        .collect();
    assert_eq!(bit.len(), 1);
    assert_eq!(bit[0].2, firmware::BIT_FILE);
    assert!(shipped(VENDOR_VERSION, firmware::BIT_FILE).is_some());
}

#[test]
fn register_tables_are_register_value_lines() {
    // the `.txt` / `.dat` files under config/ (the `.ini` files there are
    // the vendor's text configuration, checked by the file-map tests)
    let mut checked = 0;
    for rel in tree(&firmware_root()).into_keys() {
        if !rel.contains("/config/") || !(rel.ends_with(".txt") || rel.ends_with(".dat")) {
            continue;
        }
        let mut lines = 0;
        for raw in vendor_text(&rel).lines() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            let (reg, val) = line.split_once(',').unwrap_or_else(|| panic!("{rel}: {line:?}"));
            assert!(is_hex_byte(reg) && is_hex_byte(val), "{rel}: {line:?} is not 0xRR,0xVV");
            lines += 1;
        }
        assert!(lines > 0, "{rel}: empty table");
        checked += 1;
    }
    assert_eq!(checked, 12, "config/ holds 12 register tables");
}

fn is_hex_byte(s: &str) -> bool {
    s.len() == 4 && s.starts_with("0x") && s[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

#[test]
fn lookup_rules_of_the_firmware_module() {
    let dir = firmware::firmware_dir().expect("the crate ships firmware/<version>");
    assert_eq!(dir, firmware_root().join(VENDOR_VERSION));

    // no argument: the shipped image; a bare name: the same folder
    let default = firmware::resolve_hex(None).expect("default hex");
    assert_eq!(default, dir.join(HEX_FILE));
    assert!(default.is_file());
    assert_eq!(firmware::resolve_hex(Some(HEX_FILE)).expect("bare name"), default);
    assert_eq!(
        firmware::resolve_hex(Some("app_m8051_2500.hex")).expect("bare name"),
        dir.join("app_m8051_2500.hex")
    );

    // a path is taken as given
    let explicit = firmware_root().join("v2.5.0.7/app_m8051.hex");
    assert_eq!(
        firmware::resolve_hex(Some(explicit.to_str().unwrap())).expect("explicit path"),
        explicit
    );

    // nothing else resolves, and the error names what was looked at
    let err = firmware::resolve_hex(Some("no_such_image.hex")).expect_err("must fail");
    let msg = err.to_string();
    assert!(msg.contains("no_such_image.hex") && msg.contains("file not found"), "{msg}");

    // the default read is the pinned file
    let (path, bytes) = firmware::read_hex(None).expect("read default");
    assert_eq!(path, default);
    assert!(FirmwareSet::verify_hex(&bytes).is_ok());
}
