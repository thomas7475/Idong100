# Agent 12 — Full Re-extraction with SHA-256 + Camera Internals Report

Date: 2026-11-23 (turn 12). Everything re-acquired live from
`download.i-dong.com` (still up); every extracted file SHA-256-hashed.

## What this turn did

1. **`inno_bin_extract.py` updated** (user request): extraction now prints
   `SHA256 <hash>  MD5 <hash>  MD5_OK  <size>  <path>` for EVERY file,
   verifies the MD5 against the Inno data-entry checksum, appends rows to a
   TSV `--manifest`, and supports `--include REGEX` filters. Demonstrated on
   the Sports King installer: 178 records extracted, **178/178 MD5-verified,
   0 failed** (170 unique paths — 8 destination paths appear twice in the
   custom-build file table with identical bytes; all from slice 0).
2. **New `fetch_zip_members.py`**: ranged-GET fetcher for members of the
   1.5-1.7 GB vendor ZIPs (central directory + chunked member download with
   resume, SHA256/MD5 printed per member + manifest). Used to fetch 9 members
   (~267 MB incl. full `idong_setup-1.bin` slice) from the yundongwang-A and
   shishang-A archives.
3. **Both driver stacks re-extracted** with 7zz 24.08 from the NSIS payloads:
   v2.5.0.7 (fashion zip, 142 files) and v2.6.0.6 (sports-king zip, 145
   files). 2,475 files total in the unified SHA manifest
   (`analysis/reports/all_extracted_sha256.tsv`).
4. **Analysis** (scripts/analyze_extracted.py → analysis/reports/): PE
   inventory of 933 binaries, firmware parse, config dumps, version diff.

## Key new findings (all evidence-backed)

- **Mode switch PROVEN from vendor scripts**: FPGA register `0x02` =
  `0x0A` blob mode vs `0x0B` image mode. Blob script additionally programs
  blob-window regs `0x50-0x57`: 0x51/0x52 = 0x027C (636), 0x53/0x54 = 0x01D0
  (464) → **636x464 search window**, then `0x06,0x02` start.
- **Image mode = 2-sensor difference**: the v2.6 xeyezHal.ini defines four
  configs (blob/image × rolling/global; rolling entries commented out, files
  absent). The image FPGA script is literally named
  `30fps_Global_image_2sensor_diff_1LED.dat` — vendor's own label for the
  raw-image stream. Confirms turn-11 suspicion; dense-stereo scanner phase
  must handle differential frames or replace DSP firmware.
- **FPGA bitstream pinned**: work_top.ncd;HW_TIMEOUT=FALSE;UserID=0xFFFFFFFF,
  part 3s400aft256, 2010-04-06 10:33:41, payload 235,820 B.
- **Blackfin ens_code.ldr** (54,420 B): first block writes 4 B to
  0xFF800040; 66 plausible block headers → L1 data B targets dominate
  (51× 0xFF9xxxxx); 17 function pointers into 0xFFA03D1C-0xFFA0E6BE (code
  ≈ 43 KB); symbols ep0_handler / usb_init; UTF-16 "xeyez", "TOLtech".
  Exact LDR block framing NOT decoded (standard zerofill/init flag
  interpretations empirically rejected) — open item.
- **8051 images**: two Intel-HEX files (v2.6) vs one (v2.5):
  app_m8051_2500.hex 21,110 B / _2611.hex 21,362 B; reset vector at 0x0000,
  8 interrupt vectors at 0x8000-0x806D, code span 0x8073-0xD2A6/0xD3A2.
- **data0-10.dat are ASCII register-page scripts** (0xF0 = page selector;
  data0 = page 3 regs 0x00-0x42 …) — maps the whole sensor register space.
- **xeyezHal.dll**: v2.5.0.7 = 837,960 B / 61 exports; v2.6.0.6 = 153,968 B
  / 79 exports (+18: DRMRead/Write, SecurityComm, CheckCodeRead/Write,
  CheckMatch, Check_Key, CalibNumRead/Write, DeviceCamRead/Write,
  DeviceClassRead, HardVersion_Read/Write, BlobAxisRead/Write, SetClassifier,
  CmosReset; none removed).
- **StereoCamera.dll v2.6** = StereoCameraBridge C++ class (Get3dSingle,
  Get3DObject, BlobFeature2ClassResult, RegisterScDataListener,
  SetBlobFeatureMaxWidth, SetClassifier, SetLens(SC_LENS_PARAM)).
- **SVM classifiers are plain text**: SVM_Class_ball.dat = 4-feature
  support-vector model; V4 files (2012-04-11) = compact 2-D linear
  separators; retrained V1→V4 between 2011-07 and 2012-04.
- **Middleware API mapped** (Sports King slice): xEyeEngine/xEye2DEngine/
  xEyeMsgEngine (heartbeat, MouseModeSwitch, UseKeySim/UseMouseSim,
  Vibrate), xEyeUserParamRebuild.dll = xEye3DT2D reprojection;
  iDongAction.dll 3.0 (64 exports; iDongActionCallBack: action3DMouse/
  Circle/Fighting/Gesturing/Hammer/Head/Hitting/Lining/ResetPos/Running +
  keyDown/keyUp); **iDongMarkManager.dll 10-step tracking pipeline** (merge
  → clean → forecast → trace → adjust → velocity → source-select incl.
  SetTimeForExchange_Left_Right → 10th four-balls hand/leg Y-thresholds);
  iDongSimulator3.exe = replay oracle (logs: WM_IDONG_GETSTANDARDPOS,
  "Load analysis model[...] ball[0]/ball[10]", SetProperty(setStandardPos)).
- **Negative findings re-confirmed**: CapStill = screen capture; Camset =
  UVC webcam settings UI; RHCC family = audio/chat-room protocol (Borland),
  RHCCEXTV = codec blob; settings.sol defaultcamera = "USB video device"
  (the UVC webcam, not the sensor).
- **licence.txt** (GBK): clause 2 prohibits RE — private interoperability
  research on owner hardware, plain-HTTP public downloads, no DRM attacked;
  full text preserved in archive.

## Files produced

- `download/idong100_camera_research_turn12.pdf` — 17-page research report
  (cover T07, TOC, 10 chapters, 3 appendices, 8 tables, 3 figures, SHA
  manifest appendix; pdf_qa: PASS with 4 benign nested-callout warnings).
- `download/idong100_research_turn12_2026-11-23.zip` — all deliverables +
  scripts + reports + manifests + this file.
- `download/chat_context_backup_2026-11-23_turn12.md` — session backup.
- Scripts: inno_bin_extract.py (SHA printing), fetch_zip_members.py,
  analyze_extracted.py, make_figures.py, gen_report.py, merge_report.py,
  cover.html, diagram_hardware.html, diagram_hoststack.html.
- Reports: analysis/reports/{all_extracted_sha256.tsv, pe_inventory.txt,
  firmware_report.txt, config_dumps.txt, version_diff.txt} +
  manifests/{fetched_members.tsv, extracted_sportsking.tsv,
  fashion_A_zip_listing.txt}.

## Reproduction (from scratch, only needs the 2 URLs)

```bash
python3 scripts/fetch_zip_members.py <yundongwang_A_url> --member idong_setup.exe \
  --member idong_setup-1.bin --member 'dependent/xeyez_pc_v2.6.0.6/.*' \
  --outdir analysis/remote --manifest analysis/manifests/fetched_members.tsv
python3 scripts/fetch_zip_members.py <shishang_A_url> --member 'dependent/xeyez_pc_v2.5.0.7/.*' ...
7zz x ... ; python3 scripts/inno_bin_extract.py analysis/remote/idong_setup.exe \
  --extract analysis/extracted_sportsking --manifest ... --include <camera regexes>
python3 scripts/analyze_extracted.py
```

## Next turns (unchanged priorities, now with better tooling)

1. Live-device P0: replay boot + register scripts; then image-mode attempt
   (reg 0x02=0x0B + 30fps_Global_image_2sensor_diff_1LED.dat + GET_IMG_START).
2. 8051 disassembly of app_m8051_*.hex (clean vector layout known).
3. Blackfin LDR block framing (non-standard flag layout) → ep0_handler
   disasm.
4. USBPcap capture of vendor session (write-path gate).
5. Safety rules in force (read-only EP 0x82 until sure; no speculative
   0x02 writes; NEVER write T24C16A EEPROM; no full cargo builds).
