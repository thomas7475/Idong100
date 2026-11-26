# idong100 USB capture analysis - turn 28 - 2026-11-25

Source: idong_capture.zip -> USBPcap1.pcap (230,939 B, 3,260 USBPcap records,
108 s, recorded 25.11.2026 15:38-15:40 on T-PC, USBPcap 1.5.4.0, bus 4 = USBPcap1).
File is a classic little-endian pcap, linktype 249 (USBPcap). The kit's own
check_report.txt said "not a USBPcap .pcap file (wrong magic number)" - that was a
bug in the kit checker, not in the file (see turn-28 fix).

The user reported: "the capture has 2 cycles of plugging un plugging".

## 1. What the capture contains

Two complete, identical sensor boot cycles on bus 4 (device 2):

| Phase | Records | Traffic |
| --- | --- | --- |
| Cycle 1: enumerate 0471:2600 | 6-12 | GET_DESCRIPTOR device+config, SET_ADDRESS, SET_CONFIG |
| Cycle 1: MCU download | 26-1385 | 5 control frames + 335 data blocks (21,411 B) on ep 0x02/0x82 |
| Cycle 1: re-enumerate 0471:2500 | 1388-1398 | full descriptors (361-B config, 5 interfaces) |
| Cycle 1: vendor runtime | 1400-1635 | 0x701 image requests, 0x400-0x404, 0x300-0x303 config, 0x500, 0x800, 0xd00, 0xf04 |
| Cycle 2: identical repeat | 1650-3259 | same sequence, byte for byte |

Both cycles are byte-identical (OUT volumes 21,465 B and 21,465 B; the download
blocks and all runtime frames match), which is an excellent consistency check.

## 2. The wire protocol, decoded from this capture

All sensor traffic is on bulk OUT 0x02 and bulk IN 0x82, framed as
`7F 55 cmd_lo cmd_hi len_lo len_hi body...` (little-endian, no trailing CRC).
Replies have the same command number and bit 0x01 set in the reply flag field.

### Boot-loader phase (device is 0471:2600)

| Command | Name (from xeyezHal.dll analysis + this capture) | Body OUT | Body IN |
| --- | --- | --- | --- |
| 0x0800 | GetMcuVersion | - | ASCII version + padding, e.g. `2600\x00\x00\x00\x00` (8 B) |
| 0x0101 | download begin | size u16 = 0x53A3 (21,411 = exact byte count of the download) | 0x0002 |
| (data) | the image | 334 x 64 B + 1 x 35 B raw blocks, no per-block framing | sensor echoes every block back on 0x82, byte-identical (334/334 verified) |
| 0x0102 | download verify | 0x8000 + size | 0x0004 |
| 0x0103 | checksum/finalize | fe2a c9bb ae11 d5c2 (4 u16, not a standard CRC16 of the image; consistent with an 8051-side computed digest) | 0x0008 |
| 0x0104 | run | - | 0x0000 |

**Verified**: the 21,411 downloaded bytes are byte-identical to
`app_m8051_2611.hex` loaded at 0x8000 from the vendor installer
(xeyez_pc_v2.6.0.6). `app_m8051_2500.hex` does not match. The download is
therefore fully reproducible offline, and the echo on ep 0x82 gives a natural
read-back verification path for a driver.

After 0x0104 the device drops off the bus and re-enumerates as 0471:2500
(composite, class ef/02/01, bcdDevice 3.001, 5 interfaces, endpoints 0x02/0x82
bulk 512 B + 0x03/0x83 bulk 512 B + 0x85 IN iso 812/4900 B alt 1/2 + 0x84 int).

### Runtime phase (device is 0471:2500)

| Command | Meaning (observed use) | OUT body | IN body |
| --- | --- | --- | --- |
| 0x0701 | request data frame | 4 B: `03 28 00 05` / `03 22 00 05` / `01 00 00 79` | raw (not framed!): `2613\xcc`, then 64-B image-looking payloads `7261d643a11ad743...`, and `0000T\xe4`-style status words |
| 0x0400/0x0401/0x0402/0x0404 | exposure/gain config pairs | `00 38`, `00 18`, `00 38 06`, `00 18 38` | 0x00 each |
| 0x0300 | config begin | - | 0x00 |
| 0x0301 | write config entry | 8 B: `02 0a 20 10 28 10 06 00` (six entries, then a 6-B tail) | 0x00 |
| 0x0302 | read back config entry | same 8-B key | the same 8 B value (read-back verified in capture) |
| 0x0303 | config end | - | 0x00 |
| 0x0500 | reset/flush | - | 0x00 |
| 0x0800 | get version | - | ASCII `2619\x00...` (MCU app version 2619) |
| 0x0d00 | poll/status | - | 0x00 |
| 0x0f04 | poll/status | - | 0x00 |

Control transfers to ep 0 with bmRequestType 0xA1 (vendor, interface): req 0x82
(read register/TLV `02 00 0c 00 01 00 07 00 02 00 07 00 0c 00 03 0c 00`), req
0x84 (write `01 00 07 00 0c 00`), req 0x83/0x86/0x87 return full descriptors
(device or config) - the vendor host's USBVideo-style probing.

## 3. What this capture proves and what is still missing

Proven on the real hardware:
- the boot-loader handshake (0x800 -> 0101/0102/0103/0104) and the block-echo
  download mechanism, with a byte-exact match to the vendor hex file;
- the full runtime command set on 0x02/0x82 as used by the vendor host during
  the first ~30 s after boot;
- the 2500 descriptors (all 5 interfaces and all endpoints with real maxpacket
  sizes: 0x02/0x82/0x03/0x83 bulk 512, 0x85 iso 812 and 4900, 0x84 int 32);
- config write + verified read-back (0x301/0x302) - a driver can trust this path.

Still missing (the vendor program stopped at the 0x0701/0x0d00/0x0f04 polling
stage; it never opened the camera interfaces):
- the DSP upload (ens_code.ldr, 54,420 B in the installer; the DLL analysis
  expected `50 55 7F 7F` PU / `4F 44 7F 7F` OD frames - none in this capture);
- the FPGA image upload (xeyez_v20/v21.hex, ~26 KB each; A_a_v4.4_BG0313.bit
  235,930 B);
- any traffic on the isochronous endpoint 0x85 or 0x84 - the actual camera
  frame stream, which is the core input for a camera driver.

## 4. Verdict

One more capture scene is needed before a full driver can be finished; the
bootloader part of the driver can be implemented now:

1. **Needed - full vendor session**: start USBPcap first, then plug the sensor
   AND let the vendor program run until the camera image actually shows on
   screen (not just the first 30 s). The missing DSP/FPGA upload and the
   isochronous stream only happen once the vendor UI proceeds past its init
   stage. 60-120 s of the program showing live camera is enough.
2. **Implementable now**: the boot-loader driver half (enumerate 2600 ->
   0x0800 -> download app_m8051_2611.hex at 0x8000 in 64-B blocks with echo
   verification -> 0x0102/0x0103/0x0104 -> wait for re-enumeration as 2500),
   the runtime framing layer (7f55 codec, reply pairing), and the config
   store (0x0300-0x0303 with read-back).

The Rust scaffold (idong100-rs-scaffold) already has the crate layout for
this: `protocol` (7f55 codec), `boot` (loader), `runtime` (commands), `winusb`
(0x02/0x82 bulk pipes).

## 5. Kit bug found and fixed in turn 28

check_capture.bat rejected this valid pcap with "not a USBPcap .pcap file
(wrong magic number)". Cause: it accepted only pcapng, or checked a swapped
magic; USBPcap 1.5.4.0 writes classic pcap (d4 c3 b2 a1 LE, linktype 249).
The checker now accepts both containers; the turn-27 false-failure mode
("cannot create the folder") is gone as well.

## 6. Files

- scripts/analyze_capture28.py - USBPcap parser (record fields, descriptors,
  tokens, per-endpoint stats, --hex dump)
- scripts/timeline_capture28.py - chronological event timeline + 7f55 census
- work/cap28/mcu_download.bin - the 21,411 B download extracted from the pcap
- This report: download/idong100_capture_analysis_turn28.md
