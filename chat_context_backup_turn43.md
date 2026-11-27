# Chat context backup — turn 43 — 2026-11-27

Project: idong100 (Rust driver for the Toltech i-Dong Sensor V2.6)
Repository: https://github.com/thomas7475/Idong100, `main` = `c5681ab` (turn 43)

## The request

The user posted the first live T9 output (`video --iso --cmos
--frames 10`) plus `dmesg | tail -50`: steps [1/4]-[3/4] PASS (session
init, raw node + config walk, DISCONNECT_CLAIM + UVC probe/commit),
then `FAIL: SUBMITURB: No such file or directory (os error 2)` - and
asked to analyze the problem. The PAT was in the message again
(rotate it).

## The analysis (what ENOENT means here)

- os error 2 = ENOENT from the host controller, NOT a missing
  /dev/bus/usb node: Linux arms an isochronous endpoint only in the
  SELECTED alternate setting of its interface. The VS interface stood
  at alt 0 (the zero-bandwidth alt, no endpoints) - no transfer ring
  for EP 0x85, so every ISO SUBMITURB is refused.
- The turn-42 code deliberately skipped SET_INTERFACE to mirror the
  vendor capture (video flowed on 0x85 with the interface at alt 0).
  That flow cannot be replayed through Linux usbfs; the Windows stack
  evidently armed its ISO pipe outside the captured segment.
- dmesg (user-provided) shows nothing else wrong: uvcvideo binds
  0471:2500 but fails entity registration (-22, the vendor-mangled UVC
  graph - the same reason T7 STREAMON gives EIO); "invalid maxpacket
  64" on the HS bulk EP is vendor-descriptor noise. The log also shows
  the user re-flashed before the run (device 56 = 0471:2600 bootloader,
  57 = runtime 0471:2500).

## What shipped (turn 43: state commit c1393d6, merge c5681ab)

1. `video --iso`: new step [4/5] `USBDEVFS_SETINTERFACE` MI_01 ->
   streaming alt (probe -> commit -> alt, the standard UVC order).
   Auto pick = the alt with the largest ISO IN bandwidth (alt 2 =
   2412 B/microframe); `--alt N` overrides (alt 1 = 812 B);
   `--no-set-alt` keeps the old capture-mirroring mode as the A/B
   control (expect the ENOENT there).
2. Real teardown: DISCARDURB all in-flight URBs + reap drain
   (<= 200 ms), SET_INTERFACE back to alt 0 (ISO bandwidth freed),
   then RELEASEINTERFACE.
3. SUBMITURB/re-submit failures print a decoded hint; new pure
   `usbfs::submiturb_hint` + `ERR_ENOENT`/`ERR_ENOSPC` consts
   (unit-tested).
4. Docs: LINUX_TESTS T9 rewritten (bench result + the new default flow
   + alternates); DRIVER_STATUS turn-43 status line + P2.5b row + the
   stale FLASH_CONFIRMED line fixed; README turn-43 status entry;
   PROTOCOL turn-43 section. Counts 105 -> 106.
5. The sandbox was recycled (the old clone under work/ was gone):
   fresh clone; `work/rust/env43.sh` - the old CARGO_HOME registry
   trips a cargo "path source" internal error, the fresh cargo43 home
   fixes it.

TESTS: 106 green in both feature states; examples warning-free in
both states; nightly rustfmt clean.

## Next (the T9 rerun, same command)

    cargo run --features usb --example video -- --iso --cmos --frames 10

- PASS expected: [4/5] alt 2 selected, [5/5] payloads + frames ~8 fps,
  614400 B frames, channel A contrast.
- SET_INTERFACE ENOSPC: retry with `--alt 1`.
- If not one ISO byte moves: the next A/B is alt BEFORE commit; then
  the ep 0x03/0x83 bulk path (never observed on the wire).
