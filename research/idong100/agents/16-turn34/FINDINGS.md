# Turn 34: the driver files, the third gate (license), and the direct run

Date: 2026-11-26. Input: the user's two full install-folder zips
(`xeyez_pc_v2.5.0.7-inst.zip`, `xeyez_pc_v2.6.0.6-inst.zip` - the
"driver files from the install folder"), the report that the turn-33
patch did not resolve the stuck "Connecting to idong sensor" screen,
and the question whether the camera software can be run directly.

## The install folders prove the patch is gone

The v2.6.0.6 zip (root `xeyez_engine/`) contains the PRISTINE vendor
`xEyeUSB.dll` (md5 `00563d0f5c49d1d132e1475ee8f4841b`). If that zip is
the current state of the install, the turn-33 patch was wiped - a
reinstall restores the stock DLL. The turn-33 kit's install finder was
also too narrow (one registry value + `C:\TOLTech` / `D:\TOLTech`),
so it may equally never have found the real install. The turn-34 kit
finder checks: `HKLM\SOFTWARE\TOLTech\Location` (both views),
`HKLM\SOFTWARE\Toltech\xeyez_engine\Location|RootPath` (both views - the
values the real installer and the kernel driver use), the
`xEyeService`/`xeyez` service ImagePaths, `Program Files\TOLTech`
(+x86), `C:\TOLTech`, `C:\xeyez_engine` and the D: twins, plus an
optional explicit argument.

## The third gate: the license check (xEyeRight.dll)

Full static decode of `xEyeRight.dll` (v2.6.0.6 md5
`e94851c75f6f9deea0159e1cae695861`; v2.5.0.7 md5
`dcc4f2614140d55fb397f55032395501` - the MakeFinalResult region
0x927..0x967 is byte-identical in both):

- `InitxezeRight(hwnd, ...)` (VA 0x100023d7): reads `.\xEyeRight.ini`
  `[General] ServerDomainName` (default `us.i-dong.com`) + `Port`,
  stores the notify HWND at this+0x1EC, starts the rights thread.
- `ThreadProc` (VA 0x10002339), every 500 ms:
  `getLocalRightResult` (loads the local right file into this+0x44) ->
  `LocalRight` (VA 0x10001944) if this+0x28==0 -> `NetRight`
  (VA 0x10001d25) while this+0x2c==0 -> `MakeFinalResult`
  (VA 0x10001527) -> Sleep(500).
- `LocalRight`: no device-ID set (this+0x8c==0) -> nothing; the saved
  right's device ID (file bytes 4..36) != current ID -> net result 5;
  no right file (`this+0xb0` or `right_file_name.dat`) -> net result 3;
  else reads 4 B at `getRightResultPos(this+0x88)` and popcounts it
  against 0x4F803CF0: >= 10 bits -> 2, else 1.
- `NetRight`: WSAStartup, socket, `gethostbyname(us.i-dong.com)`,
  connect, OpenSSL (libeay32/ssleay32), `POST /outer/device.action`
  with `deviceID=%s&key=%s&version=%s&engineVersion=%s&xeyezVersion=%s&mcuVersion=%s`,
  response -> "authDevice" -> saveRightLocal + net result 1.
  The server is dead since ~2012: the net result can never be 1.
- `MakeFinalResult`: final result this+0x24 = net result this+0x28
  (only after 20 failed net rounds); on result==1 it sends
  message 0x7E8 to the notify HWND (the engine host's window).
- Consumers: `getFinalRigh` ("1"/...) via the tray
  (`xEyeSysTrayTip.dll`), `GetxEyeRightState` via the engine host
  (`xeyez.exe`), `getRightCount` via the USB main loop in
  `xEyeUSB.dll` (returns net-attempt count 0x1b8, or finalCount=3
  after 20 failed rounds - with finalCount=3 the loop proceeds, so
  the main loop is NOT the blocker; the HOST/UI is).
- `xeyez.exe` shows the "sensor not authorized" screen through
  `xEyeWarrningUI.dll!ShowDeviceIDInvalidWindow`
  (exported; the StartupUI image `ShiBieQiWeiShouQuan2.bmp`
  = "识别器未授权").

**Turn-34 patch (kit `idong100_engine34`):** file offset 0x927
(= VA 0x10001527), the 0x2D-byte guard block of `MakeFinalResult`
-> `xor edx,edx; mov dword ptr [ecx+0x24],1; jmp` into the vendor's
own notify tail (`push 0; push 0; push 0x7E8; push [ecx+0x1EC];
call [USER32!SendMessageA]; ret`). No absolute address is
introduced: the vendor's `call [0x10004130]` keeps its original file
offset 0x961 and therefore its existing relocation entry. Every call
of MakeFinalResult (every 500 ms) now reports "authorized" and fires
the vendor's own success notification. Emulation of all states:
vendor -> t34, t34 -> noop, t34_v25 -> noop, foreign -> refused.

## The engine state machine (why "connecting" is the stuck state)

`update_state` (VA 0x1000b2c0): mode 0 -> state 8 "识别器断开连接",
mode 1 -> 0xd "识别器连接异常", audio-loading -> 0xb "载入音频驱动",
paused -> 6 "引擎暂停", else stage() (VA 0x1000b200):
**stage 4 + device-ID flag==0 -> reported as stage 1** -> state 2
"检测识别器中" (= the "Connecting to idong sensor" screen).
Stage 4 + flag -> state 7 "引擎正常运行" (or 0xa "环境干扰过大"
when the interference counter 0x1001c320 > 0).

- `check_device_id` (msg 5, VA 0x1000a310): manager dispatch vt+8
  (msg 1) = `sub_100054b0` reads the 32-char device ID via
  `xeyezHal_DeviceIDRead` (0107 @0x0300, 33 B, backup 0x0700) into
  the manager; vt+0xc copies it out; then flag=1 and
  `xEyeRightManager::setDeviceID(serial)` (which INVALIDATES the
  right state, forcing a re-auth).
- The manager (McuManager25) vtable @0x10015c28: +4 InitDevice-check,
  +8 dispatch (msg 0 connect 0x10005360, msg 1 device-ID 0x100054b0,
  msg 5, msg 6), +0xc data-get, +0x10 "shutdown", +0x14 "needs
  reflash" (posts msg 0xc).

## The driver files (the user's question #1)

- `xeyez.sys` = "TOLtech xeyez Driver" 2.5.4.9, WinDDK **bulkusb
  sample** port ("BF527 Bulk USB DriverEntry" - the sensor's DSP is
  an **Analog Devices Blackfin BF527**; `ens_code.ldr` is its LDR
  image), x86 (objfre_wxp_x86) + x64 (objfre_win7_amd64) builds.
- Service `XEYEZ` (demand start), device
  `\Device\xeyezUsbDriverDev`, symlink `\DosDevices\xeyezUsbDriverDev`;
  apps open `\\.\xeyezUsbDriverDev` and use plain
  IRP_MJ_CREATE/READ/WRITE - **no custom IOCTLs**; EasyUSB.dll is the
  user-mode wrapper.
- The INF binds `USB\VID_0471&PID_2600` (whole bootloader),
  `USB\VID_0471&PID_2500&MI_04` (vendor interface of the running
  composite), 2100, 0668, 0666.
- The driver reads `\Registry\Machine\SOFTWARE\Toltech\xeyez_engine`
  value `Location` in kernel mode.
- `A_a_v4.4_BG0313.bit` = Xilinx bitfile, design **"work_top.ncd"**
  (`work_top;HW_TIMEOUT=FALSE`), 2010/04/06 - the FPGA image the
  engine uploads (BitFile in .\xEyeUsb.ini).
- `DriverConfig.ini`/`xEyeDriverCheck.dll` only check the legacy
  v1.x IDs (0666/0668) - not relevant to this unit.

## The direct run (the user's question #2)

- The engine = `xeyez.exe` (component host; window class
  `xEyezEngine2009`; loads xEyeUSB/xEyeRight/xEyeClass/xEyeCalculator/
  xEyeInterFace/xEyeUpdate; started by `xEyeService.exe` via
  `xEyezSvStartXeyez.exe`, or by `xeyezMonitor.exe`, or DIRECTLY).
- Camera clients talk to the engine over **named pipes**
  (`xEyeEngine.dll` = the pipe client stub:
  InitxEyeEngine/GetxEyePipeData/BackxEyeData/ClosexEyeData/SetxEyeProperty);
  the lobby talks TCP 40000/40001 (`portlobby`/`portbuddy`,
  conf/version.xml, vsid 2=PC edition).
- **`xEyeThirdLens.exe`** = the vendor's standalone camera viewer:
  imports xEyeEngine.dll + xEyeMsgEngine.dll, dynamically loads
  MiddleCameraCalib.dll, reads .\Co2D.ini - it runs the camera WITHOUT
  the lobby once the engine is up.
- The 2500 composite exposes a standard UVC video streaming interface
  (alt 1/2 with ep 0x85 IN) - once the engine unlocks, any UVC
  player can potentially open the video too.
- Kit `run_direct34.bat`: finds the engine folder (registry +
  Program Files + TOLTech), verifies the DLL states, starts
  `xeyez.exe` directly (correct working directory: all configs are
  relative), offers `xEyeThirdLens.exe`, then the capture.

## The kit (idong100_engine34)

- `patch_engine34.bat` (admin): robust finder (see above) + full
  unlock: xEyeUSB.dll -> turn-33 state (upsell + tag), sibling
  xEyeRight.dll -> turn-34 state (build-matched reference), with
  backup/verify/rollback; refuses v2.5.0.7 xEyeUSB builds and
  foreign builds.
- `verify_engine34.bat`, `restore_engine34.bat`,
  `check_unlock34.bat` (driver service + wmic device presence +
  registry + DLL states).
- `check_wire34.bat` (aliased as `check_capture.bat` for
  run_capture.bat): phase decoder - flash / session / EEPROM reads
  with (offset,len) / device-ID read+serial-reply / DSP (PU) + ldr +
  bit literals / UVC SET_CUR probe-commit / SET_INTERFACE alt>0 /
  IN bytes per endpoint -> verdict PHASE. Validated against cap7:
  3 boot cycles, "000T" reads at 0x322 (3 per cycle), calibration
  0x100/121, code B 0x328/5, NO device-ID read (consistent with the
  turn-33 findings).
- Reference DLLs: xEyeUSB vendor/t31/t33/vendor_v25 +
  xEyeRight vendor/t34/vendor_v25/t34_v25 (md5s in MANIFEST.txt).

## Next

1. User: `patch_engine34.bat` -> `run_direct34.bat` -> tray should
   reach "running" -> `run_capture.bat` 60+ s -> `check_wire34.bat`.
2. The expected new wire phases: the 0107 @0x300 device-ID read
   (+ the serial reply), possibly DSP (.ldr) and FPGA (.bit) uploads,
   UVC probe/commit + SET_INTERFACE alt>0, then video on ep 0x85.
   That capture finally completes the protocol for the video stream -
   the last missing piece for the Rust driver and the 3D-scanner
   goal.
