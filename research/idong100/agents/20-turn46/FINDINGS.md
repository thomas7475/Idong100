# Turn 46 findings: the marker/record plane decoded from the vendor stack

Date: 2026-11-27 (turn 46). Sources: the v2.6.0.6 engine DLLs re-extracted
from `upload/v2.6.0.6.zip` -> `xeyez_Lobby.dat` (NSIS, 7z; sha256 of
xEyeClass.dll `09fe223a3e776de488783e5414abfce59618b2b31b5473618e98f5e7c00d9c34`),
disassembled with `research/scripts/dis46.py` (pefile + capstone). All
disassemblies: `artifacts/disasm46_*.txt`.

The question this turn: **what starts the marker/record stream** (missing
#2 of the DRIVER_STATUS list - "the likely trigger is a command we have
not sent or EP 0x03/0x83"). The answer is fully decoded below. Short
version: the record plane is a THIRD protocol layer - the PU/OD framing
on the second MI_04 bulk pair EP 0x83 IN / EP 0x03 OUT, driven by the
`usbCom` worker of xEyeUSB.dll - and the x/y/z marker values are
HOST-computed (the device sends raw 640-B blob records, the host-side
`classifying.dll` SVM produces the marker slots).

## 1. The host pipeline (xEyeClass.dll + xEyeUSB.dll)

`xEyeClassManager::Init` @RVA 0x1ab0 wires five in-memory queues through
`xEyeList.dll` (name, capacity, element size):

| queue | element | size | direction |
| --- | --- | --- | --- |
| `Data2Class` | xEye3Ddata records | **0x3F0 B** | USB layer -> class manager |
| `Data2USB` | commands | 0xA0 B | USB layer consumes |
| `Data2Cal` | pipe form (games) | 0x3E0 B | class manager -> named pipes |
| `Main2Class` | orders (app) | 0x184 B | app -> class manager |
| `Class2USB` | orders (USB) | 0x184 B | class manager -> USB layer |

`ThreadProc` @0x2c30 (the xEyeClass reader thread) loops: pop one
`Main2Class` order -> `GetAnOrderFromMain` (vtable+0x14); drain up to a
watermark of `Data2Class` records -> `GetADataFromUSB` (vtable+0x1c);
`CheckPipeData` (vtable+0x20); pace with `Sleep`. `GetAnOrderFromMain`
@0x1e50 handles `SetPipeName` / `SetLobbyPipeName` /
`Set2DPipeNameLobby` / `SetPipeKey` / `xEyeRightState` (named-pipe
plumbing: the games consume the 0x3E0 pipe form through
`\\.\pipe\<name>`), and forwards every OTHER order verbatim into
`Class2USB` - i.e. app orders reach the USB layer as opaque 0x184-B
blocks.

## 2. The usbCom worker (xEyeUSB.dll @RVA 0x11770) - the wire loop

Startup (before the loop): power check -> load **`classifying.dll`**
(`SetLensParam`, `SetStaticPoint_Param`, `ClassifyAndTrack` - the SVM
with `SVM_Class_ball/head_*.dat`) -> **the ENS probe** (see 4) -> if
the probe fails, the **`ens_code.ldr` state machine** (see 5) ->
`SetLensParam(0x78-B param block from the ens state + 8, ...)`.

The steady-state loop, per record:

1. `EasyUSB ReadData(pipe=2, buf, 64, 500 ms)` until an **OD frame of
   type 0xF0** arrives (magic `0x7f7f444f` @+0, u16 type @+4;
   retry budget `LinkErrCnt=100` with "exit for disconnect" checks).
2. `ReadData(pipe=2, buf, **0x280 = 640 B**, 1000 ms)` - the raw record.
   Must end with the **OD magic dword at +0x27C** (checked @0x11d13).
3. `WriteData(pipe=3, buf, 64, 1000 ms)` - the ACK: a PU frame of
   **type 7** (`0x7f7f5550` @+0, u16 7 @+4, byte 0 @+0xA, payload @+0xC).
   The payload is filled from the PREVIOUS record by
   `ClassifyAndTrack` (the classifier receives a pointer to PU+0xC);
   the very first ACK is the zeroed 64-B buffer. So the ACK for record
   N carries the command computed from record N-1 - a closed loop.
4. `f660(wire_record -> xEye3Ddata)`: tiny - only decodes the status
   byte at **wire+0x25C** into trailer fields (bit 7 gates three flags
   from bits 4/5/3 inverted; bit 0 -> the +0x3B0 mode word).
5. `ClassifyAndTrack(640-B record by value, out, PU+0xC, 0, DebugShow)`
   -> 10 x 0x48-B classify slots; a remap loop (@0x11de2) packs them
   into the 10 x 0x58-B xEye3Ddata slots.
6. Frame counter @record+0 = a HOST counter (`[0x1001cbb4]++`), the
   classifier's own u16 counter (classify-out+0) is delta-checked
   (drop when > 1). Record pushed to `Data2Class`; loop paced to
   **USBComFPS = 35** (`xEyeUsb.ini`).

## 3. The endpoint mapping (pipe calibration)

EasyUSB.dll exposes `ReadData`/`WriteData` (ordinals 1/2) with a
per-pipe context table (4 read + 4 write slots, 0x20 B each); the pipe
number selects the driver's device interface. Calibration from the
known-good 7F 55 session (xeyezHal.dll engine @0x30a0: `WriteData
(pipe=0)` for commands, `ReadData(pipe=1)` for replies - and USBPcap
proves the 7F 55 traffic runs on **EP 0x02 OUT / EP 0x82 IN**):

| pipe | direction | endpoint |
| --- | --- | --- |
| 0 | OUT | 0x02 (7F 55 commands) |
| 1 | IN | 0x82 (7F 55 replies) |
| 2 | IN | **0x83** (record plane reads) |
| 3 | OUT | **0x03** (record plane ACKs) |

Pipes 2/3 must be IN/OUT respectively (reads need an IN endpoint),
and 0x83/0x03 are the only remaining MI_04 bulk endpoints (512-B
maxpacket). This settles "EP 0x03/0x83, never observed on the wire" -
they were never observed because usbCom never ran (the license gates
kept the app out of the game state in every capture).

## 4. The startup probe (the one-command decision)

`0x10640` (called from usbCom @0x11886): read 64 B from pipe 2
(EP 0x83), **100 ms timeout, up to 10 tries**, and check for an OD
type-0xF0 frame (helper `0x10590`). Success -> the stream is already
alive (skip the upload). Failure -> the ldr state machine.

**This probe is exactly what the new T12 bench example implements**
(`software/src/examples/records.rs`, `protocol::ens`) - one command now
decides on the bench whether the record plane streams spontaneously
after the session init or needs the runtime DSP upload first.

## 5. The runtime `ens_code.ldr` upload (the remaining unpinned wire bytes)

If the probe fails, `0xf790("ens_code.ldr")` runs a 7-state machine
(jmp table @0x1000f940; states call 0xfa30, 0xfae0 (the ldr file), a
skip state, 0xfe20, 0xffb0, 0x10130...) using the SAME 64-B PU/OD
framing on pipes 2/3 (write sites @0xec61...0xf23e, 0xfc2e-0xfd2f,
0x101ed-0x10466; read sites @0xeb0f-0xeee6, 0xfc5f-0xfd82, 0x10173).
This is the PU/OD traffic that NEVER appeared in the 535 s of captures
(the engine's connect gates failed before usbCom could start). One
capture with the kit-37 unlocked app in a running game pins these
bytes; the driver then replays them like `flash_mcu` replays the hex.

## 6. Open item #4 RESOLVED: the marker x/y/z layout

Two independent disassemblies pin it:

- `TransUSB2Pipe` @RVA 0x25e0 (the pipe-form packer) copies per slot:
  **qword @+0x08, qword @+0x10, qword @+0x18, dword @+0x24, dwords
  @+0x20/+0x28/+0x2c** - the three qwords are x/y/z (f64, mm).
- `GetADataFromUSB` @0x22a8 (the best-marker loop): among slots with
  quality (`i32 @+0x24`) >= 50, the **smallest @+0x18 (z) wins** - the
  NEAREST marker is the active one - and the winner's +0x08/+0x10/+0x18
  qwords become the manager's x/y/z floats (+0x770/+0x774/+0x778).

The `software/src/protocol/records.rs` parser now decodes the real
x/y/z (was: placeholder from slot+0x00). The old `gate` field WAS the
z coordinate.

IMPORTANT: the xEye3Ddata record is a HOST-side structure. On the wire
the device sends raw 640-B records (blob data - the classifier input);
the x/y/z values exist only after `classifying.dll` ran. The Rust
driver's marker plane therefore = the EP 0x83 wire records + (later, if
wanted) a host-side classifier.

## 7. Driver deliverables of this turn

- `src/protocol/ens.rs` (NEW): the record-plane model - `StatusFrame`
  (64-B OD type-0xF0), `WireRecord` (640-B, trailer + status byte +
  blob stats), `Type7Command` (64-B PU type-7 ACK/command builder).
- `src/protocol/mod.rs`: the record-plane constants (EP 0x83/0x03,
  frame/record lengths, types, probe budget).
- `src/protocol/records.rs`: real x/y/z decode, `nearest_marker()`
  (the vendor selection), tests updated.
- `src/marker.rs`: the `age` field removed (it was the misread z);
  coordinates documented as host-computed.
- `src/transport/usb.rs`: `read_bulk_ep` / `write_bulk_ep` (raw MI_04
  bulk I/O on arbitrary endpoints, behind `usb`).
- `src/examples/records.rs` (NEW): the T12 bench - session init
  (--cmos), the ENS probe, the handshake loop, raw dumps + stats.
- 119 tests green (both feature states), nightly fmt clean.

## 8. What the next bench day decides (T12)

```
cargo run --features usb --example records -- --cmos
```

- **VERDICT A** (type-0xF0 answers): records flow - dump them, decode
  the blob layout against a running-game capture.
- **VERDICT B** (silence): the runtime `ens_code.ldr` upload is needed
  first - capture the kit-37 unlocked app in a game to pin its wire
  bytes, then implement it as the next controlled replay.
