# Turn 49 findings: the ens_code.ldr upload state machine, decoded statically

Date: 2026-11-28 (turn 49). Sources: the v2.6.0.6 engine DLLs
re-extracted from `upload/v2.6.0.6.zip` -> `xeyez_Lobby.dat` (NSIS;
7zz 24.08), byte-verified against the turn-12 manifest
(`research/analysis/reports/all_extracted_sha256.tsv`):

| file | sha256 (verified == manifest) |
|---|---|
| xEyeUSB.dll | f018386028dde03e699bcbf7485ec42ac44c33e613acdffadbfcd0f1988f1173 |
| xEyeClass.dll | 09fe223a3e776de488783e5414abfce59618b2b31b5473618e98f5e7c00d9c34 |
| classifying.dll | 0b8d4ebaa5fa10101d459becf212a49af74cba73f39b554c1a4c68461f1e5deb |
| xeyezHal.dll | 87db59b40473104cff1e33481269c8ff769ed38c9e9d792217113b2bf89192c0 |
| crc.dll | 4997270573c32928a2745c3a5a36d478ea1fbbb631fa4797415f321dadcbd8b7 |

Disassembled with `research/scripts/dis46.py` (pefile + capstone).
Artifacts: `artifacts/*.txt`. The CRC model: `research/scripts/crc49_model.py`.

The question this turn closed: the T12 bench (user, turn 48/49)
returned **VERDICT B** - no type-0xF0 stream on EP 0x83 after the
session init - so the runtime `ens_code.ldr` upload is required. The
old plan made a kit-37 game capture the only way to pin its wire
bytes. That capture is retired: the upload, the whole startup machine
around it, and its checksum are all decoded here from the vendor
stack itself.

## 1. The driver: `0xf790` (the 8-state machine)

```
state = 1 (u16); retries = 0; ens = 0x1001c788 (the state block)
loop:
  idx = state - 1
  if idx > 7: state = 1                        (defensive)
  else: jmp table @0x1000f940[idx]              (8 entries, verified from .data)
state handlers (the result feeds the shared tail):
  idx0/state1 @0xf7fa: result = 0xfa30()
  idx1/state2 @0xf804: result = 0xfae0("ens_code.ldr")   <- [ebp+8], the filename arg
  idx2/state3 @0xf815: result = 3                        <- the SKIP state
  idx3/state4 @0xf81e: result = 0xfe20()
  idx4/state5 @0xf828: result = 0xffb0()
  idx5/state6 @0xf832: result = 0x10130()
  idx6/state7 @0xf83c: result = 0x104e0(ens)
  idx7/state8 @0xf84d: return ens                        <- the exit
tail:
  result > 0:  if result < 5: [0x1001cbb0] = 1            (in progress)
              elif result == 5: [0x1001cbb0] = 2 + setDeviceID(0x1001cb88)
              retries = 0; state += 1; 0xf960(result)     (the milestone logger)
  result <= 0: retries < 1 -> retries++ (RETRY the same state)
              else -> retries = 0; state = 1; [0x1001cbb0] = -1  (full restart)
  Sleep(0x32 = 50 ms)
```

So: each state gets 2 attempts, a double failure restarts the whole
machine with the error mark, and the progress flag at `0x1001cbb0`
runs -1 error / 1..2 in-progress / 3-4 link states / 5 pairing done.
`0xf960(result)` logs the milestone strings (GBK, @0x10017c8c..):
启动开始 / 代码加载 / …… / 手柄配对选择 / ID及版本 / 手柄配对 /
镜头参数 / 启动流程结束 / 出错Err - the state table below.

## 2. The wire helpers

- `0x105f0(buf, type, len)` - TX: `[buf+0] = 0x7f7f5550`,
  `[buf+4] = type (u16)`, then `WriteData(pipe 3, buf, len, 3000)`.
  Pipe 3 = EP 0x03 OUT (EasyUSB ordinal 2 = WriteData; pipe
  calibration from the 7F 55 session). Returns 1 / -1.
- `0x10590(buf, expect, len, timeout)` - RX: `ReadData(pipe 2, buf,
  len, timeout)` (EP 0x83 IN, ordinal 1). Requires n == len, OD magic
  `0x7f7f444f`, `u16@+4 == expect`. Returns 1 / -1 read / -2 frame.
- The unwritten frame bytes are STACK GARBAGE in the vendor (only
  magic + type are set; no memset in any builder). The Rust port
  zero-fills - deterministic, and every disassembled check only
  looks at magic + type.

## 3. The states

**State 1 - startup begin (`0xfa30`)**: `0x105f0(buf, 1, 0x40)` ("Tx
DSP Start" - PU type 1) -> `0x10590(ens, 1, 0x40, 3000)` (OD type 1
= the DEVICE ID frame). Logs: 'Tx DSP Start Err' / 'Rx DEVICE ID Msg
Err' / 'Rx DEVICE ID Msg illegal!'. Returns 1.

**State 2 - code-load (`0xfae0`)**, THE upload:
```
fp = fopen("ens_code.ldr", "rb"); fail -> -4
size = ftell(fp); nchunks = ceil(size / 64)
init_crc32_table()                      (crc.dll, no args)
rewind; fread(framebuf, 1, size, fp)
crc = GenerateCRC32(buf, size)           (crc.dll)

announce: [f+0] = PU magic, [f+4] = 2 (u16), [f+8] = size (u32),
          [f+0xC] = crc (u32)
WriteData(pipe 3, f, 64, 3000)          fail -> -5 ('Tx packet count & crc!')
ReadData(pipe 2, ens, 64, 3000)         fail -> -6 ('Failed to receive: Ready to load program!')
   check OD magic + u16@+4 == 2         bad -> -7
rewind
for i in 0..nchunks:
    fread(f, 1, 0x40, fp)               # RAW file bytes, no framing
    WriteData(pipe 3, f, 64, 6000)      fail -> -8 ('send program code error')
fclose
ReadData(pipe 2, ens, 64, 6000)         fail -> -9 ('Failed to receive: CRC result!')
   check OD magic + u16@+4 == 3         bad -> -10
   check u16@ens+8 == 0xF               bad -> -11 ('Error: crc result is not correct!')
return 2
```
The last partial chunk (the shipped ldr: 54,420 B = 851 chunks, the
last with 20 valid bytes) is sent as a full 64-B write; the vendor's
frame buffer holds stale bytes there, the device knows `size` from
the announce. The Rust port zero-pads the tail.

**State 3**: returns 3, no I/O (the log string is a bare "……").

**State 4 - pairing select (`0xfe20`)**: poll `ReadData(pipe 2, ens,
64, 300)` up to 11 times (100 ms apart, ~4 s); a wrong-type frame is
the immediate -13. Then `need = GetPrivateProfileInt("Config",
"NeedHandleMatch", 0, ".\xEyeUsb.ini")` -> TX PU type 0x25 (match on)
/ 0x26 (off) via `0x105f0` -> `0x10590(ens, 5, 0x40, 3000)` (OD type
5 again). The shipped `xEyeUsb.ini` has `NeedHandleMatch = 1`.
[0x1001c294] stores the match mode (state 6 reads it). Returns 4.

**State 5 - ID & version (`0xffb0`)**: TX PU type **0x8001** ("Tx ID
& Ver Resend") -> `0x10590(ens, 1, 0x40, 3000)` (OD type 1). Decodes:
`DSP_VER = %s` (8 B @ ens+0x28), the 32-B DEVICE ID @ ens+8 ->
`0x1001cb88`, printed "%02x " x32. Returns 5 - the milestone that
sets [0x1001cbb0] = 2 and triggers `setDeviceID(0x1001cb88)`.

**State 6 - the TSOL pairing loop (`0x10130`)**: skipped (returns 6)
when [0x1001c294] == 0. Otherwise loops `ReadData(pipe 2, ens, 64,
3000)`:
- OD type 0xFFF (the keep-alive) -> TX PU type 0xFFE (the "IDLE
  msg"); continue.
- OD type 4, u16@+8:
  - 1 (HANDLE FOUND, UID1 = u32@+0xC, UID2 = u32@+0x10) -> TX PU 4,
    word 0x11 @+8; if UID1 == -1 and UID2 == -1 -> [0x1001cbb0] = 3
    (error) + notify(2), else 4 + notify(3); continue.
  - 0xF (NOT FOUND) -> TX PU 4, word 0xFF; continue.
  - 2 (FOUND TIMEOUT) -> TX PU 4, word 3; both UID -1 -> [0x1001cbb0]
    = -2, elif not 3/4 -> 5; continue.
  - 8 (FOUND OVER) -> TX PU 4, word 9; [0x1001cbb0] -> 5 when -2/2;
    **EXIT LOOP** ('启动流程结束' log) -> returns 6.
  - anything else -> -25 ('ERROR: Handle status').
- wrong magic/type -> -20.

**State 7 - lens parameters (`0x104e0`)**: `0x10590(ens, 6, 0x80,
3000)` - OD type 6, **128 B** (magic + type + 0x78 B of lens params @
+8) -> TX PU type 6 (the ACK). Returns 7.

**State 8 / the hand-off**: `0xf790` returns the ens block pointer;
`usbCom` (@0x118ad) then Sleep(1000), notify(5), memcpy(0x78 B,
ens+8) -> `SetLensParam(params, float)` - the lens block state 7 (or
the hot start's OD 0x10) delivered.

## 4. The hot start (`0x10640` + `0x10900`)

The turn-46 "ENS probe" is a HOT-START DETECTOR, not just a probe:
```
ack[64] = {PU magic, type 7}                       (built at entry)
loop:
  ret = 0x10590(ens, 0xF0, 0x40, 100)              # expect a record announce
  ret == -1 (no frame): tries >= 10 -> return 0     # COLD START NEEDED
      'Hot Start no rcv'; tries++; Sleep(1)
  ret == -2 (wrong frame): 'Hot Start chk err'; reset; Sleep(1)
  ret == 1:
      n = ReadData(pipe 2, ens, 0x280, 1000)       # the 640-B record
      n != 0x280 -> 'Hot Start packet err'; retry
      trailer != OD magic -> retry
      if success_count < 2: WriteData(pipe 3, ack, 64, 300); success_count++
      else: 0x10900(ens)                            # the finish
```
`0x10900(ens)` (the finish):
```
0x105f0(buf, 0x10, 0x40)         'Tx HotStart Cmd'    PU type 0x10
0x10590(ens, 0x10, 0x80, 500)    'Rx HotStart Lens Ack' OD type 0x10, 128 B (lens)
0x105f0(buf, 0x9001, 0x40)       'Tx HotStart DID'    PU type 0x9001
0x10590(buf, 0x9001, 0x40, 3000) 'Rx HotStart DID'    OD type 0x9001 (DID)
  DSP_VER 8 B @+0x28, ID 32 B @+8 -> 0x1001cb88 -> setDeviceID
```
So: THREE 0xF0+record cycles (the first two ACKed with `{PU magic,
type 7}` - note: NOT the all-zero first ACK of the steady-state
loop), then the lens + DID exchange. The probe succeeding means the
DSP was already running from a previous app session.

## 5. The checksum: `crc.dll!GenerateCRC32`

`init_crc32_table` @0x1010 builds the table from `reverse8(i) << 24`
through 8 MSB-first rounds of poly `0x04C11DB7`, then bit-reverses
the result - numerically the standard reflected `0xEDB88320` table.
But `GenerateCRC32` @0x1110 updates hybrid-style:

```
crc = 0
for b in data: crc = ((crc << 8) | b) ^ table[crc >> 24]
return crc                                   # no final xor
```

NOT a catalog CRC: "123456789" -> **0x4D3A139E** (MPEG-2 would be
0x0376E6E7, zlib 0xCBF43926); short inputs degenerate ("a" ->
0x61). The shipped ldr: size 54,420, **CRC 0x63EAA8CA**, 851 chunks.
The literal model (Python): `research/scripts/crc49_model.py`; the
Rust port `protocol::ens::crc32_ens` is test-pinned to these
vectors and the shipped-ldr values.

## 6. Driver deliverables of this turn

- `src/protocol/ens.rs`: `RecordIo` (the EP 0x83/0x03 wire seam),
  `StartupOutcome` (device ID + DSP version + lens params),
  `crc32_ens`, `pu_frame`/`pu_frame_word`/`pu_codeload_announce`,
  `codeload_chunks` (raw 64-B slices, zero-padded tail), `OdFrame`,
  `ColdStart` (the 8 states, vendor return codes cited in the
  errors), `HotStart` (probe + 3 cycles + finish).
- `src/protocol/mod.rs`: the ENS constants (types, words, budgets,
  timeouts), each cited.
- `MockTransport` + `UsbTransport` `RecordIo` impls; the replay test
  `src/tests/ens_replay.rs` (5 tests: byte-exact cold start with the
  pairing loop, match-off, the CRC verdict failure -> vendor -11,
  the full hot start, probe silence).
- `src/examples/records.rs`: the T13 flow - probe -> hot start /
  cold start (the shipped, SHA-256-pinned ldr; `--no-ldr` keeps the
  T12 read-only behavior, `--no-match` = NeedHandleMatch 0) -> the
  record handshake loop.
- 158 tests green in both feature states (+9), nightly fmt clean.

## 7. What the next bench decides (T13)

```
cargo run --features usb --example records -- --cmos --frames 10
```

- The probe should stay silent (VERDICT B after a fresh flash) ->
  the cold start runs; every milestone prints; the DID/version/lens
  lines decode; then the record loop (0xF0 -> 640 B -> type-7 ACK).
- PASS = the marker plane is LIVE end-to-end without any vendor
  software; the blob layout decode is next.
- If the OD type-2 go-ahead never answers mid-upload: full power
  cycle, rerun (the loader upload is volatile RAM state).
