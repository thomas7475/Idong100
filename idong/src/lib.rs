//! # idong - pure protocol core for the i-Dong Sensor V2.6
//!
//! Zero USB and zero ROS dependencies: everything here is testable with
//! `cargo test` alone, against golden vectors under `tests/data/`.
//! The crate facade is [`Idong`]; see the implementation draft (turn 13)
//! section 4 for the file map and section 7 for the phase plan.
//!
//! Facts in this crate are cited to their decompiled source in
//! `docs/PROTOCOL.md` - the single source of truth for constants.
//!
//! # Safety model
//!
//! 1. Nothing in this crate writes to the device except the
//!    [`protocol::boot::BootUploader`] (the exact vendor upload
//!    sequence, further gated by [`protocol::boot::BOOT_CONFIRMED`])
//!    and the documented runtime replies (keep-alive `0xFFE`,
//!    link-status word `0x11`).
//! 2. No write-capable code path runs on real hardware before the boot
//!    upload is byte-confirmed against a USBPcap capture of the vendor
//!    stack (draft gate P2.3).
//! 3. The T24C16A EEPROM is never written: the parser is read-only.
//! 4. Firmware files are SHA-256-pinned (the turn-12 manifest) before
//!    any upload; a mismatch aborts before the first byte leaves the
//!    host ([`FirmwareSet::verify`]).
//!
//! ```no_run
//! use std::time::Duration;
//! use idong::transport::Transport; // the seam: mock in CI, rusb on the bench
//!
//! // A live (or mocked) transport is provided by idong-transport / tests;
//! // the core never opens USB itself.
//! let mut transport: Box<dyn Transport + Send> = unimplemented!("from idong-transport");
//! let streamer = idong::stream::Streamer::start(transport, 5);
//! loop {
//!     let f = streamer.read_frame(Duration::from_millis(500)).expect("frame");
//!     println!("counter {} markers {}", f.frame_counter, f.markers.len());
//! }
//! ```

#![deny(unsafe_code)]

pub mod calib;
pub mod error;
pub mod handle;
pub mod marker;
pub mod mock;
pub mod protocol;
pub mod stream;
pub mod transport;

pub use calib::{DeviceIdentity, StereoCalib};
pub use error::{BootStep, IdongError, ProtocolError, Result};
pub use handle::HandleEvent;
pub use marker::{Marker, MarkerFrame, Quality};
pub use stream::{StatsSnapshot, StreamStats, Streamer};
pub use transport::Transport;
pub use protocol::video::{ProbeCommit, VideoAssembler, VideoFrame, VideoSource};

/// Firmware images the user must supply (never redistributed with the
/// crate - safety rule: users bring their own vendor files).
///
/// Hashes pinned from the turn-12 manifest (v2.6.0.6 stack,
/// `all_extracted_sha256.tsv`), encoded in [`FirmwareSet::SHA256`].
#[derive(Debug, Clone)]
pub struct FirmwareSet {
    /// `ens_code.ldr` - Blackfin BF527 loader (54,420 B).
    pub ldr: Vec<u8>,
    /// `A_a_v4.4_BG0313.bit` - Spartan-3 FPGA bitstream (235,930 B).
    pub bit: Vec<u8>,
    /// `app_m8051_2611.hex` - 8051 microcontroller (61,756 B).
    pub hex: Vec<u8>,
}

impl FirmwareSet {
    /// SHA-256 pins from the turn-12 manifest, order `[ldr, bit, hex]`.
    pub const SHA256: [&str; 3] = [
        // ens_code.ldr (identical in v2.5.0.7 and v2.6.0.6 stacks)
        "3a19c68a33352cda9b4c9e8a476c4978e19a5b983e567cff946fe543f51a60dd",
        // A_a_v4.4_BG0313.bit
        "75df0a38037e1983d270f1003c2303c25a61dcdb554d455b1f963cd4a70b4abd",
        // app_m8051_2611.hex
        "1fad00c1dca7f995e8b58d4ed6c5c080da4005f63682ad7af6288abd4d118f35",
    ];

    /// SHA-256 of the flattened MCU image the vendor downloads to the
    /// 2500 device (turn 32, byte-verified against three captures).
    pub const MCU_IMAGE_SHA256: &str =
        "5da1565c6704d387081844e712b4be4d521083e21a619df7d6cb11551c2aba9c";

    /// Build the 21,411-B MCU image the vendor sends with 0x0101:
    /// the Intel-HEX records of `app_m8051_2611.hex` flattened over
    /// [0x8000, 0xD3A3) with 0x00 gap fill. Byte-identical to the
    /// download in cap2/cap3/cap4 (readback-verified 335/335 blocks).
    ///
    /// Rule: hex records below 0x8000 (the 3-B reset vector at 0x0000)
    /// are ignored - the vendor image starts at 0x8000; hex gaps become
    /// 0x00, not the 0xFF flash-erase value.
    pub fn mcu_image(&self) -> Result<Vec<u8>> {
        use std::path::PathBuf;
        let text = std::str::from_utf8(&self.hex)
            .map_err(|_| IdongError::Usb("app_m8051_2611.hex is not ASCII".into()))?;
        let (base, end) = (0x8000usize, 0xD3A3usize);
        let mut mem = vec![0u8; end - base];
        let mut hibase = 0usize;
        for line in text.lines() {
            let line = line.trim();
            if !line.starts_with(':') {
                continue;
            }
            let b = Self::hex_bytes(line.as_bytes())
                .ok_or_else(|| IdongError::Usb(format!("bad hex line: {line}")))?;
            if b.len() < 5 {
                continue;
            }
            let (cnt, typ) = (b[0] as usize, b[3]);
            let data = &b[4..4 + cnt];
            match typ {
                0x00 => {
                    let addr = hibase + ((b[1] as usize) << 8) + b[2] as usize;
                    if addr >= base && addr + data.len() <= end {
                        mem[addr - base..addr - base + data.len()].copy_from_slice(data);
                    }
                }
                0x02 => hibase = (((data[0] as usize) << 8) + data[1] as usize) << 4,
                0x04 => hibase = (((data[0] as usize) << 8) + data[1] as usize) << 16,
                0x01 => break,
                _ => {}
            }
        }
        if Self::sha256hex(&mem) != Self::MCU_IMAGE_SHA256 {
            return Err(IdongError::FirmwareIntegrity {
                path: PathBuf::from("app_m8051_2611.hex (flattened MCU image)"),
                got: Self::sha256hex(&mem),
                want: Self::MCU_IMAGE_SHA256.to_string(),
            });
        }
        Ok(mem)
    }

    /// Minimal ASCII-hex pair decoder (no external deps in the core).
    fn hex_ascii_val(c: u8) -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'A'..=b'F' => Some(c - b'A' + 10),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        }
    }

    fn hex_bytes(s: &[u8]) -> Option<Vec<u8>> {
        let body = &s[1..]; // skip ':'
        if body.len() % 2 != 0 {
            return None;
        }
        let mut out = Vec::with_capacity(body.len() / 2);
        for pair in body.chunks(2) {
            out.push((Self::hex_ascii_val(pair[0])? << 4) | Self::hex_ascii_val(pair[1])?);
        }
        Some(out)
    }

    /// Verify each image against its pinned SHA-256 (safety rule #3:
    /// a mismatch aborts before the first byte leaves the host).
    pub fn verify(&self) -> Result<()> {
        use std::path::PathBuf;
        let names = ["ens_code.ldr", "A_a_v4.4_BG0313.bit", "app_m8051_2611.hex"];
        let blobs = [&self.ldr, &self.bit, &self.hex];
        for ((blob, want), name) in blobs.iter().zip(Self::SHA256.iter()).zip(names.iter()) {
            let got = Self::sha256hex(blob);
            if got != *want {
                return Err(IdongError::FirmwareIntegrity {
                    path: PathBuf::from(*name),
                    got,
                    want: want.to_string(),
                });
            }
        }
        Ok(())
    }

    fn sha256hex(data: &[u8]) -> String {
        // No external crypto dep in the pure core: SHA-256 implemented
        // inline (FIPS 180-4), used only for the integrity gate.
        let mut h: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];
        let k: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut msg = data.to_vec();
        let bit_len = (data.len() as u64) * 8;
        msg.push(0x80);
        while msg.len() % 64 != 56 {
            msg.push(0);
        }
        msg.extend_from_slice(&bit_len.to_be_bytes());
        for block in msg.chunks(64) {
            let mut w = [0u32; 64];
            for i in 0..16 {
                w[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
            }
            let mut v = h;
            for i in 0..64 {
                let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
                let ch = (v[4] & v[5]) ^ ((!v[4]) & v[6]);
                let t1 =
                    v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(k[i]).wrapping_add(w[i]);
                let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
                let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
                let t2 = s0.wrapping_add(maj);
                v[7] = v[6];
                v[6] = v[5];
                v[5] = v[4];
                v[4] = v[3].wrapping_add(t1);
                v[3] = v[2];
                v[2] = v[1];
                v[1] = v[0];
                v[0] = t1.wrapping_add(t2);
            }
            for i in 0..8 {
                h[i] = h[i].wrapping_add(v[i]);
            }
        }
        h.iter().map(|x| format!("{x:08x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vectors() {
        // FIPS 180-4 / NIST test vectors - proof the integrity gate hashes
        assert_eq!(
            FirmwareSet::sha256hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            FirmwareSet::sha256hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            FirmwareSet::sha256hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }
}
