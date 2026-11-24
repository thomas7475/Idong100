# ZCode Chat Context Backup — 2026-11-23 (turn 12)

**How to use this file:** attach/upload it in a future session so the assistant can
restore full context of this conversation and continue where it left off.
This file was written by the assistant itself as a session snapshot.

---

## User's standing rules (apply to EVERY reply from now on)

1. **English only** for all communication.
2. **Zip backup of generated files** must be placed in `/home/z/my-project/download/` every time files are produced.
3. **Chat-context backup** (this kind of file) must be produced every time as well, downloadable from `/home/z/my-project/download/`.
4. **NEVER run full `cargo` builds** for the complete project — not enough RAM. Use `cargo check`, single-bin/example builds, or lowered parallelism.

## Turn 12 request and outcome

Q: "update the extraction script to print a sha for every file extracted.
Goal is to extract all the interesting files we can use to get information
about the working of the camera, analyse them, and make a PDF with the
results." (Plus standing rules: zip backup + chat backup.)

**Done — all parts:**
- `inno_bin_extract.py` updated: prints `SHA256 <hash> MD5 <hash> MD5_OK
  <size> <path>` for EVERY extracted file, verifies against the Inno-stored
  MD5, `--manifest FILE` TSV output, `--include REGEX` selective extraction.
  Live run: 178 records, 178/178 MD5_OK, 0 failed (170 unique paths; 8 paths
  duplicated in the custom-Inno file table with identical bytes).
- New `fetch_zip_members.py`: ranged-GET member fetcher for the vendor ZIPs
  (CD listing + chunked/resumable download + SHA256/MD5 per member +
  manifest). Vendor server download.i-dong.com STILL ALIVE (2026-11-23).
- Extracted & hashed **2,475 files** (unified manifest
  `analysis/reports/all_extracted_sha256.tsv`): both NSIS driver stacks
  (v2.5.0.7 from shishang-A zip: 142 files; v2.6.0.6 from yundongwang-A zip:
  145 files), Sports King slice-1 camera/middleware set (170 unique),
  9 remote members incl. full idong_setup-1.bin (197,873,664 B).
- Analysis (`scripts/analyze_extracted.py` → `analysis/reports/`): PE
  inventory 933 binaries, firmware parse, config dumps, version diff.
- **PDF**: `download/idong100_camera_research_turn12.pdf` — 17 pages, cover
  (Template 07 Crystal Blue), TOC (roman) + 10 chapters + appendices A
  (SHA-256 manifest) B (mode scripts verbatim) C (script usage), 8 tables,
  3 figures. pdf_qa PASS (4 benign nested-callout centering warnings only).
  NOTE: this session's model had NO image input, so the visual check was
  done programmatically (pdftoppm + non-white ratios per page, text
  extraction, TOC/footer consistency, placeholder scans) — all clean.

## Key new knowledge (verified; implement in Rust driver / use next turns)

- **FPGA mode switch from vendor scripts**: reg `0x02` = `0x0A` blob /
  `0x0B` image. Blob script programs regs 0x50-0x57: 0x51/0x52=0x027C(636),
  0x53/0x54=0x01D0(464) → 636×464 blob window; ends `0x06,0x02` (start).
  Image script has NO blob regs. Scripts live in `$_6_/30fps/` (+
  `30fps_1/` duplicate copy).
- **xeyezHal.ini (v2.6)**: four configs blob/image × rolling/global; rolling
  commented out (`#`), files absent → shipped = global only. IMAGE mode FPGA
  script named `30fps_Global_image_2sensor_diff_1LED.dat` → **image stream =
  2-sensor difference** (vendor's own label).
- **Bitstream pinned**: work_top.ncd;HW_TIMEOUT=FALSE;UserID=0xFFFFFFFF,
  3s400aft256, 2010-04-06 10:33:41, payload 235,820 B.
- **ens_code.ldr**: first block 4 B → 0xFF800040; header-like targets
  cluster in L1 data B (51× 0xFF9xxxxx); 17 fn-pointers 0xFFA03D1C-0xFFA0E6BE
  (code ≈43 KB); ASCII ep0_handler/usb_init; UTF-16 "xeyez"/"TOLtech".
  LDR block framing NOT decoded (standard flag layouts rejected) — open.
- **8051**: v2.6 has app_m8051_2500.hex (21,110 B) + _2611.hex (21,362 B);
  v2.5 one app_m8051.hex. Layout: reset vec 0x0000, 8 int vectors
  0x8000-0x806D, code 0x8073-0xD2A6/0xD3A2.
- **data0-10.dat = ASCII register pages** (0xF0 selects page; data0 = page 3
  regs 0x00-0x42). Not binary.
- **xeyezHal**: v2.5.0.7 837,960 B/61 exports vs v2.6.0.6 153,968 B/79
  (adds DRM/SecurityComm/CheckCode/CheckMatch/Check_Key/CalibNum/DeviceCam/
  DeviceClass/HardVersion/BlobAxis/SetClassifier/CmosReset; removes none).
- **StereoCamera.dll v2.6** = StereoCameraBridge (Get3dSingle, Get3DObject,
  BlobFeature2ClassResult, RegisterScDataListener, SetBlobFeatureMaxWidth,
  SetClassifier, SetLens(SC_LENS_PARAM)). classifying.dll = ClassifyAndTrack/
  SetLensParam/SetStaticPoint_Param. **SVM .dat files are plain text**
  (V1 4-feature SVs; V4 2-D linear separators, dated to 2012-04-11).
- **Middleware exports mapped**: xEyeEngine (Init/GetxEyePipeData/
  BackxEyeData/SetxEyeProperty), xEye2DEngine, xEyeMsgEngine 38 exports
  (Start/Get/CancelHeartBeat, MouseModeSwitch, UseKeySim, UseMouseSim,
  Vibrate, SetxEyeEngineCallBack), xEyeUserParamRebuild = xEye3DT2D;
  iDongAction.dll v3.0 64 exports + iDongActionCallBack (action3DMouse/
  Circle/Fighting/Gesturing/Hammer/Head/Hitting/Lining/ResetPos/Running,
  keyDown/keyUp); **iDongMarkManager 10-step pipeline** (Set1st..10thStep*;
  tunables MergeThreadHold, missFrameThreadhold, MaxTracingBall/LineDis,
  ForeCastRule/AdjustType, SrcSelect/CorrectType,
  SetTimeForExchange_Left_Right, 10th FourBallsHandLegYDis thresholds);
  iDongSimulator3.exe imports iDongAction+xEyeMsgEngine (replay oracle; logs
  show WM_IDONG_GETSTANDARDPOS, ball[0]/ball[10] model loading,
  SetProperty(setStandardPos), EnergePerMin config).
- **xEyeUsb.ini runtime params**: NeedHandleMatch=1, MonitorList=10,
  BgCount=5, LinkErrCnt=100, disturbance grades 0/3/7/10/11,
  Green/Yellow/Red=20/35/35, NoiseLevel=50 (test noise!), RingSize=200,
  DevicePlatform=2, SimPlatform=2, USBComFPS=35.
- **Negatives re-confirmed**: CapStill=screen capture, Camset=UVC webcam UI,
  RHCC family=audio/chat (Borland) + RHCCEXTV codec blob, settings.sol
  defaultcamera="USB 视频设备" (UVC webcam).
- **licence.txt** (GBK): clause 2 no-RE (Taishan Online/Toltech). Private
  interoperability research, public plain-HTTP downloads, DRM documented not
  attacked. Text preserved in archive.

## Files produced this turn (all in /home/z/my-project/download/)

- `idong100_camera_research_turn12.pdf` (17 pp) — main deliverable.
- `idong100_research_turn12_2026-11-23.zip` — PDF + scripts + reports +
  manifests + FINDINGS + this backup.
- `chat_context_backup_2026-11-23_turn12.md` — this file.

## Analysis artifacts (in analysis/, NOT all in the zip — re-fetchable)

- `analysis/reports/`: all_extracted_sha256.tsv (2,475 rows),
  pe_inventory.txt (933 PEs), firmware_report.txt, config_dumps.txt,
  version_diff.txt. `analysis/manifests/`: fetched_members.tsv,
  extracted_sportsking.tsv (178 rows), fashion_A_zip_listing.txt,
  slice1_fetch.log, extract_sportsking.log.
- Extracted binaries: `analysis/remote/` (9 members incl. slice 1),
  `analysis/nsis/xeyez_pc_v{2.5.0.7,2.6.0.6}/` (2,296 files),
  `analysis/extracted_sportsking/` (170 files). Disk-heavy; the SHA manifest
  + 2 URLs reproduce them exactly.
- Figures: `analysis/figures/` (diagram_hardware.png, diagram_hoststack.png,
  extraction_stats.png, cover.pdf, report_body.pdf).
- Research tree: `research/idong100/agents/12-extraction-sha/FINDINGS.md`
  (also in zip). GitHub repo thomas7475/Idong100 was cloned (public, no PAT
  needed) and is BEHIND local state (has agents 01/02/09 only); turns 10-12
  outputs are NOT pushed — user never asked to push; ask before pushing.

## Pipeline notes (rebuild if needed)

- PDF: `python3 scripts/gen_report.py` (body, ReportLab, TocDocTemplate +
  multiBuild, roman TOC/arabic body via `_body_start` marker) →
  `python3 scripts/merge_report.py` (cover via html2poster.js --width 794px
  + pypdf normalize to A4) → meta.brand → pdf_qa.py.
- Cover: Template 07 Crystal Blue (fixed palette; cover.html in scripts/).
  Diagrams: scripts/diagram_*.html → make_figures.py (playwright
  screenshot @2×) — model had NO image input this session; verified pages
  programmatically (non-white ratios, text extraction).
- 7zz binary at /tmp/7zip/7zz (from GitHub ip7z/7zip 24.08; 7-zip.org 404s
  here; no root for apt). Re-download if sandbox recycled.
- Inno extract regex gotcha: bash single quotes + Python regex — use
  `'\\Lobby\\[^\\]+\.dll$'` (2 backslashes per literal backslash).
- pefile + capstone installed via pip this turn; reinstall after recycle.

## Status / likely next turns

1. Live-device P0: replay boot + scripts verbatim, then image-mode attempt
   (0x02=0x0B + 30fps_Global_image_2sensor_diff_1LED.dat + GET_IMG_START).
2. 8051 disasm of app_m8051_*.hex; Blackfin LDR framing + ep0_handler.
3. USBPcap capture of vendor session (write-path gate). Toltech SDK email
   thread still open (serial AB460DC8BA12C6840B49CD5786C329DD).
4. Safety rules in force: read-only EP 0x82 until sure; no speculative
   EP 0x02 writes; NEVER write the T24C16A EEPROM; no full cargo builds.
