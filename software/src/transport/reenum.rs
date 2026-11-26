//! Re-enumeration handling - the 0471:2600 -> 0471:2500 swap.
//!
//! After a firmware launch (and after every self-reboot landing, which
//! the sensor does every 31-157 s) the device vanishes from the bus
//! and comes back as the runtime composite. The vendor engine handles
//! this with a plain poll loop around its device-open retry; we model
//! the same thing with a testable decision core plus (feature `usb`)
//! a rusb poll loop.
//!
//! Wire facts (cap2/cap3/cap4/cap8):
//! - the launch ack (`0x0401`) is the last frame on the 2600 device;
//! - the 2500 device appears within ~1-2 s (the engine re-polls at
//!   1 s Sleep intervals and connects on the first sighting);
//! - the engine never tries to boot a resident image: whenever it
//!   sees the bootloader again it re-runs the full flash sequence
//!   (that policy lives in [`crate::session::Session::recover`], this
//!   module only reports what the bus currently shows).
//!
//! The decision core is pure (a bus snapshot in, an action out) so the
//! self-reboot timing table can be replayed in unit tests without any
//! hardware.

use std::time::Duration;

use crate::protocol::{PID_BOOTLOADER, PID_RUNTIME};

/// How long the vendor engine waits for the new device before giving
/// up (its connect retry loop: 3 attempts, ~10 s each in the engine
/// host; we use the tighter observed re-enumeration budget).
pub const REENUM_TIMEOUT: Duration = Duration::from_secs(10);

/// Bus poll interval during the wait (the engine host polls at 1 s).
pub const REENUM_POLL: Duration = Duration::from_secs(1);

/// What one bus snapshot says about the sensor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusState {
    /// No 0471 sensor on the bus (the re-enumeration gap, or absent).
    None,
    /// Only the bootloader 0471:2600 - flash territory.
    Bootloader,
    /// The runtime 0471:2500 is present (with or without a stale
    /// bootloader entry - the runtime device wins, like the engine).
    Runtime,
    /// Only an older-generation PID (identification only).
    Legacy,
}

/// Classify one bus snapshot from the visible sensor PIDs.
pub fn bus_state(pids: &[u16]) -> BusState {
    use crate::protocol::{PID_V10, PID_V20, PID_V21};
    let mut any = false;
    let mut boot = false;
    let mut runtime = false;
    for &p in pids {
        match p {
            PID_RUNTIME => runtime = true,
            PID_BOOTLOADER => boot = true,
            PID_V21 | PID_V20 | PID_V10 => any = true,
            _ => {}
        }
    }
    if runtime {
        BusState::Runtime
    } else if boot {
        BusState::Bootloader
    } else if any {
        BusState::Legacy
    } else {
        BusState::None
    }
}

/// The result of one decision step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitEvent {
    /// Nothing relevant visible yet - keep polling.
    StillWaiting,
    /// The runtime 0471:2500 is on the bus - open it (claim MI_04) and
    /// connect / resume the session.
    RuntimeHere,
    /// The bootloader 0471:2600 is on the bus again - a self-reboot
    /// landed (or a launch never happened). The caller re-flashes via
    /// [`crate::session::Session::recover`].
    BootloaderAgain,
    /// The wait budget is spent.
    TimedOut,
}

/// The decision core: feed it bus snapshots and the elapsed time, get
/// the next action. Pure - no clock, no USB.
#[derive(Debug, Clone)]
pub struct ReenumWaiter {
    timeout: Duration,
    seen_gone: bool,
}

impl ReenumWaiter {
    /// New waiter with the given budget (default: [`REENUM_TIMEOUT`]).
    pub fn new(timeout: Duration) -> Self {
        ReenumWaiter { timeout, seen_gone: false }
    }

    /// One decision step over a bus snapshot.
    ///
    /// `pids` is every 0471 sensor PID currently on the bus;
    /// `elapsed` is the time since the wait began (the caller's clock).
    pub fn step(&mut self, pids: &[u16], elapsed: Duration) -> WaitEvent {
        match bus_state(pids) {
            BusState::Runtime => WaitEvent::RuntimeHere,
            BusState::Bootloader => {
                // A bootloader while we wait for the runtime: with
                // seen_gone this is a re-landing (self-reboot back to
                // the bootloader) - the caller re-flashes. Without
                // seen_gone the 2600 never left: the launch ack we
                // acted on did not take effect - same handling.
                WaitEvent::BootloaderAgain
            }
            BusState::None | BusState::Legacy => {
                self.seen_gone = true;
                if elapsed >= self.timeout {
                    WaitEvent::TimedOut
                } else {
                    WaitEvent::StillWaiting
                }
            }
        }
    }

    /// Whether the sensor was seen leaving the bus at least once
    /// (diagnostics for the timeout message).
    pub fn saw_departure(&self) -> bool {
        self.seen_gone
    }
}

impl Default for ReenumWaiter {
    fn default() -> Self {
        Self::new(REENUM_TIMEOUT)
    }
}

/// The rusb poll loop: wait until the runtime device shows up (the
/// standard wait after a flash launch or a self-reboot recovery).
///
/// Returns the runtime device summary; `Err(BootloaderState)` means the
/// sensor landed on the bootloader again - re-flash via
/// [`crate::session::Session::recover`].
#[cfg(feature = "usb")]
pub fn wait_runtime(timeout: Duration, poll: Duration) -> crate::Result<super::usb::UsbDeviceInfo> {
    wait_until(timeout, poll, |st| st == BusState::Runtime)
}

/// Generic rusb wait: poll [`super::usb::list_devices`] until `ready`
/// accepts the bus state. `RuntimeHere` requires the runtime device;
/// `BootloaderAgain` maps to [`crate::IdongError::BootloaderState`].
#[cfg(feature = "usb")]
pub fn wait_until(
    timeout: Duration,
    poll: Duration,
    ready: impl Fn(BusState) -> bool,
) -> crate::Result<super::usb::UsbDeviceInfo> {
    use std::time::Instant;

    let start = Instant::now();
    let mut waiter = ReenumWaiter::new(timeout);
    loop {
        let devices = super::usb::list_devices();
        let pids: Vec<u16> = devices.iter().map(|d| d.pid).collect();
        let state = bus_state(&pids);
        if ready(state) {
            let want = if state == BusState::Runtime { PID_RUNTIME } else { PID_BOOTLOADER };
            return devices.into_iter().find(|d| d.pid == want).ok_or(crate::IdongError::NotFound);
        }
        match waiter.step(&pids, start.elapsed()) {
            WaitEvent::StillWaiting => std::thread::sleep(poll),
            WaitEvent::RuntimeHere => continue, // covered by ready() above
            WaitEvent::BootloaderAgain => {
                return Err(crate::IdongError::BootloaderState);
            }
            WaitEvent::TimedOut => {
                return Err(crate::IdongError::Usb(format!(
                    "re-enumeration wait timed out after {timeout:?} (device seen leaving bus: {})",
                    waiter.saw_departure()
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_all_bus_states() {
        assert_eq!(bus_state(&[]), BusState::None);
        assert_eq!(bus_state(&[PID_BOOTLOADER]), BusState::Bootloader);
        assert_eq!(bus_state(&[PID_RUNTIME]), BusState::Runtime);
        // both on the bus: the runtime device wins, exactly like the
        // engine (it opens the 2500 composite for the session).
        assert_eq!(bus_state(&[PID_BOOTLOADER, PID_RUNTIME]), BusState::Runtime);
        assert_eq!(bus_state(&[0x2100]), BusState::Legacy);
        assert_eq!(bus_state(&[0x1234]), BusState::None);
    }

    #[test]
    fn launch_wait_gone_then_runtime() {
        // cap8 shape: launch ack at t=17.8 s, 2600 gone, 2500 on the
        // bus within ~1-2 s.
        let mut w = ReenumWaiter::new(REENUM_TIMEOUT);
        assert_eq!(w.step(&[PID_BOOTLOADER], Duration::ZERO), WaitEvent::BootloaderAgain);
        assert_eq!(w.step(&[], Duration::from_millis(200)), WaitEvent::StillWaiting);
        assert!(w.saw_departure());
        assert_eq!(w.step(&[], Duration::from_millis(800)), WaitEvent::StillWaiting);
        assert_eq!(w.step(&[PID_RUNTIME], Duration::from_millis(1500)), WaitEvent::RuntimeHere);
    }

    #[test]
    fn self_reboot_lands_on_bootloader_again() {
        // The recovery case: mid-session the sensor vanishes and comes
        // back as 2600 - the caller re-flashes (Session::recover).
        let mut w = ReenumWaiter::default();
        assert_eq!(w.step(&[], Duration::ZERO), WaitEvent::StillWaiting);
        assert_eq!(
            w.step(&[PID_BOOTLOADER], Duration::from_millis(500)),
            WaitEvent::BootloaderAgain
        );
    }

    #[test]
    fn timeout_after_budget() {
        let mut w = ReenumWaiter::new(Duration::from_secs(10));
        assert_eq!(w.step(&[], Duration::from_secs(9)), WaitEvent::StillWaiting);
        assert_eq!(w.step(&[], Duration::from_secs(10)), WaitEvent::TimedOut);
        assert!(w.saw_departure());
    }

    #[test]
    fn legacy_only_is_not_a_timeout_shortcut() {
        // An old-generation device on the bus must not be mistaken for
        // our sensor appearing - keep waiting until the budget is spent.
        let mut w = ReenumWaiter::new(Duration::from_secs(2));
        assert_eq!(w.step(&[0x2100], Duration::from_secs(1)), WaitEvent::StillWaiting);
        assert_eq!(w.step(&[0x0668], Duration::from_secs(2)), WaitEvent::TimedOut);
    }
}
