# Chat context backup - turn 39 - 2026-11-27

Project: idong100 (Toltech / i-dong "idong Sensor V2.6.0.6", USB 0471:2600
boot -> 0471:2500 running). Repo:
https://github.com/thomas7475/Idong100.git (main b765fb1 at turn 39 end,
branch project/state-turn39 = 1913d6b). Phase 2: kit unlock + Rust
driver; goal: the camera as 3D scanner.

## User message of turn 39

Repo link + fresh PAT + the FULL Linux bench run output (the first live
hardware run of the turn-38 code) + "test results, the folder structure
is still messed up all the code should be in the src folder not only
part of it, lets fix the problem then continue with the next phase,
lets add 8051 firmware upload."

Standing rules: English only; zips + chat backup in download/ every
turn; PAT never printed or stored; robust modifications only.

## What the bench run PROVED (turn-38 code, real sensor, first live run)

- cargo test: 82 green on the bench box, both suites (unit +
  integration + doc-test) - the crate builds and the pure core holds.
- T1-T5 all PASS on hardware: list (bootloader + runtime sightings),
  bringup (MI_04 claimed, version "2619", device ID
  AB460DC8BA12C6840B49CD5786C329DD, edition tag 000T, code B 2613,
  calibration 120 B), probe_bootloader ("2600"), session_full --cmos
  (register init complete: tables + chunks + end marker; poll cycle
  6/6: 0003 0004 0005 000D 0404 040F; three 0x0205 unanswered =
  expected without a stream). P2.2 + P2.4 acceptance COMPLETE.
- uvcvideo BINDS the runtime composite: /dev/video0 (capture=true,
  streaming=true, format YUYV 640x480 + 320x240, current 640x480,
  bpl 1280, sizeimage 286331153) + /dev/video1 (metadata node), both
  on usb interface "1-2:1.0". The video plane exists on Linux - the
  P2.5 stream test only failed at the sensor-node cross-match (the
  two bugs fixed this turn).
- The self-reboot cycle observed live: bootloader at addr 91/92/93 ->
  runtime 94 (bringup PASS) -> bootloader again 95/97 with runtime 96
  in between (session_full PASS at 96; one transient "Resource busy"
  right after a re-enumeration that cleared on rerun - the kernel was
  still settling the fresh composite).
- Two FAILs, both now fixed (see below): bringup's calibration parse
  ("need 121 bytes, have 120") and the video probe's empty
  "sensor usb ports: []".

## What turn 39 did

1. RESTRUCTURE (the user request - "all the code should be in the src
   folder"): examples/ -> src/examples/, tests/ -> src/tests/. ALL code
   now lives under src/ (lib + examples + integration tests, one tree
   to manage). Cargo.toml carries explicit [[example]]/[[test]] path
   targets, so EVERY command is unchanged
   (cargo run --features usb --example bringup still works). The stale
   upload_firmware stub (a TODO print) was deleted - replaced by the
   real flash example (3.). src/tests/README.md rewritten for the real
   files.

2. THE TWO LIVE-BENCH FAILS FIXED (both root-caused, both unit-tested):
   a) bringup "calibration parse: need 121 bytes, have 120" - the
      example fed SessionInfo::calibration (the 120-B PAYLOAD whose
      wire checksum read_eeprom already consumed + verified) to
      parse_calibration_record (which wants the 121-B wire record =
      payload + checksum byte). NEW calib::parse_calibration_payload
      takes exactly the 120-B payload; test-proven to agree with the
      record parser on the same bytes; bringup + flash_mcu use it.
   b) video probe "sensor 0471:2500 attached at usb port(s): []" - the
      sysfs reader parsed idVendor/idProduct as DECIMAL, but sysfs
      values are HEX ASCII ("2500" parsed as 2500, not 0x2500=9472),
      so the port list was ALWAYS empty and no /dev/video node ever
      cross-matched. NEW parse_sysfs_hex16 (tested). Plus: the stream
      test's node pick now PREFERS THE CAPTURE NODE when several nodes
      sit on the same interface (the bench: video0 capture + video1
      metadata, both "1-2:1.0") - read-only QUERYCAP, the same query
      --probe does.

3. THE P2.3 8051 FIRMWARE UPLOAD (the user request - "lets add 8051
   firmware upload") - NEW src/examples/flash_mcu.rs, the CONTROLLED
   hardware flash replay:
   - Dry run (default): verifies BOTH SHA-256 pins (raw hex against
     the turn-12 manifest pin + the flattened 21,411-B image against
     the turn-32 capture pin) and opens NO device - proven against the
     real hex from xeyez_pc_v2.6.0.6-inst.zip (raw 1fad00c1...,
     flattened 5da1565c... both match in this sandbox).
   - --confirm run: the TWO-KEY gate (FirmwareSet::verify_hex + NEW
     protocol::runtime::FlashPermit - a private-field token ONLY
     constructible via FlashPermit::from_consent(FLASH_CONSENT_PHRASE
     = "flash-8051-controlled-replay"), so no accidental call site can
     reach the sequence); waits out the 31-157-s bootloader window via
     the tested reenum core (replug hint printed; the sensor sits in
     the bootloader across replugs until something flashes it - T8 IS
     that something on a pure-Linux bench); runs the EXACT cap3
     sequence (0x0101 begin size 0x53A3, 335 raw 64-B blocks, 0x0201
     verify + FULL-image readback that ABORTS before key+launch on any
     mismatch, 0x0301 key COMMIT_KEY, 0x0401 launch); waits for the
     2500 re-enumeration; proves the runtime (version "2619" + the
     four EEPROM reads + calibration parse).
   - FLASH_CONFIRMED (the compile-time gate on flash_mcu) STAYS CLOSED
     until the first clean bench PASS of T8 - that PASS is exactly the
     acceptance test that flips it. Safety shape: the image lands in
     8051 RAM, the mask-ROM bootloader is never overwritten (a failed
     flash is always recoverable by rerunning), the EEPROM is never
     touched, and the vendor engine re-runs this very sequence on
     every bootloader landing several times an hour.

4. Tests: 87 green (was 82; +5: flash_permit_requires_the_exact_phrase,
   permitted_flash_sends_the_byte_exact_sequence,
   session_payload_parser_matches_record_parser, sysfs_hex_ids_parse,
   verify_hex_rejects_a_wrong_blob), both feature states; nightly-fmt
   clean; all 8 examples compile warning-free in both states; the
   flash_mcu dry run verified against the real vendor hex in this
   sandbox; the no-hardware FAIL paths all run clean.

5. Docs: docs/LINUX_TESTS.md got T8 (the flash step: dry-run +
   --confirm procedure, abort semantics, where to get the hex, PASS
   criteria, the FLASH_CONFIRMED flip note); BRINGUP.md points at T8;
   DRIVER_STATUS.md rewritten for turn 39 (live-PASS rows, the two
   fixes, what's missing in order); README updated (layout: all under
   src/, status: live-tested).

## Repo state (main b765fb1)

Commit state(turn 39) = 1913d6b on project/state-turn39 + the no-ff
merge into main (b765fb1), both pushed. Chat backup pushed to
chat/context (chat_context_backup_turn39.md, this file). Driver
deliverable zip: idong100_phase2_turn39_2026-11-27.zip in download/
(the single-crate tree: src/ with ALL code + docs + Cargo.toml +
README + rustfmt.toml).

## Next step for the user (the bench, after git pull)

1. `cargo test && cargo test --features usb` - expect 87 green.
2. T3 again: `cargo run --features usb --example bringup` - the
   calibration line should now PASS (baseline ~100.0 mm, focal
   ~432.3 px).
3. T6/T7 again: `cargo run --features usb --example video -- --probe`
   (expect `matches sensor: true` now) then
   `cargo run --features usb --example video -- --cmos --frames 10`
   (the capture node /dev/video0 should be picked automatically).
4. T8 the flash: extract app_m8051_2611.hex from the vendor installer
   (xeyez_pc_v2.6.0.6-inst.zip -> xeyez_engine/Chs/), then
   `cargo run --features usb --example flash_mcu -- --hex <path>`
   (dry run) and `... --confirm` (the flash itself; replug the sensor
   first to land it in the bootloader - the example also waits out
   the 31-157-s window). Send the outputs: one clean T8 PASS flips
   FLASH_CONFIRMED.
5. Still open for the 3D goal: a capture with a RUNNING GAME (marker
   records / EP 0x03/0x83) - kit 37 makes the lobby+game runnable on
   Windows.
