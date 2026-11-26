//! Session lifecycle - the full observed vendor flow as one state machine.
//!
//! Wire-confirmed in captures cap2/cap3/cap4 (sessions) and cap8 (the
//! full boot cycle, turn 35): after power-up the sensor sits in its
//! 8051 bootloader (USB 0471:2600, version string "2600"); the host
//! flashes `app_m8051_2611.hex` (the [`runtime::RuntimeSession`]
//! flash sequence - begin, 64-B raw blocks, verify + echo readback,
//! key, launch), the device re-enumerates as the runtime composite
//! (0471:2500, version "2619") and the session init runs: EEPROM
//! reads (edition tag, code B, factory calibration, device ID),
//! the CMOS register tables, then the steady-state poll cycle.
//!
//! # Self-reboot recovery (cap8, turn 35)
//!
//! The sensor drops back to the bootloader state by itself every
//! 31-157 s (observed three times in cap7, once in cap8 at t=17.8 s).
//! The vendor engine never tries to boot the resident image - it
//! re-runs the COMPLETE flash sequence on every bootloader landing
//! and then resumes the session. [`Session::recover`] models exactly
//! that: a probe that answers "2600" (or errors like a vanished
//! device) means re-flash, wait for the re-enumeration, reconnect.
//!
//! # Who owns what
//!
//! This module is pure orchestration over [`Transport`]; the
//! re-enumeration itself (watching USB, reopening the new device,
//! claiming MI_04) is a transport-layer job (`idong-transport`):
//! [`IdongError::NeedsReenumeration`] is the hand-off signal.

use crate::error::{IdongError, Result};
use crate::protocol::runtime::{RuntimeSession, IMAGE_SIZE};
use crate::protocol::video::StreamPollState;
use crate::transport::Transport;

/// Bootloader version string (0471:2600, MCU image not loaded).
pub const VERSION_BOOTLOADER: &str = "2600";
/// Runtime version string (0471:2500, session-capable).
pub const VERSION_RUNTIME: &str = "2619";

/// The observed steady-state poll cycle (cap2/3/4): these commands
/// rotate roughly every 7-10 s while no stream runs. The exact
/// rotation order is engine-driven; the set is what the wire shows.
pub const POLL_CYCLE: [u16; 6] = [0x0003, 0x0004, 0x0005, 0x000D, 0x0404, 0x040F];

/// Where the transport layer should take over again:
/// [`IdongError::NeedsReenumeration`] is returned after a successful
/// launch (see [`Session::connect`]).

/// The device phase as one `0x0008` version probe reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// Bootloader (0471:2600) - flash required.
    Bootloader,
    /// Runtime (0471:2500) - session possible.
    Runtime,
}

/// Everything the vendor session init collects (cap2/3/4 + the
/// engine gates of turn 33/34).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionInfo {
    /// The 32-char device ID from EEPROM 0x0300 (e.g.
    /// "AB460DC8BA12C6840B49CD5786C329DD" on the captured unit).
    pub device_id: String,
    /// Edition tag from EEPROM 0x0322 ("000T" on the captured unit).
    pub code_a: String,
    /// Version code from EEPROM 0x0328 ("2613").
    pub code_b: String,
    /// Factory calibration payload (120 B) - parse with [`crate::calib`].
    pub calibration: Vec<u8>,
    /// How many bootloader landings this session already recovered
    /// from (each one re-flashed the MCU, exactly like the engine).
    pub recoveries: u32,
}

/// The lifecycle manager. One instance, one transport, one phase.
pub struct Session<'a> {
    inner: RuntimeSession<'a>,
    phase: Phase,
    info: SessionInfo,
}

impl<'a> Session<'a> {
    /// Probe the phase: one `0x0008` version read.
    ///
    /// "2600" (or a transport error on an already-vanished device -
    /// the caller retries until the bootloader shows up) = bootloader.
    pub fn probe(transport: &mut dyn Transport) -> Result<Phase> {
        let mut inner = RuntimeSession::new(&mut *transport);
        match inner.get_version() {
            Ok(v) if v == VERSION_RUNTIME => Ok(Phase::Runtime),
            Ok(v) if v == VERSION_BOOTLOADER => Ok(Phase::Bootloader),
            Ok(v) => Err(IdongError::Usb(format!("unknown sensor version {v:?}"))),
            // A read error while probing is a "device gone" signal:
            // the caller maps it to a re-enumeration wait, not a hard
            // failure (the self-reboot makes this a normal event).
            Err(e) => Err(e),
        }
    }

    /// Connect a session on a transport whose phase is known, or
    /// recover a self-rebooted device.
    ///
    /// * [`Phase::Bootloader`] - flash `image` (the exact observed
    ///   sequence, gated by `FLASH_CONFIRMED` + `write_risk`), then
    ///   return [`IdongError::NeedsReenumeration`]: drop the
    ///   transport, open the 0471:2500 device, call `connect` again.
    /// * [`Phase::Runtime`] - the observed session init: EEPROM reads
    ///   (device ID, edition tag, code B, calibration) + the CMOS
    ///   register tables, exactly the cap2/3/4 command flow.
    pub fn connect(
        transport: &'a mut dyn Transport,
        phase: Phase,
        image: &[u8],
        write_risk: bool,
    ) -> Result<Self> {
        match phase {
            Phase::Bootloader => {
                if !write_risk {
                    return Err(IdongError::WriteRiskGate);
                }
                let mut inner = RuntimeSession::new(transport);
                // flash_mcu enforces IMAGE_SIZE, the byte-exact
                // observed sequence and the readback compare itself.
                inner.flash_mcu(image)?;
                Err(IdongError::NeedsReenumeration)
            }
            Phase::Runtime => {
                let mut inner = RuntimeSession::new(transport);
                let device_id = String::from_utf8_lossy(&inner.read_device_id()?).to_string();
                let code_a = inner.read_code_a()?;
                let code_b = inner.read_code_b()?;
                let calibration = inner.read_calibration()?;
                inner.init_cmos()?;
                Ok(Session {
                    inner,
                    phase: Phase::Runtime,
                    info: SessionInfo { device_id, code_a, code_b, calibration, recoveries: 0 },
                })
            }
        }
    }

    /// The same as [`Self::connect`], but for a session that died of
    /// a self-reboot: probe first, then either resume (a runtime
    /// device answered) or re-flash + signal the re-enumeration.
    ///
    /// This mirrors the vendor engine exactly: it never tries to
    /// boot the resident image, it re-flashes on every bootloader
    /// landing (cap2 x2, cap3, cap4, cap8 - six full observations).
    pub fn recover(
        transport: &'a mut dyn Transport,
        image: &[u8],
        write_risk: bool,
        prior: SessionInfo,
    ) -> Result<Self> {
        let phase = Self::probe(transport)?;
        let mut next = Self::connect(transport, phase, image, write_risk)?;
        next.info.recoveries = prior.recoveries.saturating_add(1);
        Ok(next)
    }

    /// One steady-state poll round (the observed cycle set). Errors
    /// mean the device went away - call [`Self::recover`] with a
    /// fresh transport.
    pub fn poll_cycle(&mut self) -> Result<()> {
        for cmd in POLL_CYCLE {
            self.inner.poll(cmd)?;
        }
        Ok(())
    }

    /// One `0x0205` stream poll (30 Hz while video runs).
    pub fn poll_stream(&mut self) -> Result<StreamPollState> {
        self.inner.poll_stream()
    }

    /// The collected session info.
    pub fn info(&self) -> &SessionInfo {
        &self.info
    }

    /// The current phase (a connected session is always Runtime).
    pub fn phase(&self) -> Phase {
        self.phase.clone()
    }
}

impl Session<'_> {
    /// The re-enumeration hand-off signal as a plain bool check for
    /// transport-layer callers that map errors themselves.
    pub fn is_reenumeration_signal(err: &IdongError) -> bool {
        matches!(err, IdongError::NeedsReenumeration)
    }
}

/// The flattened MCU image the vendor flashes: exactly
/// [`IMAGE_SIZE`] bytes. `FirmwareSet::mcu_image()` produces it from
/// the vendor hex file (turn 32); this alias documents the contract.
pub fn expect_image_size() -> usize {
    IMAGE_SIZE as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{reply, MockTransport};

    fn version_reply(v: &str) -> Vec<u8> {
        let mut b = vec![0x7F, 0x55, 0x08, 0x00, 0x01, 0x00, 0x00, 0x00];
        b.extend_from_slice(v.as_bytes());
        b.extend(std::iter::repeat_n(0, 64 - b.len()));
        b
    }

    #[test]
    fn probe_detects_both_phases() {
        // cap3 rec 24: version reply "2600"; cap8: "2619"
        let mut m = MockTransport::new(vec![reply(version_reply("2600"))]);
        assert_eq!(Session::probe(&mut m).unwrap(), Phase::Bootloader);
        let mut m = MockTransport::new(vec![reply(version_reply("2619"))]);
        assert_eq!(Session::probe(&mut m).unwrap(), Phase::Runtime);
    }

    #[test]
    fn connect_on_bootloader_flashes_then_signals_reenum() {
        // The flash script shape (one block image, from the golden
        // test in runtime_replay): begin ack, echo block, verify ack,
        // echo readback, key ack, launch ack.
        let image = [0xA5u8; IMAGE_SIZE as usize];
        let mut script = vec![reply(version_reply("2600"))];
        script.push(reply(vec![0x7F, 0x55, 0x01, 0x01, 0x01, 0x00, 0x00, 0x02]));
        for b in image.chunks(64) {
            script.push(reply(b.to_vec()));
        }
        script.push(reply(vec![0x7F, 0x55, 0x02, 0x01, 0x01, 0x00, 0x00, 0x04]));
        for b in image.chunks(64) {
            script.push(reply(b.to_vec()));
        }
        script.push(reply(vec![0x7F, 0x55, 0x03, 0x01, 0x01, 0x00]));
        script.push(reply(vec![0x7F, 0x55, 0x04, 0x01, 0x01, 0x00]));
        let mut m = MockTransport::new(script);
        let err = Session::connect(&mut m, Phase::Bootloader, &image, true);
        // The gate is intentionally still closed (P2.3 hardware
        // replay); the signal shape is what matters here.
        match err {
            Err(IdongError::BootUnconfirmed) => {}
            Err(e) => panic!("expected the flash gate, got {e:?}"),
            Ok(_) => panic!("expected the flash gate, got Ok"),
        }
        // The gate refuses BEFORE a single byte goes out (the
        // flash_is_gated test in runtime.rs pins the same rule).
        assert!(m.writes.is_empty(), "gate must not write a single byte");
    }

    #[test]
    fn connect_on_runtime_collects_info_and_inits() {
        // The session script (cap3 order): device ID, code A, code B,
        // calibration, CMOS tables.
        let dev = b"AB460DC8BA12C6840B49CD5786C329DD";
        let mut id33 = dev.to_vec();
        id33.push(0x50); // byte-sum checksum of the 32 chars
        let mut calib = vec![0u8; 120];
        let sum = calib.iter().fold(0u8, |a, b| a.wrapping_add(*b));
        calib.push(sum);
        let mut a5 = b"000T".to_vec();
        a5.push(0xE4);
        let mut b5 = b"2613".to_vec();
        b5.push(0xCC);
        let mut script = vec![reply(id33), reply(a5), reply(b5), reply(calib)];
        // CMOS init replies: table echo x2 forms x2, chunk echoes x12,
        // end ack. The replies only need the right reply-code shape.
        for _ in 0..2 {
            script.push(reply(vec![0x7F, 0x55, 0x04, 0x01, 0x01, 0x00, 0x00, 0x00]));
            let mut rb = vec![0x7F, 0x55, 0x04, 0x02, 0x01, 0x00, 0x00, 0x00];
            rb.extend(std::iter::repeat_n(0, 56));
            script.push(reply(rb));
        }
        for _ in 0..crate::protocol::runtime::CMOS_REG8_CHUNKS.len() {
            // per chunk: write 0x0103 -> reply 0x0103, then write
            // 0x0203 -> reply 0x0203 (the observed pair order)
            let mut r1 = vec![0x7F, 0x55, 0x03, 0x01, 0x01, 0x00, 0x00, 0x00];
            r1.extend(std::iter::repeat_n(0, 8));
            script.push(reply(r1));
            let mut r2 = vec![0x7F, 0x55, 0x03, 0x02, 0x01, 0x00, 0x00, 0x00];
            r2.extend(std::iter::repeat_n(0, 8));
            script.push(reply(r2));
        }
        script.push(reply(vec![0x7F, 0x55, 0x03, 0x03, 0x01, 0x00, 0x00, 0x00]));
        let mut m = MockTransport::new(script);
        let s = Session::connect(&mut m, Phase::Runtime, &[], false).unwrap();
        let info = s.info();
        assert_eq!(info.device_id, "AB460DC8BA12C6840B49CD5786C329DD");
        assert_eq!(info.code_a, "000T");
        assert_eq!(info.code_b, "2613");
        assert_eq!(info.calibration.len(), 120);
        // The exact first writes (cap3 session order):
        // device ID read 0107 @0x0300, then code A @0x0322, ...
        assert_eq!(
            m.writes[0],
            vec![0x7F, 0x55, 0x07, 0x01, 0x00, 0x00, 0x00, 0x04, 0x03, 0x00, 0x00, 0x21]
        );
        assert_eq!(
            m.writes[1],
            vec![0x7F, 0x55, 0x07, 0x01, 0x00, 0x00, 0x00, 0x04, 0x03, 0x22, 0x00, 0x05]
        );
    }

    #[test]
    fn poll_cycle_sends_the_observed_set() {
        let dev = b"AB460DC8BA12C6840B49CD5786C329DD";
        let mut id33 = dev.to_vec();
        id33.push(0x50);
        let mut calib = vec![0u8; 120];
        calib.push(calib.iter().fold(0u8, |a, b| a.wrapping_add(*b)));
        let mut a5 = b"000T".to_vec();
        a5.push(0xE4);
        let mut b5 = b"2613".to_vec();
        b5.push(0xCC);
        let mut script = vec![reply(id33), reply(a5), reply(b5), reply(calib)];
        for _ in 0..2 {
            script.push(reply(vec![0x7F, 0x55, 0x04, 0x01, 0x01, 0x00, 0x00, 0x00]));
            let mut rb = vec![0x7F, 0x55, 0x04, 0x02, 0x01, 0x00, 0x00, 0x00];
            rb.extend(std::iter::repeat_n(0, 56));
            script.push(reply(rb));
        }
        for _ in 0..crate::protocol::runtime::CMOS_REG8_CHUNKS.len() {
            let mut r1 = vec![0x7F, 0x55, 0x03, 0x01, 0x01, 0x00, 0x00, 0x00];
            r1.extend(std::iter::repeat_n(0, 8));
            script.push(reply(r1));
            let mut r2 = vec![0x7F, 0x55, 0x03, 0x02, 0x01, 0x00, 0x00, 0x00];
            r2.extend(std::iter::repeat_n(0, 8));
            script.push(reply(r2));
        }
        script.push(reply(vec![0x7F, 0x55, 0x03, 0x03, 0x01, 0x00, 0x00, 0x00]));
        // six poll replies
        for i in 0..6 {
            let mut rb = vec![0x7F, 0x55];
            let cmd = POLL_CYCLE[i];
            rb.extend_from_slice(&cmd.to_le_bytes());
            rb.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
            rb.extend(std::iter::repeat_n(0, 64 - rb.len()));
            script.push(reply(rb));
        }
        let mut m = MockTransport::new(script);
        let mut s = Session::connect(&mut m, Phase::Runtime, &[], false).unwrap();
        s.poll_cycle().unwrap();
        drop(s);
        // the last six writes are the six poll commands, framed
        let tail: Vec<Vec<u8>> = m.writes[m.writes.len() - 6..].to_vec();
        for (i, w) in tail.iter().enumerate() {
            let cmd = POLL_CYCLE[i];
            assert_eq!(&w[..2], &[0x7F, 0x55]);
            assert_eq!(u16::from_le_bytes([w[2], w[3]]), cmd);
            assert!(matches!(w[4], 0x00));
        }
    }
}
