//! After firmware upload the device re-enumerates as 0471:2500.
//! Wait via rusb hotplug if available, else poll with backoff; then
//! open_runtime() claims MI_04 (vendor bulk interface).
// TODO(P3.2)
