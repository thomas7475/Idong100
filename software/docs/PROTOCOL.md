# PROTOCOL.md - wire facts (P0.1 deliverable)

Every row cites its source (DLL + RVA where known). Constants here are
encoded verbatim into `src/protocol/mod.rs` - the Rust module is
the executable form of this table. Paths are relative to `software/`
(the crate; turn 40).

## USB identity

| Item | Value | Source |
|---|---|---|
| bootloader | 0471:2600, iface 0, EP 0x02 OUT / 0x82 IN, maxpacket 64 | xeyez.inf, both driver generations |
| runtime | 0471:2500 composite, vendor MI_04 (UVC/UAC on 0-3) | xeyez.inf, DriverConfig.ini |
| legacy | 0471:2100 / 0668 / 0666 (identification only) | xeyez.inf |
| bootloader maxpacket quirk | 64 B (full-speed value at high speed - spec violation, accepted) | live lsusb, CONTEXT.md |
| runtime read chunk | 4096 B (sys) / 16 KiB accumulation | xeyez.sys `MaximumTransferSize`, EasyUSB.dll |
| write retries | 3 attempts, 100 ms apart | EasyUSB.dll |

## Boot framing (PU/OD magic)

| Item | Value | Source |
|---|---|---|
| H2D magic | 0x7f7f5550, LE bytes `50 55 7f 7f` ("PU") | xEyeUSB.dll |
| D2H magic | 0x7f7f444f, LE bytes `4f 44 7f 7f` ("OD") | xEyeUSB.dll |
| frame header | `[u32 magic LE][u16 type LE][payload]`, no length field | xEyeUSB.dll |
| packet size | 64 B type-3 | `_InitBlobUpload` |
| timeouts | 3 s ready / 6 s packet / 300 ms x 10 dev-ID / 1 s reset | xEyeUSB.dll (GetTickCount deltas) |

### Boot sequence (`_InitBlobUpload`, xEyeUSB.dll)

1. TX DSP-start -> RX type 2 "Ready to load program!" (3 s)
2. TX type 2: packet count + CRC of the blob
3. TX type 3 x 64-byte packets (6 s each)
4. RX type 3: CRC result - word 0xF at frame offset +8 aborts
   (boot_protocol @0x1000fdee)
5. RX type 2: device ID / DSP version (300 ms poll x <= 10)
6. "Get DSP Reset ACK OK" -> TX type 7 (1 s)

## Runtime framing (7F 55)

| Item | Value | Source |
|---|---|---|
| layout | 7F 55; cmd@[2]; b2-b4@[3..5]; u16 arg@[6..7] BYTE-SWAPPED; payload <= 56 B | xeyezHal builder @0x10003010 |
| cmd 3 | register write, 8 B chunks | SetCmosTable loop |
| cmd 5 | blob data read | GetBlobData |
| cmd 6 | GET_IMG_START (status 1) | GetImageBegin |
| cmd 15 | classifier/reg write | chunk loop @0x10003a50 |
| timeouts | 1 s TX / 5 s RX | xeyezHal engine @0x100030a0 |
| keep-alive | RX 0xFFF -> TX 0xFFE within 1 s | xEyeClass reader |
| link status | type 4; subcode 1/0xF; reply word 0x11 @+0x06 | tx_builder @0x1001028e/0x10010327 |

## Marker record (xEye3Ddata) - HOST-side structure (turn 46)

0x3BC B minimum (host queue element 0x3F0): frame counter u32 @ +0
(overlays slot 0; HOST-assigned by the usbCom worker, one per record);
10 x 88 B slots with **x f64 @ slot+0x08, y f64 @ slot+0x10, z f64 @
slot+0x18, quality i32 @ slot+0x24 (>= 50 tracked)** - pinned turn 46
by `TransUSB2Pipe` @RVA 0x25e0 (the packer) and the best-marker loop
of `GetADataFromUSB` @RVA 0x20f0 (smallest z wins - the NEAREST marker
is the active one). Mode flag u32 @ +0x3B8 (== 1 -> pipe-writer skip);
pipe form 0x208 B. **The x/y/z values are HOST-computed**: the wire
record is raw blob data for `classifying.dll!ClassifyAndTrack` (see
"Runtime record plane" below).

| Item | Value |
|---|---|
| slots | 10 x 88 B at +0x000..+0x370 |
| x/y/z | f64 @ slot+0x08 / +0x10 / +0x18 (mm) |
| trailer | 72 B (+0x370..+0x3B8) |
| mode flag | u32 @ +0x3B8 |
| quality gate | i32 @ slot+0x24 >= 50 |
| frame rate | 50 fps (xEyeFrame=50, USBComFPS=35 transport) |
| quality window | RingSize=200, thresholds Green=20/Yellow=35/Red=35 (xEyeUsb.ini) |

## EEPROM T24C16A (2048 B, verified dump MD5 dc02fce8ca39155135a4b0368b3f33ae)

| Offset | Record | Length | Content |
|---|---|---|---|
| 0x0000 | header tag | 16 B | `ef0123456789abcd` |
| 0x0100 | calibration | 120 B | 28 x f32 LE OpenCV: fx fy cx cy x2, dist x2, R 3x3, T 3 |
| 0x0180 | version | 16 B | "2059" |
| 0x0192 | int16 | 16 B | 386, 473, 215, 303, 142, 221, 0, 0 |
| 0x0300 | serial | 32 B | `AB460DC8BA12C6840B49CD5786C329DD` |
| 0x0322 | code A | 4 B | "000T" |
| 0x0328 | code B | 4 B | "2613" |
| +0x400 | mirror | - | every record duplicated |

Record framing: `[payload][checksum = sum(payload) & 0xFF][0xFF]`.
Known-good values: |T| = 100.0000 mm (designed baseline), mean fx
432.33 px, |k1| < 0.05, R orthonormal, cx ~312-313 px (640-px frames).

## Firmware SHA-256 pins (v2.6.0.6, turn-12 manifest)

The three files ship with the crate under `firmware/` (turn 40; the
whole vendor folder, its provenance and the vendor file map:
`firmware/README.md`). Before any byte leaves the host the driver
verifies them against these hashes (`FirmwareSet::verify` - safety rule
#3; the flash path checks the hex alone with `FirmwareSet::verify_hex`):

| File | Size | SHA-256 |
|---|---|---|
| ens_code.ldr (BF527) | 54,420 | 3a19c68a33352cda9b4c9e8a476c4978e19a5b983e567cff946fe543f51a60dd |
| A_a_v4.4_BG0313.bit (Spartan-3A XC3S400A) | 235,930 | 75df0a38037e1983d270f1003c2303c25a61dcdb554d455b1f963cd4a70b4abd |
| app_m8051_2611.hex (8051) | 61,756 | 1fad00c1dca7f995e8b58d4ed6c5c080da4005f63682ad7af6288abd4d118f35 |

The hashes are the turn-12 manifest values
(`research/analysis/reports/all_extracted_sha256.tsv`); the test
`cargo test --test firmware_files` re-checks them on the shipped files.
`src/firmware.rs` locates the folder, so `flash_mcu` reads
`firmware/v2.6.0.6/app_m8051_2611.hex` without a `--hex` argument.

## Open items (P2.3 USBPcap gate)

- multi-firmware staging order (ldr/bit/hex)
- CRC variant announced in the count+CRC step (CRC16-CCITT vs CRC32,
  endianness - the current `blob_crc` is a placeholder)
- exact count/CRC payload framing
- DSP-start payload bytes (64-B scratch buffer, open item #3)
- ~~marker x/y/z offsets inside the 88-B slot (open item #4)~~ RESOLVED
  turn 46: x f64 @+0x08, y @+0x10, z @+0x18, quality @+0x24
  (`TransUSB2Pipe` @RVA 0x25e0 + the best-marker loop @0x22a8 - see
  "Runtime record plane (turn 46)")
- handle-config blob layout (open item #6)

## Turn 24: 2600 and 2500, and what a capture has to contain

### The state of the sensor

| USB ID | State | What happens there |
| --- | --- | --- |
| 0471:2600 | Boot loader of the 8051 microcontroller, after every power-up | The vendor host downloads the MCU program `app_m8051_2611.hex` |
| 0471:2500 | The MCU program runs, composite device | DSP upload `ens_code.ldr` and FPGA upload, then the normal camera traffic |

The vendor host side, read from `xeyezHal.dll` and `xEyeUSB.dll` 2.6.0.6:
`xEyeUSB.dll!InitDevice` calls `xeyezHal_StartFlow` (mode 2), that runs
`McuUpdate`: `GetMcuVersion` answers "2600", then `DownLoadHex` and
`CodeDownload_Flow` send the hex file with the `7F 55` framing. The host then
polls up to 20 times with `Sleep(1000)` for plug state 5, which is 0471:2500,
and runs `InitDevice` again on 2500 ("MCU application program has run!"):
`_InitBlobUpload` loads the DSP blob and the FPGA image. A polling thread of the
vendor program starts `InitDevice` again when the device appears again.

Consequences for the capture:

- The MCU download is on the wire only between a power-up and the first
  `InitDevice` of the vendor program. To record it, USBPcap must run before the
  sensor is plugged in.
- A sensor that is already at 0471:2500 cannot be put back to 2600 with a
  command from our side. The vendor DLL has `McuReset`, and the kit never sends
  it. Unplug and plug in again instead.
- If the vendor program is not running, the sensor stays at 2600 and nothing is
  downloaded.

### What check_capture.bat looks for

| Name | On the wire | Meaning | Gate |
| --- | --- | --- | --- |
| PU | `50 55 7F 7F` | Program upload, host to sensor, on the MCU endpoint 0x02 | at least 10 frames |
| OD | `4F 44 7F 7F` | Sensor to host during the DSP upload, endpoint 0x82 | at least 3 frames |
| RT | `7F 55` | Runtime framing of the vendor protocol | counted |
| Banner | ASCII `Ready to load program!` in an OD frame | The DSP loader is ready | reported |

Both 0471:2600 and 0471:2500 are found by their device descriptor, which
USBPcap records as a control transfer. The USB address of each is printed in
the Sensor line. When both phases run on the same address, the counts of both
are summed and the report says so.

### USBPcap file layout, as the scanner reads it

24 byte pcap header (link type 249), then per packet 16 byte record header, 27
byte USBPcap header (28 for control transfers), then the payload. Fields used:
transfer type at offset 22 (0 isochronous, 1 interrupt, 2 control, 3 bulk),
direction bit in the info byte at offset 16, device address at offset 19,
endpoint at offset 21. A bulk or interrupt transfer to the sensor carries its
payload in the submit packet, from the sensor in the complete packet. A control
transfer is a SETUP packet with stage 0 and a COMPLETE packet with stage 3.

## Turn 28 update: the real wire protocol, read off a real capture

The phase-2 capture (see SOLVE_DIALOG turn 28) corrected the model built from
the DLL analysis. All traffic is bulk on ep 0x02 OUT / 0x82 IN; framing is
`7F 55 cmd_lo cmd_hi len_lo len_hi body` (little-endian; replies repeat the
command with a 0x01 flag byte). The MCU download is NOT 50 55 7f 7f framed:
it is raw 64-B blocks after a 0101 handshake carrying the total size, and the
sensor echoes every block back on 0x82 (verified byte-identical, 334/334).
The downloaded image is app_m8051_2611.hex at 0x8000, byte-identical.

Boot loader commands (0471:2600): 0x0800 GetMcuVersion -> "2600"; 0x0101
begin (size); data blocks; 0x0102 verify (0x8000+size); 0x0103 finalize
(four u16); 0x0104 run -> re-enumerates as 0471:2500.

Runtime commands (0471:2500): 0x0701 request-frame (raw replies: version
"2619"+cc, 64-B image payloads, 5-B status words); 0x0400/0x0401/0x0402/
0x0404 exposure/gain (00 38 / 00 18 pairs); 0x0300 begin, 0x0301 write entry,
0x0302 read back (verified to return the written value), 0x0303 end;
0x0500 flush; 0x0800 version -> "2619"; 0x0d00 and 0x0f04 polls.

The 2500 configuration descriptor: 5 interfaces; endpoints bulk 0x02/0x82/
0x03/0x83 (maxpkt 512), isochronous IN 0x85 (alt 1: 812 B, alt 2: raw
wMaxPacketSize 0x1324 = base 804 x 3 = 2412 B per microframe - the exact
capture packet stride; the earlier "4900" here was the raw field read as
decimal), interrupt IN 0x84 (32 B). Vendor control requests on ep 0 with
bmRequestType 0xA1: req 0x82/0x84 register TLV access, req 0x83/0x86/0x87
return descriptors.

Still unobserved on the wire: the DSP upload, the FPGA upload, and any
traffic on ep 0x85/0x84 (the camera stream). These need the "full vendor
session" capture of turn 28's next step.
## Turn 32: the observed protocol, complete (cap2/cap3/cap4)

Three full-session captures settle it. **The PU/OD model above never
happens on the wire: 0 PU/OD frames in ~12,000 records. No DSP upload,
no FPGA upload - ens_code.ldr and A_a_v4.4_BG0313.bit are never sent.**
The `7F 55` framing carries everything:

- Frame: `7F 55 <cmd u16 LE> <flags> <00> <body>` (one 64-B bulk
  transfer). H2D body `<00> <alen> <args BE>`, flags 0x02 on table
  writes. D2H body `<00> <code> <data>`, flags 0x01.
- Raw 64-B records (no magic): download blocks, image echo, EEPROM data.
- Boot on 0471:2600: `0x0008->"2600"`, `0x0101 begin(size BE)`, 335 raw
  blocks, `0x0201 verify(0x8000,size) -> 00 04` + full echo readback,
  `0x0301 key FE2AC9BBAE11D5C2`, `0x0401 launch` -> re-enumerates as
  0471:2500 (~4 s).
- Session on 0471:2500: `0x0008->"2619"`, `0x0107(off,len)` EEPROM read
  (raw `len` bytes, last = sum checksum; the calibration record
  (0x0100,121), "000T" (0x0322), "2613" (0x0328)), `0x0104/0x0204`
  CMOS register tables (56 B + 24 B), `0x0103/0x0203` 8-B chunks (x6),
  `0x0303` end, polls `0x0003/0x0004/0x0005/0x000d/0x0404/0x040f`,
  cycle every ~7-10 s. No image/marker stream in any capture.
- Turn-28 command IDs were big-endian readings of the same bytes:
  0102<->0x0201, 0103<->0x0301, 0104 ("run")<->0x0401, 0701
  ("request-frame")<->0x0107, 0f04<->0x040f.

The downloaded image: app_m8051_2611.hex flattened over [0x8000,0xD3A3),
gap bytes 0x00 - byte-identical to the wire (download and echo), SHA-256
`5da1565c6704d387081844e712b4be4d521083e21a619df7d6cb11551c2aba9c`.
Implemented: `src/protocol/runtime.rs` (RuntimeSession,
golden-replayed in `src/tests/runtime_replay.rs`), `FirmwareSet::mcu_image()`.
Full evidence: `research/idong100/agents/14-captures32/FINDINGS.md`.

Open: what starts the camera/marker stream (the vendor app never entered
that state in 535 s of capture); CMOS register semantics.
The role of app_m8051_2500.hex is settled (turn 40, vendor file map
`firmware/v2.6.0.6/config/xeyezHal.ini`): it is the `HexFileName` of
section `[Version2.5]` (`LastVersion=2700`) and app_m8051_2611.hex the
one of `[Version2.6]` - the MCU images of two sensor generations. The
"2500" in that name is not the USB id: the 0471:2500 runtime device of
the test unit runs the 2611 image (cap2/cap3, byte-identical).

## Turn 33: why the vendor app never left "connecting" (cap7)

Capture 7 (300 s, kit v31, patched DLL) plus a full static analysis of
the engine closed the last open question. The vendor app RUNS its whole
sensor session - but its own connect is rejected at the very last check,
so the UI state machine stays at "detecting sensor" forever.

**The engine's connect chain (xEyeUSB.dll 2.6.0.6, McuManager25):**

1. `AutoPreDetectDevice` -> `CheckSpecialDevice(0, id)` per hardware
   ID (table @0x10007d98: `0471:0666`, `0668`, `2100`, `2600`,
   `2500&MI_04`): enumerate + CreateFile each. Return 5/4/3/2 = found
   (device class), 1 = the 0666 unit is present ("hardware mismatch"
   -> state-freeze flag), 0 = nothing found.
2. InitDevice -> `connect`: `xeyezHal_SetFpgaFile` (BitFile from
   .\xEyeUsb.ini), `SetLensBuffer`, `StartFlow` (the 8051 flash +
   session init = everything observed on the wire), session init,
   `GetMcuVersion` ("2619"), edition check.
3. **The gate (new in v2.6.0.6, absent from v2.5.0.7):** the version
   loop (3 attempts) calls `xeyezHal_DeviceVerRead(4)` = **`0x0107`
   EEPROM read at 0x0322, 5 B** ("000T"+0xE4 on this unit) and compares
   the FOURTH byte against the constant `"DSHY"` at 0x10016034 - 'D',
   'S', 'H' or 0x00 pass. **This unit's 'T' fails all three
   attempts** -> close -> retry every 10 s -> the wire shows the tag
   read 3x at the end of every cycle, and the screen never leaves
   "正在连接idong感应器". The sensor also spontaneously re-enumerates
   as the 0471:2600 bootloader every 31-157 s (3x in cap7: t=209.7,
   t=255.7; also seen in cap2), the engine re-flashes it each time.
4. After the gate passes (kit idong100_engine33, 1 byte: DLL offset
   0x16036 'H'->'T'), the state machine needs the device-ID check:
   message 5 -> `check_device_id` -> `xeyezHal_DeviceIDRead(32)` =
   `0x0107` EEPROM read at **0x0300, 33 B** (32 ASCII + sum checksum,
   backup at 0x0700). This unit: "AB460DC8BA12C6840B49CD5786C329DD"
   +0x50, intact in the T24C16 dump. Then stage -> "running" (state 7).

**The 0471:2500 composite topology (cap7 descriptors, 361-B config):**

- if0 Video Control (0E.01.00), if1 Video Streaming (0E.02.00; alt 0
  no ep, alt 1 ep 0x85 IN, alt 2 ep 0x85 IN) - the app opens the
  camera via DirectShow/UVC right after each re-enumeration (75
  class-interface GET_INFO/MIN/MAX/RES/DEF queries, wValue CS 02..08),
  but never selects an alt > 0 in any capture so far;
- if2 Audio Control (01.01.00), if3 Audio Streaming (01.02.00, ep
  0x84 IN iso 32 B);
- if4 vendor class (00.00.00): ep 0x02 OUT / 0x82 IN = the 7F 55
  pipes, ep 0x03 OUT / 0x83 IN (512 B, unused so far).
- The bootloader (0471:2600) is a single ff.00.00 interface with just
  ep 0x02/0x82 (32-B config). The launch (0x0401) re-enumerates
  2600 -> 2500.

**"FPGA blobs" decoded:** the 30fps/*.dat "FPGA" files are NOT
bitstreams - they are text register tables ("0x06,0x02\r\n0x07,0x81...")
sent as the observed 0x0103/0x0203 8-B chunks and 0x0104/0x0204 CMOS
tables. The FPGA/DSP bitstreams are never host-uploaded (factory
images). Implemented in the driver: `read_device_id()`,
`edition_tag_accepted()`, `EDITION_LETTERS`, constants in
`src/protocol/runtime.rs`; golden test `turn33_engine_gate_reads`.

Open: the stream itself (ep 0x85 alt>0 / ep 0x03/0x83, marker records)
- reachable once the engine-unlock kit runs the app into the lobby.

## Turn 34: the third gate (license), the driver files, the direct run

Full findings: `research/idong100/agents/16-turn34/FINDINGS.md`.
Kit: `idong100_engine34` (patch/verify/restore/check_unlock/run_direct/
check_wire + reference DLLs; md5s in its MANIFEST.txt).

**The connect gate chain, complete:**

1. GATE 1 - version tag "DSHY" vs this unit's "000T" (turn 33, DLL
   offset 0x16036 H->T).
2. GATE 2 - device-ID read 0107 @0x0300 (intact on this unit).
3. GATE 3 - **the license check** (`xEyeRight.dll`): `LocalRight` +
   `NetRight` (HTTPS `POST /outer/device.action` to `us.i-dong.com`,
   `deviceID=...&key=...`) -> `MakeFinalResult` -> final result 0x24
   (1 = authorized) + window message 0x7E8. The server is dead, so a
   stock install can NEVER be authorized; the engine host
   (`xeyez.exe`) then refuses to go fully live
   (`ShowDeviceIDInvalidWindow` = the "sensor not authorized" screen).
   Turn-34 fix: `MakeFinalResult` file offset 0x927 (VA 0x10001527),
   the 0x2D-byte guard block -> `xor edx,edx; mov [ecx+0x24],1; jmp`
   into the vendor's own notify tail (relocation-safe: the vendor
   `call [USER32!SendMessageA]` keeps its original offset 0x961 and
   relocation entry).

**The host stack (new facts):** `xeyez.exe` = the engine host
(window class `xEyezEngine2009`, started by `xEyeService.exe` via
`xEyezSvStartXeyez.exe` or by `xeyezMonitor.exe` or directly);
camera clients use named pipes (`xEyeEngine.dll` = the pipe stub:
`InitxEyeEngine`/`GetxEyePipeData`); the lobby uses TCP 40000
(`portlobby`, conf/version.xml); `xEyeThirdLens.exe` = the standalone
camera viewer (imports the pipe stub + `MiddleCameraCalib.dll`).
The tray states (xEyeSysTrayTip.dll): unknown/stop/need-reinsert/
**connecting**/handling/matching/pause/lost/hub-lost/video-excep/
audio-excep/link-excep/noise/**running**; "connecting" = stage 4 with
the device-ID flag still 0 (stage() reports 1).

**The kernel driver (from the user's install zips):** `xeyez.sys`
2.5.4.9 = the WinDDK bulkusb sample ported ("BF527 Bulk USB" - the
sensor DSP is an Analog Devices Blackfin BF527, `ens_code.ldr` is
its LDR image); x86 + x64; service `XEYEZ`; device
`\Device\xeyezUsbDriverDev` -> apps open `\\.\xeyezUsbDriverDev`
and use plain ReadFile/WriteFile (NO custom IOCTLs; EasyUSB.dll is
the user-mode wrapper); INF binds 2600 / 2500&MI_04 / 2100 / 0668 /
0666; reads `SOFTWARE\Toltech\xeyez_engine\Location` in kernel
mode. `A_a_v4.4_BG0313.bit` = Xilinx bitfile, design "work_top.ncd".

**Driver impact:** for a user-mode Rust driver, two options -
(a) keep the vendor kernel driver and talk to
`\\.\xeyezUsbDriverDev` with plain reads/writes (documented model
above), or (b) replace it with WinUSB/libusb and drive the bulk
endpoints 0x02/0x82 directly. The 0107/0300 device-ID read +
checksum and the edition letters are already implemented in
`src/protocol/runtime.rs`.

## Video plane (turn 35, the xEyeThirdLens capture)

Input: `idong_capture.7z` run1 (120 s, 16,293 packets) - the first
capture reaching the video phase. Sensor timeline: 2500 session with
0x0205 polls -> self-reboot to 2600 @ 17.8 s -> the host re-runs the
FULL boot sequence (0008/0101 + 335 blocks/0201/0301/0401) @ 35.2 s ->
2500 re-enum @ 42.1 s -> UVC negotiation @ 51.8 s -> isochronous video
on EP 0x85 @ 52.0 s to the end (82.8 MB, 1358 ISO records, 63960 B each).

| Item | Value | Source |
|---|---|---|
| UVC probe/commit | fmt 1, frame 1, interval 333333 (30 fps), dwMaxVideoFrameSize 614400 = 640x480x2, commit payload 962 | SET_CUR cs 01/02 if 1, capture 51.83-51.88 s |
| streaming EP | 0x85 IN, isochronous; NO SET_INTERFACE alt>0 observed | capture 52.02 s+ |
| UVC payload header | 12 B: `[len=0x0c][flags 0x8c/0x8d][pts LE u32][scr 6 B]`; flags = EOH|SCR|PTS(+FID) | every payload |
| payload size | <= 2400 B image data; ISO record = up to 27 payloads, 2412 B apart (last one short, 1236 B) | record walk |
| PTS | constant within a frame; +1,920,000 per frame; TICK RATE MEASURED 48 MHz (run 13: the summed per-session PTS span 615,360,000 ticks over 297 deltas vs the measured 12.8 s active wall span - 0.2% agreement at 48 MHz, impossible at 15.36/44.1 MHz; 1,920,000 ticks = 40.0 ms = 25 fps nominal; skips quantize at 480,000 ticks = 10 ms: run 13's deltas were only 1.92M x246 / 2.4M x8 / 2.88M x43 = 4/5/6 quanta) | run-13 bench log + `research/scripts/analyze_run13_pts.py` |
| raster | 1280-B rows = 640 px x 2 byte-interleaved channels; row-to-row corr 0.998 (A) / 0.97 (B); planes mutually uncorrelated | correlation analysis |
| channels | A (even bytes) = the scene image (a room + hand is visible when rendered); B (odd bytes) = a second, low-contrast channel, range 104-154 peaked at 128 | rendered PNGs |
| stream poll | cmd 0x0205 on the vendor interface at ~30 Hz; 64-B reply: code 04 = new frame with 4-B LE frame counter, code 00 = empty; trailer `FF FF FF 00 1F 00 05/04 00` | 2963 OUT/2961 IN @ 0-13.3 s |

The 0x0205 poll and the video raster are implemented and pinned in
`src/protocol/video.rs` + `src/protocol/runtime.rs`
(`poll_stream`, `VideoAssembler`, `ProbeCommit`, `StreamPollState`),
golden-tested against capture bytes in `src/tests/video_golden.rs`.

**Bootloader recovery confirmed (second full observation):** the
sensor self-rebooted mid-session (17.8 s) and the host re-ran the
complete turn-32 boot (0008 "2600" -> 0101 `<00 02 53 A3>` -> 335 raw
64-B blocks -> 0201 verify + echo -> 0301 key -> 0401 launch). The
engine re-programs the flash on EVERY bootloader landing (observed in
cap2 x2, cap3, cap4, and now cap8) - it never tries to boot the
existing image first. The engine recovers automatically.

**Driver impact:** the video plane is a standard UVC interface - on
Linux the video flows through V4L2 (or usbfs ISO) while the vendor
bulk interface (ep 0x02/0x82) carries the control/poll protocol. The
Rust driver's `VideoSource` seam models this: one `read_payload` =
one ISO packet (header + data); `VideoAssembler` closes frames on PTS
advances exactly like the vendor's DirectShow filter.


## Turn 42: the turn-41 bench settles P2.3/P2.4, T7 FAILS at STREAMON -> P2.5b

The full bench ladder run (turn 41, the user's Linux box):

- T1 (100 tests), T2-T5: PASS - the same state as turn 39, plus
  `session_full --cmos` completing register init + 6/6 polls.
- **T8: PASS TWICE** - both controlled flash replays readback-matched
  21,411 B, launched, re-enumerated as 0471:2500, version "2619",
  identity + calibration intact (device ID
  AB460DC8BA12C6840B49CD5786C329DD, baseline 100.0 mm, focal 432.3 px).
  `FLASH_CONFIRMED` = true since this turn: the ungated `flash_mcu` is
  open; the consent-gated example remains the interactive path.
- T6: PASS inside the stream run - the sensor's capture node is
  /dev/video0 (uvcvideo, "UVC Camera (0471:2500)"), 640x480 YUYV
  negotiated.
- **T7: FAIL at VIDIOC_STREAMON with EIO** - the documented alt-setting
  decision point. uvcvideo cannot stream this firmware.

The response (turn 42, this turn): the P2.5b raw usbfs-ISO backend.

- `src/transport/usbfs.rs` (pure, 5 tests): the configuration
  descriptor walker, the wMaxPacketSize decode (bits 10..0 base, bits
  12..11 additional transactions - 0x1324 -> 2412 B/microframe,
  exactly the capture stride), and the UVC control builders
  (SET_CUR/GET_CUR probe/commit, GET_DESCRIPTOR).
- `src/examples/video.rs --iso`: DISCONNECT_CLAIM of the UVC
  VideoStreaming interface away from uvcvideo (reversible), the
  observed probe/commit SET_CURs (golden bodies), then ISO URBs on
  EP 0x85 with ASAP scheduling - a 4-URB x 8-microframe ring, reaped
  and re-submitted. No SET_INTERFACE at all, mirroring the capture
  (the vendor app never raised the alt; video flowed with the
  interface at alt 0).
- The UAPI mirror is ABI-pinned (urb header 56 B, iso packet desc 12 B,
  ioctl request numbers gcc-verified against
  /usr/include/linux/usbdevice_fs.h).

Doc fixes shipped with it: the `sha256sum -c` command in
LINUX_TESTS.md (the SUMS paths are relative to `firmware/`, so the
check must run from `software/firmware` - the turn-41 bench hit
exactly that), and the alt-2 "4900 B" misread above (0x1324 read as
decimal; the true capacity is 2412 B/microframe).

The next hardware question T9 answers: does the firmware stream
whenever ISO URBs are posted on EP 0x85 (the hypothesis) or does the
alt-setting matter after all (then a SET_INTERFACE alt 1/2 precedes
the URBs). A FAIL plus dmesg decides the fallback (ep 0x03/0x83 bulk,
never observed on the wire).

## Turn 43: the T9 first run answers the alt question - ENOENT at alt 0

The first live T9 run (`video --iso --cmos --frames 10`, turn 42's
capture-mirroring shape) proved the negative: the run took MI_01 from
uvcvideo, sent the observed probe/commit SET_CURs (both PASS), and
then every SUBMITURB on EP 0x85 failed with `No such file or
directory (os error 2)`.

The errno decoded: ENOENT from the host controller, not the
/dev/bus/usb node. Linux arms an isochronous endpoint only in the
SELECTED alternate setting of its interface - the VS interface stood
at alt 0 (the zero-bandwidth alt that declares no endpoints), so the
host controller had no transfer ring for EP 0x85 and refused the ISO
URB. The vendor capture's no-SET_INTERFACE flow cannot be replayed
through usbfs; the Windows stack evidently armed its ISO pipe outside
the captured segment. The dmesg the bench sent along confirms nothing
else moved (the uvcvideo entity-registration failure is the separate,
known reason V4L2 cannot drive this firmware; the bulk-maxpacket
warnings are vendor-descriptor noise).

The fix (turn 43): `USBDEVFS_SETINTERFACE` MI_01 -> streaming alt
(probe -> commit -> alt, the standard UVC order; default alt 2 =
2412 B/microframe, `--alt 1` = 812) before the ring, and on teardown
DISCARDURB + drain, alt 0 back, then RELEASEINTERFACE. The pure half
(`src/transport/usbfs.rs`) gained `submiturb_hint` (ENOENT and ENOSPC
decoded for the bench); 106 tests green. The next T9 question: does
the firmware stream under probe -> commit -> alt, or does it need
the alt BEFORE the commit (the next A/B), or does nothing move at all
(then EP 0x03/0x83, never observed on the wire).

## Turn 44: the T9 rerun PASSes - EOF carries the frame tail

The T9 rerun (`video --iso --cmos --frames 10`, the turn-43 fix) is
the first full raw-ISO PASS: 1281 payloads, 0 packet errors, frames
end to end. Three wire facts it decided:

1. **probe -> commit -> SET_INTERFACE(alt 2) -> ring is the working
   shape.** The capture-mirroring no-alt flow stays the documented
   dead end (`--no-set-alt`).

2. **The firmware flags the final payload of every frame with EOF,
   and that payload carries the frame tail.** The bench closed every
   frame as TWO frames under one PTS: a 612000-B body (255 full
   payloads) + a 2400-B tail - 256 x 2400 = exactly 614400 B, the
   committed `dwMaxVideoFrameSize`. That was the assembler's old
   close-before-append EOF order leaking the last payload into the
   next frame, not a device quirk. Turn 44 fixed
   `VideoAssembler::push_payload`: an EOF payload completes the
   frame WITH its data (an error payload still drops its own bytes;
   the PTS-advance rule and the golden capture shapes are unchanged
   - 108 tests green). Reruns now close one 614400-B frame per PTS.

3. **The raw-path cadence:** the PTS advanced 5 times in the window,
   in steps of exactly 3, 4.5, 4.5 and 6 frame periods
   (`PTS_PER_FRAME` = 1,920,000 units; the golden window read them as
   ~1.8 frames/s because the pre-turn-48 constant assumed 15.36 MHz
   - see the run-13 resolution below).

   **Turn 47 decoded the turn-46 run-11 cadence:** the constant
   ~4.0-4.5-period deltas of run 11 were STILL host starvation, not
   device pacing - `--rate` parsed its flag but never suppressed the
   per-frame work (two PNG encodes, two plane-stats passes, a
   console line per frame, all on the reap thread), and a 4-URB ring
   is only 4 ms of ISO buffer. 86.8% of the device's frames never
   arrived (1675 gap-counted "drops", ~1000 of which were actually
   reboot gaps the old accounting wrongly included). The turn-47 loop
   moves ALL per-frame work to a worker thread, deepens the ring to
   `--ring N` (default 16 URBs = 16 ms), drops the in-ring 0x0205
   polls (each unanswered poll blocked the reap loop ~3 s), and
   prints the wall-vs-PTS rate table. A clean rerun (dt 1.0 flat,
   intra-session drops 0) then measures the REAL device rate and the
   implied PTS tick (15.36 vs 48 MHz) in one shot; the run-11 bench
   could not tell them apart.

   **Run 13 settled it (turn 48): `--iso --cmos --rate 300
   --recover`, 300 frames across 3 sessions.** The undisturbed first
   session ran 110/110 consecutive PTS deltas EXACTLY 1,920,000
   (40.0 ms at 48 MHz = 25 fps, matching the ~25 fps uvcvideo
   measured and the ~23.3-25 fps of the worker-thread ISO loop);
   the disturbed sessions' deltas were only 1.92M x246 total /
   2.4M x8 / 2.88M x43 - whole multiples of a 480,000-tick (10 ms)
   skip quantum, never fractional. The clincher: the summed
   per-session PTS span 615,360,000 ticks over 297 deltas vs the
   measured 12.8 s active wall span - 12.820 s at 48 MHz (0.2%),
   impossible at 15.36 MHz (40.1 s) or 44.1 MHz (13.95 s). The
   15.36 MHz constant had been a golden-capture window artifact
   (546 FID transitions / 68 s read 8.0 fps; the golden video window
   is that same anomaly). `DEVICE_CLOCK_HZ` = 48,000,000 and
   `PTS_SKIP_QUANTUM` = 480,000 since turn 48; the frame period is
   40 ms and the negotiated 30 fps (interval 333333) is an upper
   bound only. Alt 2's 2412 B/microframe = 19.3 MB/s is exactly the
   headroom 25 fps x 614,400 B needs (15.36 MB/s).

The same bench day also resurrected T7: uvcvideo STREAMON - the
turn-41 EIO "dead end" - PASSED right after the T8 re-flash (10
frames, 614400 B, real image). The EIO is state-dependent, not
permanent; both observations are recorded in `docs/LINUX_TESTS.md`.
The self-reboot the bench saw after every video run (the sensor
lands on 0471:2600) is the documented 31-157 s cycle, not a teardown
bug: with `--recover` (P2.4b, turn 44) the driver now re-flashes and
resumes on every landing, exactly like the vendor engine.
## Runtime record plane (turn 46: the marker stream, decoded)

Full evidence: `research/idong100/agents/20-turn46/FINDINGS.md` (the
v2.6.0.6 engine DLLs re-extracted and disassembled: usbCom worker
@xEyeUSB 0x11770, ens subsystem @0xeb00-0x10900, TransUSB2Pipe @xEyeClass
0x25e0, ThreadProc/queues @0x1ab0/0x2c30). The "what starts the marker
stream" question is answered: it is a THIRD protocol layer - the PU/OD
framing on the second MI_04 bulk pair.

| Item | Value | Source |
|---|---|---|
| endpoints | EP 0x83 IN (records), EP 0x03 OUT (ACKs) | pipe calibration: 7F 55 = write pipe 0 / read pipe 1 = EP 0x02/0x82 (USBPcap), usbCom uses read pipe 2 / write pipe 3 |
| framing | PU/OD magic u32 @+0, u16 type @+4, 64-B command frames | usbCom loop @0x11c41-0x11c95 |
| record announcement | OD frame type 0xF0, 64 B, on EP 0x83 | loop @0x11c47 (`word@+4 == 0xf0`) |
| raw record | 640 B (0x280) on EP 0x83, OD magic dword trailer @ +0x27C | read @0x11c7b, trailer check @0x11d13 |
| ACK | 64-B PU frame type 7 on EP 0x03, payload @ +0xC, first one zeroed | write @0x11cc2, build @0x11d91 |
| ACK semantics | the ACK for record N carries the type-7 command computed from record N-1 (classifier output) | ClassifyAndTrack receives PU+0xC |
| startup probe | read 64 B from EP 0x83, 100 ms x <= 10 tries, expect OD type 0xF0 | @0x10640, called @0x11886 |
| if probe fails | the `ens_code.ldr` upload state machine runs on EP 0x03/0x83 first (8 states incl. a skip, 64/128-B PU/OD frames) - DECODED turn 49, see "ENS startup" | @0xf790; the app never reached usbCom (license gates) |
| record payload | RAW blob data (the classifier input; `SVM_Class_ball/head_*.dat`), status byte u8 @ +0x25C | f660 transform @0xf660 |
| status byte | bit 7 gates flags = (bit4/5/3 == 0) -> +0x378/+0x37C/+0x380; bit 0 -> mode word @ +0x3B0 | f660 @0xf660 |
| classification | `classifying.dll!ClassifyAndTrack` runs ON THE HOST: 640-B record -> 10 x 0x48 slots -> remapped into the 10 x 0x58 xEye3Ddata slots | remap loop @0x11de2 |
| pacing | USBComFPS = 35 (xEyeUsb.ini), Sleep-calculated period | loop tail @0x12379 |
| frame counter | u32 @ xEye3Ddata+0 is HOST-assigned (one per record) | @0x12249 |

Implemented: `protocol::ens` (StatusFrame / WireRecord / Type7Command,
all constants above), `transport::usb::{read_bulk_ep, write_bulk_ep}`,
the bench example `records.rs`
(`cargo run --features usb --example records -- --cmos`).

## ENS startup (turn 49: the record-plane bring-up, decoded)

The T12 bench (user, turn 48/49) returned **VERDICT B** - silence on EP
0x83 after the session init - so the runtime `ens_code.ldr` upload is
required. Its wire format, previously "the last unpinned bytes", is
now fully decoded STATICALLY from the vendor stack itself (no game
capture needed): the 7z-extracted v2.6.0.6 engine DLLs, byte-verified
against the turn-12 manifest, disassembled with
`research/scripts/dis46.py`. Full walkthrough:
`research/idong100/agents/21-turn49/FINDINGS.md`.

`usbCom` brings the record plane up one of two ways:

**Hot start** (probe @0x10640 + finish @0x10900) - the stream is
already alive (a previous app session loaded the DSP):

| step | exchange | source |
|---|---|---|
| probe | expect OD 0xF0 (10 x 100 ms), read the 640-B record, trailer check | 0x10640 |
| cycle x3 | 0xF0 + record; the first two cycles ACKed with `{PU magic, type 7}` 64-B frames on EP 0x03 | 0x10640 @0x1089a |
| finish 1 | TX PU type 0x10 (the hot-start cmd) | 0x10900 |
| finish 2 | RX OD type 0x10, 128 B = the lens frame (0x78 B params @ +8) | 0x10900 |
| finish 3 | TX PU type 0x9001, RX OD type 0x9001 = DID (32 B @ +8, 8 B @ +0x28) -> `setDeviceID` | 0x10900 |

**Cold start** (the 8-state machine @0xf790, driver loop with jmp
table @0xf940, per-state retry, 50 ms pacing, progress flag at
0x1001cbb0: -1 error / 1..5 milestones). The state handlers' own GBK
log strings name every state:

| state | handler | milestone | exchange |
|---|---|---|---|
| 1 | 0xfa30 | 启动开始 startup-begin | TX PU type 1 ("Tx DSP Start"), 3 s -> RX OD type 1 (device ID) |
| 2 | 0xfae0 | 代码加载 code-load | TX PU type 2 {u32 size @+8, u32 CRC32 @+0xC} -> RX OD type 2 ("Ready to load program!") -> ceil(size/64) RAW 64-B chunks (6 s each, no framing) -> RX OD type 3 with u16 0xF @+8 (the CRC verdict) |
| 3 | skip | ... | returns 3, no I/O |
| 4 | 0xfe20 | 手柄配对选择 pairing select | poll RX OD type 5 (11 x 300 ms, 100 ms apart) -> TX PU type 0x25 (NeedHandleMatch=1) / 0x26 -> RX OD type 5 |
| 5 | 0xffb0 | ID及版本 ID & version | TX PU type 0x8001 -> RX OD type 1: 32-B ID @+8, 8-B version @+0x28, `setDeviceID` |
| 6 | 0x10130 | 手柄配对 pairing (only when match on) | loop: RX OD 0xFFF -> TX PU 0xFFE ("IDLE msg"); RX OD 4 with sub-word @+8: 1/2/0xF/8 -> TX PU 4 word 0x11/3/0xFF/9; sub 8 exits |
| 7 | 0x104e0 | 镜头参数 lens params | RX OD type 6, 128 B (0x78 params @ +8) -> TX PU type 6 |
| 8 | exit | 启动流程结束 finished | returns the ens block; usbCom Sleep(1000) then `SetLensParam(ens+8, 0x78)` |

Frame builders: TX `0x105f0` = {PU magic, u16 type @ +4} on pipe 3
(EP 0x03), 3 s; RX `0x10590` = pipe 2 (EP 0x83), exact length + OD
magic + one expected type (returns -1 read / -2 frame). The
code-load checksum is `crc.dll!GenerateCRC32` - a vendor variant:
the standard reflected 0xEDB88320 table, but the hybrid MSB-style
update `crc = ((crc << 8) | byte) ^ table[crc >> 24]`, init 0, no
final XOR ("123456789" -> 0x4D3A139E; the shipped 54,420-B ldr ->
0x63EAA8CA). Not a catalog CRC - the port in `protocol::ens::crc32_ens`
is literal and test-pinned.

Implemented (turn 49): `protocol::ens::{RecordIo, StartupOutcome,
ColdStart, HotStart, crc32_ens, pu_frame, pu_frame_word,
pu_codeload_announce, codeload_chunks, OdFrame}` + the `RecordIo`
impls (`UsbTransport` behind `usb`, `MockTransport`), the replay test
`src/tests/ens_replay.rs` (byte-exact cold start, match-off, the CRC
verdict failure, the full hot start, probe silence), and the T13 bench
flow in `records.rs`: probe -> hot start / cold start (the shipped,
SHA-256-pinned ldr) -> the record handshake loop (`--no-ldr` keeps the
old read-only T12 behavior, `--no-match` mirrors NeedHandleMatch=0).

## FPGA bring-up (turn 50: `A_a_v4.4_BG0313.bit` - the missing step, decoded)

The T13 bench failed at the cold machine's state 1 (`vendor -2: Rx
DEVICE ID Msg Err`) with steps 1-3 PASS. Root cause, decoded this turn
from xeyezHal.dll v2.6.0.6
(`research/idong100/agents/22-turn50/FINDINGS.md`, artifacts +
`research/scripts/dis50_hal.py`): **the driver never configured the
FPGA**. The vendor chain (xEyeUSB `McuManager25::connect`, turn 33):

```
GetPrivateProfileStringA("Config","BitFile", ... xEyeUsb.ini)   @0x5172
-> xeyezHal_SetFpgaFile(name)        (dispatch impl 0xbd10)
-> xeyezHal_StartFlow()              (dispatch @0x16300 -> open seq @0x13b70)
     init -> SetCmosIndex -> SetCmosTable -> SetFpgaTable -> SetBoard
     -> SetTransMode (all dirty-flag gated)
     -> the 8051 flash update (0x13060, platform != 1: observed on wire)
     -> DownLoadFpga 0x10040 - UNCONDITIONAL, every StartFlow
     -> ChipIDVerify (0x118b0) -> CMOS_CFG (0xfd90: CMOS0/1 WRITE/READ_REG)
     -> FPGA_CFG (0xfc70: FPGA_WRITE_REG) -> LensRead (0x139f0)
     -> LED tables (0xfa70: FPGA_BULK Led_WR_TABLE/Led_RD_TABLE/Verify)
-> session init -> GetMcuVersion -> license gates
-> _InitBlobUpload = xeyezHal_GetBlobBegin(0) (0x11270: starts the
   "GET_CAL_INFO_START" data flow + the StereoCameraBridge listener;
   NOT the FPGA download despite the name)
```

The bitstream: `A_a_v4.4_BG0313.bit`, 235,930 B, Xilinx BIT format,
design `work_top.ncd;HW_TIMEOUT=FALSE;UserID=0xFFFFFFFF`, part
**3s400aft256** (Spartan-3A XC3S400A FT256), built 2010/04/06. The
configuration is VOLATILE - the vendor re-downloads it on every
device open; without it the DSP behind the FPGA never answers, which
is exactly the T13 state-1 timeout. The bit file ships byte-identical
in v2.5.0.7 and v2.6.0.6 (sha256 75df0a38..., pinned in
`firmware/SHA256SUMS`).

The wire (frame builder @0xb2f0, flow runner `flow_open` @0xb380,
`DownLoadFpga` impl @0x10040, `SendFpgaCode` @0xd570; session plane
EP 0x02 write / EP 0x82 read, the SAME pair the whole 7F 55 session
uses; every reply is 8 B, magic `7f 55` + ack `01` @+4, 3 s budget;
vendor codes: -10000 tx / -10015 short reply / -10014 frame /
-10013 fopen / -10012 chunk tx):

| step | TX | note |
|---|---|---|
| FPGA_DOWNLOAD_START | `7f 55 02 00 00 00 00 00` | u16 cmd 0x0002 |
| FPGA_DOWNLOAD_QUERY_CFG | `7f 55 02 01 00 00 00 00` | cmd 0x0102; reply bytes 5..8 kept raw (idcode candidate) |
| SendFpgaCode announce | `7f 55 02 02 00 00 00 04` + size **BE** u32 (12 B) | cmd 0x0202, flag word 4 = "a 4-B field follows" |
| the file | the WHOLE file **bit-reversed** (`u8::reverse_bits`, the literal `(b&1)<<7 | ...` chain @0xd605), RAW **<=64 KiB** bulk writes, 3 s each, NO replies | the vendor reverses in place, .bit header included |
| FPGA_DOWNLOAD_END | `7f 55 02 03 00 00 00 00` | cmd 0x0302 |
| FPGA_DOWNLOAD_DONE | `7f 55 02 01 00 00 00 01` | cmd 0x0102, flag word 1 |

Platform flag [device+0x41c44] == 1 would skip the bit-reversal and
the flash-update step - this unit is platform != 1 (the flash update
RAN in every capture), so the reversal applies. The open sequence's
steps 5/6/8 (CMOS_CFG / FPGA_CFG / LED tables) are decoded but NOT
yet implemented - the candidate next step if the plane stays dead
after a clean FPGA download (payloads: the ini `*_Fpga` tables,
`Led_Scale_Fpga_v26.txt` = addr/value pairs 0x58..0x6b).

Implemented (turn 50): `protocol::fpga::{fpga_step_frame,
fpga_send_announce, fpga_bit_reverse, FpgaDownload,
FpgaDownloadReport}` over `Transport`, the replay test
`src/tests/fpga_replay.rs` (byte-exact happy path, chunking, the
vendor-chain pin over all 256 bytes, the four failure codes), and the
records bench step [4/7] (`--no-fpga` skips, `--fpga-only` runs just
the download). 166 tests green both feature states.
