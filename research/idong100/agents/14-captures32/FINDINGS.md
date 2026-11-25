# Turn 32: the three full-session USBPcap captures (cap2/cap3/cap4)

Date: 2026-11-26. Input: idong_capture2.zip, idong_capture3.zip,
idong_capture4.zip (uploaded 2026-11-25; kit v31, sessions 120 s /
120 s / 300 s, vendor running, sensor replugged during capture).

## What the captures contain

| Capture | Records | Span | Device timeline |
| --- | --- | --- | --- |
| cap2 | 3,434 | 116 s | 2500 (attached) -> 2600 @ 6.8 -> flash @ 11.4 -> 2500 @ 17.9 -> 2600 @ 46.7 (no flash) -> 2600 @ 69.4 -> flash @ 73.8 -> 2500 @ 80.0 |
| cap3 | 3,068 | 116 s | 2600 @ 8.5 -> flash @ 14.0 -> 2500 @ 20.6 |
| cap4 | 5,866 | 301 s | 2600 @ 14.9 -> flash @ 19.6 -> 2500 @ 25.9 |

Each "flash" is the complete vendor boot: 0x0008 "2600", 0x0101 begin,
335 raw 64-B blocks, 0x0201 verify + full image echo, 0x0301 key,
0x0401 launch, ~4 s, re-enumeration as 2500. The 2500 session then runs
the register-init + polling cycle until the capture stops.

The kit's check_report.txt said ERROR for all 12 files ("could not read
the first 24 bytes (ADODB/VBArray)") - the reader bug, not bad files.
Fixed in tools/capture/check_capture.bat (text-stream reader, no
VBArray). The new gate returns PASS for all three captures.

## The wire protocol, corrected and completed

Frame: `7F 55 <cmd u16 LE> <flags> <00> <body>`; H2D body
`<00> <alen> <args BE>`; D2H body `<00> <code> <data>`; flags 01 =
reply, 02 = table write. One frame = one 64-B bulk transfer. Raw
(unframed) 64-B records carry the download, the echo and the EEPROM
data. **Zero PU/OD frames (50 55 7f 7f / 4f 44 7f 7f) in ~12,000
records: the DLL's PU/OD model never happens on this wire.**

### Boot (0471:2600, the 8051 bootloader)

| Step | Command (LE) | Body / reply | Evidence (cap3) |
| --- | --- | --- | --- |
| 1 | 0x0008 | `-> "2600\0\0\0\0"` | rec 21/24, t=14.01 |
| 2 | 0x0101 | `<00 02 53 A3>` -> `00 02` (size BE = 21,411) | rec 25/28 |
| 3 | raw | 335 x 64-B blocks (last 35 B) | rec 29-698 |
| 4 | 0x0201 | `<00 04 80 00 53 A3>` -> `00 04`, then 335 raw echo blocks (byte-identical) | rec 699/702/704 |
| 5 | 0x0301 | `<00 08 FE 2A C9 BB AE 11 D5 C2>` -> `00 08` (constant key, 3 captures + both cap2 cycles) | rec 1373/1376 |
| 6 | 0x0401 | `<00 00>` -> `00 00`; ~4 s later the device re-enumerates as 0471:2500 | rec 1377/1380 |

### Session (0471:2500, the running app)

| Command (LE) | Meaning | Evidence |
| --- | --- | --- |
| 0x0008 | version -> "2619" | rec 1577/1580 |
| 0x0107 `<off BE> <len BE>` | EEPROM read: raw `len` bytes, last byte = sum checksum (len INCLUDES the checksum) | (0x0100,121)=calibration, (0x0322,5)="000T"+E4, (0x0328,5)="2613"+CC |
| 0x0104 / 0x0204 (flags 02) | CMOS register tables, 56 B + 24 B; 0x02xx echoes the table back | rec 1455-1478 |
| 0x0103 / 0x0203 | 8-B register chunks (x6); 0x0203 reply re-reads (last byte may differ: 00 -> 02) | rec 1511-1558 |
| 0x0303 | end of the config phase | rec 1559 |
| 0x0003, 0x0004, 0x0005, 0x000d, 0x0404, 0x040f | polls, empty body, `00 00` reply (0x0004 takes ~1.2 s; 0x040f only in cap2's last 15 s) | throughout |

The session cycle repeats every ~7-10 s: version, code-B read,
calibration read, poll, table writes, chunk writes, end. cap4 (300 s)
holds 27 cycles. Nothing else happens - **no frame/marker/image stream
in any capture** (no traffic on ep 0x85/0x84, no marker records on the
bulk pipe; the largest D2H raw record is the 121-B calibration).

### Turn-28 naming reconciled

Turn 28 wrote commands in wire byte order (big-endian). Same protocol,
new little-endian reading: 0102<->0x0201, 0103<->0x0301 ("finalize
four u16" = the COMMIT_KEY write), 0104<->0x0401 ("run" = launch),
0701<->0x0107 ("request-frame" = the EEPROM read: its "version 2619+cc,
5-B status words" were the code-B reads, the "64-B image payloads" were
the echo blocks), 0301/0302/0303 <-> 0x0103/0x0203/0x0303, 0401/0402
<-> 0x0104/0x0204, 0f04<->0x040f.

## The downloaded image, identified byte-exactly

The 21,411-B (0x53A3) image is **app_m8051_2611.hex, Intel-HEX records
flattened over [0x8000, 0xD3A3), gap bytes 0x00**:

- hex records below 0x8000 (the 3-B reset vector LJMP) are not sent;
- hex gaps (the 5-B and 2-B holes between the interrupt-vector records)
  are 0x00 on the wire, not 0xFF and not the last written byte;
- result is byte-identical to the download AND the echo in all three
  captures: SHA-256
  `5da1565c6704d387081844e712b4be4d521083e21a619df7d6cb11551c2aba9c`.

Verification chain (analysis host): NSIS solid LZMA stream of
xeyez_Lobby.dat decompressed (0xDEADBEEF "NullsoftInst" header, props
5d 00 00 80 00), the hex text runs located by scan, SHA-256 matched
against the turn-12 manifest (app_m8051_2611.hex =
1fad00c1...), then the flatten rule compared against
cap3_dl_image.bin - byte-identical. Encoded in the driver as
`FirmwareSet::mcu_image()` and `tests/runtime_replay.rs` replays the
whole flash phase frame-for-frame.

`app_m8051_2500.hex` (61,031 B, ffb8b962...) is present in the same
installer payload but is NOT downloaded in any capture - its role is
unknown (open item). Two further ~26 KB hex blobs (SHA 3f82b8cc... /
0298a4d8...) exist in the Lobby payload at offsets 39,645,142 /
39,671,059 and are also never sent.

## What this settles

1. The 7F 55 protocol model is complete for boot and idle-session
   states; the driver's `protocol::runtime` implements it with golden
   tests (48 tests, all green).
2. No DSP/FPGA upload exists on the wire: ens_code.ldr and
   A_a_v4.4_BG0313.bit never appear. The 0471:2500 composite's DSP and
   FPGA run firmware that is not host-loaded (factory-programmed, or
   loaded by the 8051 app from a source not on the USB wire).
3. The vendor app, patched with the turn-31 DLL fix, runs its complete
   sensor session (flash, calibration, register init, polls) - but never
   starts a camera or marker stream. The "live camera image" stage was
   not reached in any of the three sessions.

## Open items (next captures)

1. **The stream**: what makes the app start sending/receiving frames?
   Candidates: a menu action in the vendor UI (the R/P/S/I/G/X tray
   menu), the "exercise" flow, or the ep 0x85 iso interface (alt 1/2
   after SET_INTERFACE). A capture with the camera view actually
   showing is the missing piece - run the vendor app to the live image
   FIRST, then start USBPcap for ~30 s while the image is visible.
2. Marker records (0x3BC B on the bulk pipe, xEyeClass GetADataFromUSB):
   never seen; presumably part of the same missing stream state.
3. The 0x0107 EEPROM read only exposes offsets the vendor reads; a full
   EEPROM image can come from the T24C16 dump already on file.
4. CMOS register tables: byte-carried; per-register semantics (BG0313
   sensor map) still to decode.
5. app_m8051_2500.hex role.
