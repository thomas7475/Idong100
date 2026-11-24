
> **Turn-22 update —** the turn-21 kit still stopped at "Start install
> driver!" (预安装xeyez失败！ = "pre-install xeyez FAILED"): the very first
> sub-step (xEyeDriverCheck.dll!PreinstallSpecialDriver - CopyFile +
> CoDeviceInstallers/Driver-Signing registry writes + SetupCopyOEMInfA on
> the 2008-era inf) fails on Win10/11. The current kit stubs that thunk to
> return success so the install proceeds; it also adds
> `install_drivers_manual.bat` (pnputil route) as a fallback. Replace
> `xeyez_alone_patched.dat` (23,637,085 B; the exe is unchanged), re-run
> `install_patched.bat`; details in SOLVE_DIALOG.md (turn-22 note).


> **Turn-21 update —** the turn-20 kit extracted everything but failed at
> "Start install driver!" (驱动目录没有找到！ = "driver directory not
> found"). That gate compares the OS via GetVersionExA and accepts only
> XP/Vista/Win7; on Win10/11 it must be bypassed. The current kit's
> `xeyez_setup_patched.exe` (82,838 B) and `xeyez_alone_patched.dat`
> (23,635,851 B) carry that fix inside the embedded xEyeSetupHelper.dll.
> Replace any older copies and re-run `install_patched.bat`; details in
> `SOLVE_DIALOG.md` (turn-21 note). If Windows then refuses the 2008-era
> driver by signature, use the one-boot 'Disable driver signature
> enforcement' route (section 4).


> **Turn-20 update —** if the turn-19 patched installers showed
> **"NSIS Error — Installer integrity check has failed"**, the turn-19 repack
> had a stale CRC. Fixed: the current kit's `xeyez_setup_patched.exe`
> (82,842 B) and `xeyez_alone_patched.dat` (23,638,139 B) pass the stub's
> integrity check (verified by emulating the stub). Use `install_patched.bat`
> as before; details in `SOLVE_DIALOG.md` turn-20 note.

> **Turn-19 update —** a second vendor gate exists: an **OS-version
> whitelist** in `VersionCheck.dll` (`VerifyInstallSystem`) that accepts
> only XP 5.1 / Vista 6.0 / Win7 6.1 and, on 64-bit Windows, ONLY Win7.
> On Win8/10/11 the installers show `操作系统不兼容` and exit. Compatibility
> mode cannot fix it (the gate reads `_winmajor`/`_winminor` from
> MSVCR80.dll, not `GetVersionEx` — the compat lie never reaches it).
> Use the kit's **patched installers** — `install_patched.bat` swaps in
> `xeyez_setup_patched.exe` / `xeyez_alone_patched.dat` (4 gate jumps
> rewritten, every path returns 0; round-trip-verified repack) and
> launches the installer. Details: `SOLVE_DIALOG.md` § "The OS-version
> gate — turn 19".

# P2.3 THE GATE — USBPcap capture of a full vendor boot

> Repo copy of the kit README. The runnable kit (installers + scripts) is
> `download/idong100_p2_usbpcap_kit.zip` on the analysis host; this file
> is the canonical procedure documentation.

Goal: byte-verify the boot upload sequence the Rust driver will use
(`_InitBlobUpload`, xEyeUSB.dll) before ANY write-capable code path is
allowed on hardware. This resolves open items #1 (multi-firmware staging
order), #2 (CRC variant), #3 (count/CRC + DSP-start payload framing).

The capture target: the sensor boots in the VM (Windows + vendor stack),
the whole USB traffic is recorded, and the transcript is compared
byte-for-byte against the Phase-1 decoder (`idong/src/protocol/boot.rs`).

## 0. Why a VM is fine for this

The vendor stack is Windows-only and 32-bit. QEMU/KVM with virt-manager
passes the physical sensor through to Windows; USBPcap sits inside
Windows and records everything the vendor app does. The capture is
exactly as good as bare metal: same driver, same timing budget, same
device.

## 1. Kit contents

| File | Purpose | SHA-256 |
|---|---|---|
| `software/USBPcapSetup-1.5.4.0.exe` | USB sniffer (desowin, signed) | `87a7edf9bbbcf07b5f4373d9a192a6770d2ff3add7aa1e276e82e38582ccb622` |
| `software/Wireshark-win64-4.6.10.exe` | GUI + pcapng reader + extcap | `c3bd915305aef0cfaffb29981790623f97b8aa673fb5c790b1c64a3fd85c1e94` |
| `software/xeyez_setup.exe` | Vendor v2.6.0.6 stack (yours) | `4252597e98bb4242402b89352b9938e6681c9f5dabe0e43d363c9f630f982711` |
| `scripts/run_capture.bat` | One-shot capture session | — |
| `scripts/check_capture.bat` | Magic-byte sanity check (in VM; pure cmd + cscript, no PowerShell) | — |
| `MANIFEST.txt` | Hashes of every kit file | — |

The two `.dat` payloads (`xeyez_Lobby.dat`, `xeyez_alone.dat`) must sit
next to `xeyez_setup.exe` when the vendor installer runs — the setup exe
launches them. Copy them from `analysis/remote/dependent/
xeyez_pc_v2.6.0.6/` if the VM install re-downloads nothing (they are on
your host already; the kit is intentionally lean — ask if you want the
20 MB `.dat` files zipped in too).

## 2. VM requirements (QEMU / virt-manager)

- **Windows 10 64-bit or Windows 7 32-bit guest.** The vendor app is
  32-bit but runs fine on x64 Windows (WOW64). Win10 x64 is the least
  painful: test-signing is not required (see §6).
- **QXL/Spice display is fine**; no GPU passthrough needed.
- **CPU**: 2 vCPUs, 4 GB RAM is plenty (the vendor app is light).
- **USB passthrough — the one critical setup detail:**

  The sensor RE-ENUMERATES during boot: it starts as 0471:2600
  (bootloader, one configuration) and re-appears as 0471:2500 (runtime
  composite, FOUR interfaces incl. UVC video). Virt-manager's USB
  host-device passthrough is bound to the *device address*, and a
  re-enumeration changes the address → **the passthrough silently
  DETACHES mid-boot and the device vanishes from the VM.**

  Two working options:

  **A. QEMU command-line USB host passthrough (recommended).**
  Edit the VM's XML (virt-manager → ⚙ → XML tab) and add inside the
  `<devices>` section:

  ```xml
  <hostdev mode='subsystem' type='usb' managed='yes'>
    <source>
      <vendor id='0x0471'/>
      <product id='0x2600'/>
    </source>
    <address type='usb' bus='0' port='1'/>
  </hostdev>
  ```
  Add a SECOND identical block with `product id='0x2500'`. With TWO
  hostdev rules (one per PID), QEMU re-attaches automatically at the
  new identity after re-enumeration — this is the known recipe for
  devices that change PID on firmware load. (USB redirects via
  spice/virt-viewer "USB device selection" do NOT survive
  re-enumeration; avoid them for this capture.)

  **B. If your QEMU is older** and dual-PID matching misbehaves, use
  bus/port addressing: attach by the physical port the sensor is
  plugged into (find it via `lsusb -t`), so any device that appears on
  that port is passed through:
  ```xml
  <hostdev mode='subsystem' type='usb' managed='yes'>
    <source>
      <address bus='1' port='2'/>  <!-- physical hub port, from lsusb -t -->
    </source>
  </hostdev>
  ```

  Verify inside the VM BEFORE the session: the bootloader must appear in
  Device Manager as an unknown/RAW device with VID 0471 PID 2600 (it has
  no driver bound yet — that's normal and CORRECT: we want the VENDOR
  driver to install it during the capture).

## 3. Software install order (inside the VM)

1. **Wireshark** first — run `Wireshark-win64-4.6.10.exe`, defaults are
   fine. Do NOT launch it yet.
2. **USBPcap** second — run `USBPcapSetup-1.5.4.0.exe`. It installs
   `USBPcapDriver.sys` + `USBPcapCMD.exe`. **The installer asks to
   reboot — this is required** (the filter driver loads at boot). Do
   the reboot inside the VM (a VM reboot is enough, not a host reboot).
3. **Vendor stack** — copy `xeyez_setup.exe` + the two `.dat` files into
   one folder, run `xeyez_setup.exe`. Choose the **"alone"** flavour if
   asked (the standalone tool, not the Lobby game portal — "xeyez_alone"
   is the driver+utility install we analyzed; the Lobby variant also
   works). When the installer finishes, the tray app `xeyezMonitor.exe`
   / service may auto-start. **Stop here — do not plug the sensor yet.**

## 4. Driver-signature caveats (Windows 10)

- The vendor `xeyez.sys` (2008-era) is signed with an old AuthentiCode
  cert chain. On Win10 x64 it generally installs (the vendor co-installer
  `xeyezCoIns.dll` uses `UpdateDriverForPlugAndPlayDevices`), but if
  Windows refuses it: Settings → Update & Security → Recovery → Advanced
  startup → Troubleshoot → Advanced options → Startup Settings → **"Disable
  driver signature enforcement"** (option 7, one-boot only). That is the
  fastest unblock; test-signing mode (`bcdedit /set testsigning on`) is
  the persistent variant.
- USBPcap 1.5.4.0 is properly signed — no issue there.

## 5. THE CAPTURE PROCEDURE (the 10-minute session)

Everything below is automated by `scripts/run_capture.bat`; it is
spelled out here so the capture can be re-done by hand if needed.

1. In the VM, plug/attach the sensor via the hostdev rules (§2). Wait
   5 s. Confirm the bootloader is present: Device Manager shows
   "TOLTech xeyez Bootloader"-ish unknown device, or Wireshark's USBPcap
   interface list shows a `0471:2600` device.
2. Start the capture (run_capture.bat does):
   `"C:\Program Files\USBPcap1\USBPcapCMD.exe" -d \\.\USBPcap1 -o
   C:\idong_capture\boot.pcapng --snap 0` — then attach the sensor.
   USBPcapCMD lists devices when run bare; run_capture.bat targets the
   root hub filter `-d \\.\USBPcap1` so BOTH enumerations (2600 → 2500)
   land in one file.
3. **Launch the vendor app** (xeyez.exe / the tray monitor that auto-
   started). This triggers `_InitBlobUpload`: the exact sequence we
   decode in `idong/src/protocol/boot.rs`:
   DSP-start → "Ready to load program!" → count+CRC → 64-B type-3
   packets → CRC verdict → device-ID polls → DSP reset → re-enum.
4. Let the app run **at least 60 s past the boot** — the runtime
   keep-alives (0xFFF/0xFFE) and marker stream are also wanted (they
   validate stream.rs). If the app shows a preview/marker view, let it
   stream a minute.
5. Stop the capture (run_capture.bat stops USBPcapCMD; or press the
   key/click the console window's stop). `boot.pcapng` is complete.
6. Run `scripts/check_capture.bat` in the VM: it scans the pcapng for
   the PU/OD magics (bytes `50 55 7f 7f` / `4f 44 7f 7f`) and the
   "Ready to load program!" reply; it reports counts so you
   KNOW the capture is good before copying it out.
7. Copy the whole `C:\idong_capture\` folder out of the VM (shared
   folder, or `scp`/RDP clipboard) to the analysis host.

## 6. Acceptance criteria (what makes this capture THE GATE)

The pcapng must contain, in order:

1. Device-attach of 0471:2600 + GET_DESCRIPTOR traffic (bootloader).
2. Host→device bulk OUT frames with the PU magic `50 55 7f 7f` — the
   DSP-start, then type-2 count+CRC, then the type-3 64-byte packet
   train (about 3687 packets for the 235,930-B `.bit` — or a different
   count if the staging order starts with `.ldr`/`.hex`; THAT is what
   the capture will tell us — open item #1).
3. Device→host bulk IN frames with the OD magic `4f 44 7f 7f` — the
   "Ready to load program!" ASCII (type 2), the CRC verdict (type 3,
   status word), device-ID string.
4. USB disconnect of 2600 + re-connect of 2500 (re-enumeration), with
   the composite descriptors (UVC/UAC interfaces 0-3 + vendor MI_04).
5. Runtime traffic: 0xFFF polls answered 0xFFE within ~1 s, link-status
   type-4 exchanges, and bulk-IN marker records at ~50 fps.

If any of 2-4 is missing, the capture is void — redo §5 with the
sensor freshly power-cycled (it boots to the bootloader again only
after being unplugged from power; a warm USB replug may keep it in
runtime state — that's the vendor design, not an error).

## 7. After the capture (host side, next turn)

`check_capture.bat` only sanity-checks; the byte-for-byte comparison
against our decoder happens on the host:
`tshark -r boot.pcapng -Y usb.capdata` → extract the bulk payloads →
feed them through the Phase-1 `FrameDecoder`/`BootUploader` replay
(golden-vector test, `tests/data/`). When the transcript matches to the
byte, `BOOT_CONFIRMED` flips to `true` and Phase 3 (real hardware boot
via our own uploader) is unblocked. Until then, nothing writes.

## 8. Safety rules reminder (unchanged)

- The capture itself is passive — it cannot damage anything. Run it as
  many times as needed.
- The vendor stack DOES write to the device (it's their job); that's
  fine, they wrote it.
- OUR code still writes nothing until the gate passes.
