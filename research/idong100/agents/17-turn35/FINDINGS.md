# Turn 35: the video capture (phase 5) - the protocol is complete

Date: 2026-11-27. Input: `idong_capture.7z` (run1, 120 s - the
`xEyeThirdLens.exe` session), `idong.zip` (the lobby/games install
folder), the report that the patched `xEyeUSB.dll` still leaves the
lobby at "Connecting to idong sensor", and two questions: can the
camera software run directly, and is more USB capture needed for the
driver.

## 1. The capture - VIDEO IS STREAMING (phase 5)

The turn-34 prediction held: with the engine up, xEyeThirdLens
negotiated the video and streamed it. `check_wire34`-equivalent
decoding (`scripts/wire35.py`): **PHASE 5 - VIDEO STREAMING**,
82.8 MB isochronous IN on endpoint 0x85 over ~68 s.

Timeline (capture t=0 at 17:21:56):

| t | event |
| --- | --- |
| 0-13.3 s | 2500 session live; 0x0205 stream polls at ~30 Hz (2,963 OUT) |
| 17.8 s | sensor self-reboots to 2600 (the known 31-157 s instability) |
| 18.9 s | host 0x000d poll (unanswered - device gone) |
| 35.2 s | 0x0008 -> "2600"; **full boot re-run**: 0x0101 `<00 02 53 A3>`, 335 raw 64-B blocks, 0x0201 verify + echo, 0x0301 key, 0x0401 launch |
| 42.1 s | re-enumerates as 2500; host walks the UVC GET_CUR/MIN/MAX/RES set (cs 02/03/06/07/08) |
| 51.8 s | UVC probe SET_CUR x2 + commit SET_CUR (fmt 1, frame 1, 333333, 614400) |
| 52.0 s+ | isochronous video on EP 0x85 (the 0x0205 polls had already resumed) |

The boot re-programming is the second full observation (cap2/3/4 were
the first) - the engine re-flashes on every bootloader landing, it
never tries to boot the resident image.

## 2. The video format, decoded

- **UVC payloads**: 12-B headers `0c 8c/8d` = `[len][flags EOH|SCR|PTS
  (+FID)] [pts LE u32] [scr 6 B]`, then up to 2400 B image data. One ISO
  record = up to 27 payloads, 2412 B apart (last one 1236 B).
- **Frames**: PTS constant per frame, +1,920,000 per frame. At the
  device clock of 15.36 MHz that is exactly 125 ms -> **8.0 fps**
  steady state (546 FID transitions / 68 s; the negotiated 30 fps is an
  upper bound). EOF flags appear only late in the session (80 payloads,
  after record 958) - frame assembly must key on PTS, which is what the
  Rust `VideoAssembler` does.
- **Raster**: row stride 1280 B - confirmed by row-to-row correlation
  0.998 at width 1280 and autocorrelation peaks at 1280/2560/3840.
  = **640 px x 2 byte-interleaved channels**:
  - channel A (even bytes): the scene image. Rendered as PNG, the room
    with a hand is clearly visible (VLM-verified).
  - channel B (odd bytes): a second image-like raster (row corr 0.97),
    low contrast (range 104-154, peak 128), heavy 2-px column pattern,
    with faint scene content (the hand blob appears). Two planes are
    mutually uncorrelated at every shift - two different images, not
    hi/lo bytes of one pixel.
- The negotiated format claims 640x480x2 = 614,400 B/frame, but the
  delivered frames are ~190,908 / 127,272 B (alternating 3/2 ISO
  records) - the steady-state rate is 1.27 MB/s, not 18.4 MB/s. The
  vendor firmware sends partial frames (the raster restarts at row 0 of
  each PTS frame; the partial row tail is dropped). For the 3D-scanner
  goal the raw stream is what matters - capture first, interpret later.

## 3. The stream poll (cmd 0x0205) - the new runtime command

While video streams, the engine polls the vendor interface at ~30 Hz:

```text
OUT: 7f 55 05 02 00 00 00 00
IN : 7f 55 05 02 01 00 | 00 <code> <frame_no LE u32> <aux 4> |
     ... zeros ... | <trailer: ff ff ff 00 1f 00 05/04 00> | zeros
```

`code` 0x04 = new frame ready (4-B arg = the video frame counter,
+1 per poll tick while streaming), 0x00 = empty. The counter
(`1b 6d ...` = 27,931 at capture start) advances ~30/s - a camera-frame
counter, distinct from the 8-fps video PTS. Implemented as
`RuntimeSession::poll_stream()` + `StreamPollState`.

## 4. The lobby - why it still says "Connecting" (and the direct answer)

The user patched `xEyeUSB.dll` (turn-31/33) - correct but not
sufficient. The lobby's camera chain is:

```text
lobby.exe -> VideoSign.dll (Init / IsSigned / GetXYZStr)
             -> xEyeMsgEngine.dll (InitxEyeMsgEngine + StartHeartBeat)
                -> named pipes / window msgs -> xeyez.exe (engine host)
lobby.exe -> ConfigManagement.dll (GetLobbyVer etc.)
```

`VideoSign::Init` sets the lobby pipe name **TOL_LOBBYDATA_PIPE**, then
requests two engine properties over the pipe protocol:
`getMiddleCameraCalibrateData` and - the gate the "Connecting to idong
sensor" screen waits on - **`getMiddleCameraCalibrateState`**
(`IsSigned` = that property must report ready; disassembly: VideoSign.dll
Init @0x10001040, IsSigned @0x10001360; engine-side handler:
xEyeMsgCenter.dll `?getMCCalibrateState@xEyeMsgCenterManager@@`,
message `xEyeMsgEnum_GetMCCalibrateState`).

The engine host serves that state only when it is fully up. Three gates
must ALL pass:

1. xEyeUSB.dll connect + edition tag (turn-31/33) - the user patched
   this one.
2. **xEyeRight.dll license** (turn-34) - phones the dead server
   us.i-dong.com, never authorized, the engine host pops the
   "sensor not authorized" window (`ShowDeviceIDInvalidWindow`).
   **NOT patched by the user** - `patch_engine34.bat` patches BOTH
   xEyeUSB.dll (t33 state) AND xEyeRight.dll (MakeFinalResult 0x927 ->
   always authorized + the vendor's own 0x7E8 success notification).
3. The middle-camera calibration itself (engine-side
   MiddleCameraCalib.dll + Co2D.ini + the sensor EEPROM calibration
   blocks at 0x0100/121 - all present and reading fine on this unit).

The lobby's own config.ini also references the dead server
(`ServerBaseURL=http://us.i-dong.com/outer/server.action`,
`NetTestURL=.../heartbeat.htm`) - announcements/login will time out,
but that is cosmetic for the sensor connection.

**Direct answer: yes - two levels.** (a) The camera software already
runs directly: `run_direct34.bat` (xeyez.exe engine + xEyeThirdLens.exe
viewer) - THIS capture proves it works end-to-end (video streamed).
(b) The games also bypass the lobby: `Lobby/flash` + FlashPlayer.exe
load the game SWFs, but they use the same engine pipes
(xEye2DEngine.dll) - so the precondition is the same engine state. The
practical unlock order on the Windows box:

1. `patch_engine34.bat` (admin) - patches BOTH DLLs, verify with
   `verify_engine34.bat`.
2. `run_direct34.bat` - engine up; watch the tray reach "running"
   (引擎正常运行).
3. Start the lobby (lobby.exe / StartLobby.exe). With the license gate
   patched the engine's middle-camera calibrate state should reach
   ready and the lobby should finally pass "Connecting".
4. If it still hangs, the next diagnostic: does the tray show "sensor
   not authorized" (license, re-check step 1) or "running" but the
   lobby still waits (then it is the MC-calibration state and we
   disassemble `?getMCCalibrateState` next).

## 5. Is more USB capture needed for the driver? - NO

This capture completes the protocol set:

| plane | status |
| --- | --- |
| bootloader flash sequence (0008/0101/335 blocks/0201/0301/0401) | observed twice, byte-pinned |
| session register/EEPROM/CMOS cycle | observed (cap2/3/4), byte-pinned |
| device-ID read + edition gates | observed (turn 33/34) |
| UVC negotiation + probe/commit values | observed THIS capture |
| video stream format (headers, PTS, raster) | observed THIS capture |
| stream poll 0x0205 | observed THIS capture |

## 6. Driver: the video phase, implemented (60 tests green)

- `idong/src/protocol/video.rs` (new): `UvcHeader` (12-B EOH|SCR|PTS
  form), `VideoAssembler` (PTS-advance frame closure, EOF/error aware),
  `VideoFrame` + `split_channels` (the 640-px two-channel raster),
  `ProbeCommit` (26-B little-endian form, golden host-probe +
  device-commit bodies), `StreamPollState` (the 64-B poll reply),
  `VideoSource` (the transport seam: one read = one ISO payload;
  V4L2/usbfs behind it on Linux).
- `idong/src/protocol/runtime.rs`: `CMD_STREAM_POLL = 0x0205` +
  `RuntimeSession::poll_stream()`.
- `idong/tests/video_golden.rs` (new): 11 tests, every byte literal
  from the capture (poll request/replies, payload headers with the
  1,920,000 PTS delta, probe/commit bodies, capture-stride transfer
  walk, channel split).
- Whole suite: 37 unit + 5 mock_replay + 6 runtime_replay + 11
  video_golden + 1 doc test = **60 green**, `cargo fmt --check` clean.

Open items for the next turn:
- the 16-bit/2-channel pixel semantics of channel B (second lens? FPGA
  intermediate? correlate against the projector pattern),
- frame completeness (are the ~159 KB frames partial 480-row images?),
- V4L2 backend: claim MI_04 through libusb + open the UVC node, wire
  `poll_stream` + `VideoSource` together into a `VideoStreamer`.

## 7. Artifacts

- `scripts/wire35.py` - Python phase decoder (check_wire34 port),
- `scripts/cap8_deep.py`, `scripts/iso_probe.py`, `scripts/iso_hdr.py`,
  `scripts/frames35.py`, `scripts/render35.py`, `scripts/raster35.py`,
  `scripts/pitch35.py`, `scripts/planes35.py`, `scripts/poll502.py` -
  the capture analysis chain,
- `work/cap8/frames/*.png` - rendered channel-A/B images (the room+hand
  proof), `stream_raw.bin` - the header-stripped video stream.
