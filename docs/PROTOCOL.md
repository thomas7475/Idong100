# PROTOCOL.md - wire facts (P0.1 deliverable)

Every row cites its source (DLL + RVA where known). Constants here are
encoded verbatim into `src/protocol/mod.rs` - the Rust module is
the executable form of this table.

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

## Marker record (xEye3Ddata)

0x3BC B: frame counter u32 @ +0 (overlays slot 0); 10 x 88 B slots;
quality i32 @ slot+0x24 (>= 50 tracked); gate f64 @ slot+0x18; mode
flag u32 @ +0x3B8 (== 1 -> pipe-writer skip); pipe form 0x208 B after
the RVA 0x25e0 transform. Source: xEyeClass.dll GetADataFromUSB @RVA
0x20f0.

| Item | Value |
|---|---|
| slots | 10 x 88 B at +0x000..+0x370 |
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

Firmware is never redistributed; users supply their own files, verified
by these hashes (`FirmwareSet::verify` - safety rule #3):

| File | Size | SHA-256 |
|---|---|---|
| ens_code.ldr (BF527) | 54,420 | 3a19c68a33352cda9b4c9e8a476c4978e19a5b983e567cff946fe543f51a60dd |
| A_a_v4.4_BG0313.bit (Spartan-3) | 235,930 | 75df0a38037e1983d270f1003c2303c25a61dcdb554d455b1f963cd4a70b4abd |
| app_m8051_2611.hex (8051) | 61,756 | 1fad00c1dca7f995e8b58d4ed6c5c080da4005f63682ad7af6288abd4d118f35 |

## Open items (P2.3 USBPcap gate)

- multi-firmware staging order (ldr/bit/hex)
- CRC variant announced in the count+CRC step (CRC16-CCITT vs CRC32,
  endianness - the current `blob_crc` is a placeholder)
- exact count/CRC payload framing
- DSP-start payload bytes (64-B scratch buffer, open item #3)
- marker x/y/z offsets inside the 88-B slot (open item #4 - resolve via
  xEyeClass RVA 0x25e0 disassembly)
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
0x03/0x83 (maxpkt 512), isochronous IN 0x85 (alt 1: 812 B, alt 2: 4900 B),
interrupt IN 0x84 (32 B). Vendor control requests on ep 0 with bmRequestType
0xA1: req 0x82/0x84 register TLV access, req 0x83/0x86/0x87 return
descriptors.

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
that state in 535 s of capture); the role of app_m8051_2500.hex; CMOS
register semantics.

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
| PTS | constant within a frame; +1,920,000 per frame; device clock 15.36 MHz -> 125 ms -> 8.0 fps steady state | 546 FID transitions over 68 s |
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
