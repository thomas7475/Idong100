# ZCode Chat Context Backup — 2026-11-23 (turn 11)

**How to use this file:** attach/upload it in a future session so the assistant can
restore full context of this conversation and continue where it left off.
This file was written by the assistant itself as a session snapshot.

---

## User's standing rules (apply to EVERY reply from now on)

1. **English only** for all communication.
2. **Zip backup of generated files** must be placed in `/home/z/my-project/download/` every time files are produced.
3. **Chat-context backup** (this kind of file) must be produced every time as well, downloadable from `/home/z/my-project/download/`.
4. **NEVER run full `cargo` builds** for the complete project — not enough RAM. Use `cargo check`, single-bin/example builds, or lowered parallelism.

## Turn 11 question and answers (the user asked, answers all evidence-backed)

Q: "is there some way to get 3d depth data from the camera, can we modify the
algorithm running in the fpga, goal is to use the camera as 3d scanner"

**A1 — 3D depth: YES, three tiers.**
- Stock firmware streams sparse 3D: 10 targets/frame, X/Y/Z doubles, host
  triangulates via StereoCamera.dll Get3dSingle with EEPROM calibration.
- **Raw image path EXISTS in stock firmware**: xeyezHal.dll exposes
  GetGlobalImageSize / SetTransMode / GetImageBegin (cmd 6 "GET_IMG_START") /
  GetImageData (bulk reads). Register scripts select modes: reg 0x02 =
  0x0A blob mode vs 0x0B image mode (2-sensor diff). Driver replays vendor
  scripts → images → host dense stereo → depth maps.
- If image mode is diff-only: replace the Blackfin DSP program (ens_code.ldr,
  host-uploaded volatile) to stream raw frames.

**A2 — FPGA: replaceable, not editable.**
- FPGA = Xilinx Spartan-3 **XC3S400-FT256**, design `work_top`, 2010/04/06,
  235,930 B. Bitstream is HOST-UPLOADED at every boot (`BitFile=` in
  xEyeUsb.ini; xeyezHal_SetFpgaFile/DownLoadFpga). Zero brick risk (volatile).
- No public Spartan-3 bitstream decompiler → cannot edit vendor algorithm.
- Replacement possible (ISE 14.7 WebPACK free) but needs board pinout —
  last resort. DSP (BF527, open bfin-elf-gcc toolchain) is the better lever.

**A3 — Scanner: viable at 0.3–1.5 m.** sigmaZ = Z²σu/(fx·B), fx=432.33 px,
B=100.00 mm: 1.2/2.9/5.8 mm @0.5 m (σu=0.2/0.5/1.0 px), 4.6/11.6/23.1 mm @1 m.
Phases P0 (blob stream + tracked-stylus probe) → P1 (image mode dumps) →
P2 (dense stereo host-side) → P3 (sweep + fusion, PLY/STL). Fallbacks in
order: DSP firmware, then FPGA bitstream.

## Key new protocol knowledge (verified, implement in Rust driver)

- **Runtime framing (gen-2, after boot)**: TX `[0x7F][0x55][cmd][b2][b3][b4]
  [u16 arg BYTE-SWAPPED][payload…]`; RX same magic, status byte @+4.
  Engine @ xeyezHal v2.6.0.6 VA 0x100030a0; builder 0x10003010; WriteData
  pipe 0 / ReadData pipe 1; timeouts 1000/5000 ms. Image data reads use
  "pipe 3" (EasyUSB logical channel; physical = same bulk EPs 0x02/0x82).
- Commands: 3 = register write (≤8 B chunks, u16 = chunk len), 5 = blob data
  read (resp mode 4), 6 = GET_IMG_START (resp status 1), 15 = reg/classifier
  write variant. Boot-stage framing stays 'PU\x7f\x7f'/'OD\x7f\x7f' 64 B.
- Register scripts (ASCII "0xRR,0xVV"): 30fps_Global_0316_Cmos.txt (common),
  blob script (mode 0x0A + regs 0x50-0x57), image script (mode 0x0B).
  data0.dat writes command reg 0xF0 (page selector). NEVER write 0x02/0xF0
  speculatively.
- xeyezHal.dll v2.5.07 = 61 exports, v2.6.0.6 = 79 (adds DRM/SecurityComm/
  CalibNum/BlobAxis/DeviceCam/CheckCode — v2.6 has anti-piracy; replay vendor
  files unmodified for P0-P1).
- xeyez.inf PIDs: 0666/0668/2100/2500/2600 = generations; user's = 2600 gen-2.
- CapStill.dll/exe = SCREEN capture (game photos), NOT camera frames —
  negative finding. Camset.dll = UVC webcam UI (other camera in the kit).
- StereoCamera.dll PDB path: "V2XeYeZToASIC" (they moved to ASIC later).

## Files produced this turn (all in /home/z/my-project/download/)

- `idong100_3D_Depth_FPGA_Study.docx` / `.pdf` — 16 pp, 10 chapters +
  Appendices A (xeyezHal inventory) B (register scripts) C (packet formats),
  12 tables, all numbers computed from artifacts. postcheck 8/9 (1 benign
  table-line-spacing warning). R1 cover, DM-1 palette, roman TOC/arabic body.
- `RUST_DRIVER_DRAFT.md` — rev 2: new Part III (xeyezHal protocol, image
  capture, scanner phases, module delta: protocol/cmd.rs, hal/{registers,
  image,control}.rs, geometry/{calib,depth}.rs, bin/dump_frames.rs) +
  Appendix A extended with runtime framing.
- `chat_context_backup_2026-11-23_turn11.md` — this file.
- `idong100_research_turn11_2026-11-23.zip` — all of the above + the three
  xeyezHal disassembly note files.

## Analysis artifacts (not in download/, in analysis/)

- `analysis/xeyezHal_protocol_notes.txt` (21K lines, all exports disasm)
- `analysis/xeyezHal_impl_disasm.txt`, `analysis/xeyezHal_builder.txt`
- `analysis/capstill/` — CapStill.dll+exe + Camset.dll (MD5-verified,
  extracted from live vendor server slice 1 via ranged GET; server
  download.i-dong.com still ALIVE as of this turn)
- `analysis/live_test/idong_setup-1.bin` (2.5 MB prefix), `idong_setup-2.bin` (2.5 MB)
- Scripts: `scripts/hal_disasm.py`, `scripts/hal_impl_disasm.py`,
  `scripts/fetch_slice_extract.py` (ranged-GET slice fetcher),
  `scripts/depth_report.js` (report generator).

## Pipeline notes (rebuild if needed)

- docx: `node scripts/depth_report.js` → `add_toc_placeholders.py --entries
  '<json>'` (H1-only list; --auto inserts 28 entries = TOC overflows to a
  sparse 2nd page — known gotcha) → `fix_toc_ghost.py` → `patch_footers.py`
  → postcheck → soffice pdf → pdftoppm render check (no page <2% non-white).
- Xilinx .bit header parse: 00 09 + 9 sync bytes, 00 01+'a' key, then
  design(u16 len), then raw keys b/c/d/e + u16 BE len + value (values carry
  trailing NULs — trim). Marker-based parse in depth_report.js works.
- Inno slice extraction: chunk offsets are absolute file offsets; slices
  named setup-N.bin; CapStill.dll chunk @0xcdcfa in slice 1 spans beyond a
  900 KB prefix — fetch ≥2.5 MB of the slice.

## Status / likely next turns

1. User runs the Rust driver boot sequence against the live device → first
   live GET_IMG_START / GetGlobalImageSize results.
2. USBPcap capture of vendor calibration/test session → confirm remaining
   command IDs + blob stream framing byte-for-byte.
3. If image mode returns diff-only → Blackfin disasm of ens_code.ldr
   (54,420 B, BF527; blocks at 0xFF800040 data / 0xFFA00000+ code) to map
   the FPGA↔DSP interface for the custom-DSP-firmware path.
4. Prior standing threads: Toltech SDK request email (with serial
   AB460DC8BA12C6840B49CD5786C329DD); read-only flash dump option;
   safety rules in force (read-only EP 0x82 until sure; no speculative
   EP 0x02 writes; NEVER write the T24C16A EEPROM; no full cargo builds).
