//! udev rule for unprivileged access on Linux.
//!
//! Without this rule the sensor needs root (or a udev-acl session) for
//! libusb to claim the vendor interface MI_04. The rule covers both
//! device states: the bootloader 0471:2600 and the runtime composite
//! 0471:2500 (claiming MI_04 does not touch the UVC/UAC interfaces).
//!
//! Install (Debian/Ubuntu/Arch the same):
//!
//! ```text
//! sudo cp /path/to/42-idong.rules /etc/udev/rules.d/
//! sudo udevadm control --reload-rules
//! sudo udevadm trigger
//! # replug the sensor (or: sudo udevadm trigger --action=change)
//! ```
//!
//! Then `cargo run --features usb --example list` works as a normal
//! user. Nothing here writes to the device; the rule only changes file
//! permissions.

/// The rule file name (42- keeps it before local admin rules).
pub const RULE_FILE: &str = "42-idong.rules";

/// The rule text, one line per device state.
pub const RULES: &str = "\
# Toltech i-Dong sensor (idong crate) - unprivileged libusb access
# bootloader: 0471:2600
SUBSYSTEM==\"usb\", ATTRS{idVendor}==\"0471\", ATTRS{idProduct}==\"2600\", MODE=\"0666\"
# runtime composite: 0471:2500 (vendor interface MI_04; UVC/UAC untouched)
SUBSYSTEM==\"usb\", ATTRS{idVendor}==\"0471\", ATTRS{idProduct}==\"2500\", MODE=\"0666\"
";

/// The install steps as a printable list.
pub const INSTALL_HELP: &str = "\
1. write the rule:              sudo tee /etc/udev/rules.d/42-idong.rules < 42-idong.rules
2. reload the rules:             sudo udevadm control --reload-rules
3. re-trigger the device nodes:  sudo udevadm trigger
4. unplug and replug the sensor";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_cover_both_states() {
        assert!(RULES.contains("idProduct}==\"2600\""));
        assert!(RULES.contains("idProduct}==\"2500\""));
        assert!(RULES.contains("MODE=\"0666\""));
    }
}
