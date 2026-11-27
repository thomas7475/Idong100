# Turn 50 findings: `A_a_v4.4_BG0313.bit` IS the missing bring-up step

Date: 2026-11-28 (turn 50). Trigger: the T13 bench (user, this turn)
failed at the cold machine's state 1 - `ens startup-begin failed
(vendor -2: Rx DEVICE ID Msg Err / illegal)` - while steps [1/6]..[3/6]
(enumerate, identity, full session init) all passed. The user asked to
research that failure AND the role of `A_a_v4.4_BG0313.bit` in the
vendor software.

## 0. The verdict up front

**The T13 failure is our driver never configuring the FPGA.**
`A_a_v4.4_BG0313.bit` is the Xilinx Spartan-3A bitstream of the sensor
board, and the vendor software downloads it to the FPGA on EVERY
device open (the configuration is volatile), over the same 7F 55
session plane our driver already speaks. The DSP sits behind that
FPGA: without the download the record plane (EP 0x83/0x03) is dead -
the probe stays silent and state 1's DEVICE-ID read times out (the
vendor -2). The turn-32 conclusion "no DSP/FPGA upload exists on the
wire" was a false negative: every capture started AFTER
`McuManager25::connect` had already run the download.

## 1. Where the vendor names the file

`xEyeUsb.ini` [Config] (identical bytes, sha 6e1d73d2.., in v2.5.0.7
AND v2.6.0.6 - `research/analysis/reports/all_extracted_sha256.tsv`):

```ini
#Bit文件
BitFile=A_a_v4.4_BG0313.bit
```

The same ini section carries `NeedHandleMatch` (the ENS state-4 key).
`xEyeUSB.dll` reads it in the usb-manager init (@RVA 0x50e0, the
`GetPrivateProfileStringA("Config", "BitFile", "", ..., ".\xEyeUsb.ini")`
@0x5172) and hands it straight to the HAL:
`xeyezHal_SetFpgaFile(name)` (thunk 0x13964 -> IAT 0x153ac). The other
`BitFile` string @0x15f74 is the duplicated .rdata block referenced by
the second manager init @0x6500 (push @0x659f).

The bit file itself ships byte-identical in both installer payloads
(sha256 75df0a38..., 235,930 B): Xilinx BIT format, design
`work_top.ncd;HW_TIMEOUT=FALSE;UserID=0xFFFFFFFF`, part **3s400aft256**
(Spartan-3A XC3S400A, FT256), build 2010/04/06 10:33:41, 233,598-B
configuration payload. The turn-34 FINDINGS already quoted that header
without knowing the consumer.

## 2. The bring-up chain (who downloads it, when)

xEyeUSB.dll `McuManager25::connect` (VA 0x10006500, the turn-33
decode) - now with the StartFlow internals filled in:

```
GetPrivateProfileStringA("Config","BitFile", ... xEyeUsb.ini)
-> xeyezHal_SetFpgaFile(name)            sets [device+8] = name
-> xeyezHal_SetLensBuffer(...)
-> xeyezHal_SetEnChipIDVerify(0)
-> xeyezHal_StartFlow(0)                 == THE OPEN SEQUENCE @0x13b70
-> xeyezHal_GetMcuVersion -> setMcuVersion
-> xeyezHal_UserLensParaRead(...)
-> _InitBlobUpload() = xeyezHal_GetBlobBegin(0)  (0x57c0 -> thunk
   0x13970 -> IAT 0x153f4; starts the DATA flow "GET_CAL_INFO_START"
   and registers the StereoCameraBridge listener - NOT the FPGA)
```

`xeyezHal_StartFlow` (@0x16300, dispatches on the platform word
@0x100821f4; type 2 would be a no-op for the FPGA) runs the open
state machine @0x13b70 for type 1. Its steps, in order (artifact
`hal_openseq_full_0x13b70.txt`):

| step | gate | call | log name |
|---|---|---|---|
| init | dirty 0x100204c4/c8 | 0x103c0 | "Init StartFlow" |
| gate | SetFpgaFile never called -> ERROR -1006 | - | "SetFpgaFile" |
| cmos idx | dirty 0x100204c0 | 0xbcc0(4) | "SetCmosIndex" |
| cmos tbl | dirty 0x100204c4 | 0xbc60(0,0) | "SetCmosTable" |
| fpga tbl | dirty 0x100204c8 | 0xbc00(0,0) | "SetFpgaTable" |
| board hook | dirty 0x100204cc | (lens ptr) | - |
| board | dirty 0x100204d0 | 0xbb90(0) | "SetBoard" |
| trans mode | dirty 0x100204d4 | 0xbb50(0) | "SetTransMode" |
| - | platform != 1 | 0x13060 | the 8051 flash update (observed on the wire) |
| **3** | **ALWAYS** | **DownLoadFpga_impl 0x10040** | (below) |
| 4 | platform != 1 && [dev+0x41c50] | 0x118b0 | "ChipIDVerify" x4 |
| 5 | always | 0xfd90 | "CMOS_CFG_START/0/1_WRITE_REG/0/1_READ_REG/END" |
| 6 | always | 0xfc70 | "FPGA_CFG_START/FPGA_WRITE_REG/FPGA_CFG_END" |
| 7 | platform != 1 && [dev+0x41c4c] | 0x139f0 | "LensRead"/"LensToCfgParam" |
| 8 | platform != 1 && [dev+0x41c4c] | 0xfa70 | "FPGA_BULK_START/Led_WR_TABLE/END; Led_RD_TABLE; FPGA_BULK_Verify" |
| 9 | - | return 1 | done |

The setters mark the dirty words (SetFpgaFile sets 0x100204bc, which
the gate REQUIRES nonzero - the download cannot run without the file
name); the flags clear after the run. **Step 3 has no dirty gate - the
FPGA download runs on every StartFlow.** Our unit is platform != 1
(the flash-update step it gates ran in every capture), so
`SendFpgaCode` takes the bit-reversal path too.

## 3. The wire format (decoded this turn)

`DownLoadFpga` (@0x14ba0 -> impl @0x10040) is a five-step named flow
on EasyUSB write pipe 0 / read pipe 1 = **EP 0x02 / EP 0x82**, the
session plane. The frame builder @0xb2f0 writes: `7f 55` magic u16 LE,
u8 cmd-lo @+2, u8 cmd-hi @+3, u8 flags @+4, u8 @+5, u16 flag word
byte-swapped @+6. `flow_open` (@0xb380) writes with a 1000 ms timeout,
reads `h` bytes with a 3000 ms timeout, and accepts only
`u16@+0 == 0x557f && u8@+4 == 1` (the same reply shape the whole 7F 55
session uses).

| step | TX | RX |
|---|---|---|
| FPGA_DOWNLOAD_START (@0x1005a) | `7f 55 02 00 00 00 00 00` | 8 B, ack 1 |
| FPGA_DOWNLOAD_QUERY_CFG (@0x10094) | `7f 55 02 01 00 00 00 00` | 8 B, ack 1 (bytes 5..8 kept raw - likely the idcode) |
| SendFpgaCode announce (@0xd732) | `7f 55 02 02 00 00 00 04` + size BIG-ENDIAN u32 (12 B) | 8 B, ack 1 |
| the file | the WHOLE file bit-reversed, RAW <=64 KiB bulk writes (3000 ms each), no replies | - |
| FPGA_DOWNLOAD_END (@0x100f9) | `7f 55 02 03 00 00 00 00` | 8 B, ack 1 |
| FPGA_DOWNLOAD_DONE (@0x10130) | `7f 55 02 01 00 00 00 01` | 8 B, ack 1 |

`SendFpgaCode` (@0xd570): `fopen` the name (fail -> -10013), read the
whole file, **bit-reverse every byte in place** (the literal
`(b&1)<<7 | (b&2)<<5 | (b&4)<<3 | (b&8)<<1 | (b&0x10)>>1 |
(b&0x20)>>3 | (b&0x40)>>5 | (b&0x80)>>7` chain = `u8::reverse_bits`,
skipped only when the platform flag [dev+0x41c44] == 1 - not our
unit), announce `bswap32(size)`, then stream 0x10000-B chunks from
object+0xb08, logging every 100th chunk. Vendor return codes:
-10000 tx fail, -10015 short/failed reply, -10014 frame mismatch,
-10013 fopen, -10012 chunk tx fail.

## 4. Why this was invisible in the captures

The captures (turns 32-35) all began with the 2500 session already
live or mid-retry: `McuManager25::connect` had already run StartFlow
(flash + session reads) before the first frame was recorded, and the
boot re-flash sequences in the captures start at the 0x0008
version poll - the FPGA download happens right after the flash step
inside the SAME connect, before any capture caught it (the run1
pcap re-checked this turn contains zero bulk-out bytes - the sensor
was not even present). Turn 32's "no DSP/FPGA upload exists on the
wire" over-generalized from that blind spot.

## 5. The new usbCom pre-loop decode (bonus, @0x114c0)

Re-disassembling around the reset strings turn 49's FINDINGS missed
(`Tx Reset DSP Cmd err` @0x18668 etc.) turned up the function right
before usbCom (@0x11770): a **pre-loop** that waits for the record
stream - 64-B reads at 500 ms, up to 100 bad reads ("Dynamic Match
Rcv pkt err"/"exit for err"), expecting OD 0xF0 announces, reading the
0x280 record, ACKing the first TWO records with PU type 7, then on the
THIRD record sending **PU type 0x7FF** and expecting **OD type 0x7EE**
("Get DSP Reset ACK OK"), up to 6 tries - before usbCom proper starts
(classifying.dll load, SetLensParam/SetStaticPoint_Param/
ClassifyAndTrack). This is the hot-side counterpart of the turn-49
probe/finish decode; pinned for the record-plane bring-up, not
implemented this turn (the cold machine comes first either way).

## 6. Driver deliverables of this turn

- `src/protocol/fpga.rs`: `fpga_step_frame` / `fpga_send_announce` /
  `fpga_bit_reverse` (unit-pinned to the literal vendor chain),
  `FpgaDownload` (the five-step flow over `Transport`, vendor codes
  cited in every error, the cause kept in the message), 
  `FpgaDownloadReport` (raw QUERY_CFG reply, size, chunks).
- `src/tests/fpga_replay.rs` (8 tests): the byte-exact happy path,
  64 KiB chunking, the vendor-chain bit-reversal pin over all 256
  byte values + the involution, silence -> -10015 with cause, wrong
  ack -> -10014, short reply -> -10015, chunk-tx failure -> -10012,
  empty image -> -10013.
- `src/examples/records.rs`: the bench is now 7 steps - [4/7] runs the
  FPGA download with `--cmos` (`--no-fpga` skips, `--fpga-only` runs
  just the download), then the probe -> hot/cold start -> record loop.
  The state-1 error of the cold machine now carries its cause
  (`Short{64,0}` = silence vs `BadType{..}` = something answered).
- 166 tests green in BOTH feature states (+8), nightly fmt clean,
  examples warning-free.

## 7. The next bench decides (T14)

```
cargo run --features usb --example records -- --cmos --frames 10
```

- [4/7] should now print the QUERY_CFG reply and PASS. If the FPGA
  download fails at START: the FPGA side is fresh-dead (fine, that is
  the normal cold case) - the failure to watch for is a REPLY mismatch,
  which would falsify the frame decode.
- [5/7] the probe may STILL be silent (the DSP also needs its
  ens_code.ldr upload - the [6/7] cold machine) or may come up if the
  FPGA was the only missing piece on this unit.
- PASS for the turn: [4/7] PASS + the cold machine reaching state 2
  (the code-load announce accepted) would prove the DSP answers once
  the FPGA is up. A still-silent state 1 after a clean [4/7] moves the
  question to the FPGA register tables (steps 5/6/8 - CMOS_CFG/
  FPGA_CFG/LED tables, not yet implemented) or the exact DSP-release
  timing.
- If [4/7] PASSes but the plane stays dead: implement steps 5/6/8
  (the CMOS_CFG/FPGA_CFG/LED table flows - the ini tables
  `Led_Scale_Fpga_v26.txt` + the 30fps `*Fpga*` files are the
  payloads) as the next decode target.
