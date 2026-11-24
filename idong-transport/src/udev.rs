//! udev rule for unprivileged access (no root requirement in the lib):
//! SUBSYSTEM=="usb", ATTRS{idVendor}=="0471", ATTRS{idProduct}=="2600",
//! MODE="0666"  (+ "2500" companion rule for the runtime composite).
// TODO(P2.2): rule text + install helper.
