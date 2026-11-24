# ZCode Chat Context Backup — 2026-11-24 (turn 20)

**How to use this file:** attach/upload it in a future session so the
assistant can restore full context of this conversation and continue
where it left off. Written by the assistant itself as a session snapshot.

---

## User's standing rules (apply to EVERY reply)

1. **English only.**
2. **Zip backup of generated files** in `/home/z/my-project/download/`.
3. **Chat-context backup** (this kind of file) every time, in
   `/home/z/my-project/download/`.
4. **NEVER run full `cargo` builds** — RAM limit. `cargo check`,
   single bin/example builds, or lowered parallelism only.
5. **Git state tracking:** https://github.com/thomas7475/Idong100.git,
   branches `project/state` and `chat/context` — updated at end of
   every turn. PAT re-supplied by user each turn (embedded in this
   turn's first message; do not print it).
6. **(turn 18) NO PowerShell in deliverables** — pure cmd `.bat` only.

## Turn 20 request and outcome

User: ran check_gate.bat (all OK, platform gate SATISFIED), then
`install_patched.bat`; "the patched exe does not work" + screenshot
`idong-xeyez-setup3.png` = **NSIS Error — "Installer integrity check
has failed. Common causes include incomplete download and damaged
media."**

**Root cause (turn 19's bug):** the repack rebuilt the compressed
stream but (a) left the firstheader's `length_of_all_following_data`
(x2) stale and (b) never recomputed/aligned the stored CRC field, so
the NSIS 2.44 stub's loadHeaders check failed BEFORE the OS-gate was
ever reached.

**Root cause recovery method:** emulated the ORIGINAL stub's
loadHeaders (function 0x2cc4) with Unicorn (`scripts/
emulate_stub_check20.py`), redirecting all IAT slots to `ret 4N`
gadgets and hooking them for file I/O. The emulation PASSED on the
original and revealed the exact check:

```
firstheader fh (28B) = {flags, 0xDEADBEEF, 'NullsoftInst'(12B), x1, x2}
check: crc32(file[512 .. fh + x2 - 4)) == u32 at (fh + x2 - 4)
```

- The first 512 bytes of the file are EXCLUDED; the PE stub,
  firstheader (including the x2 dword!), props and the whole solid
  LZMA stream are covered.
- The stored field sits at `fh+x2-4` (4 bytes between the stream end
  and the LZMA end-marker area).
- NSIS's CRC32 is the plain standard CRC-32 (per-chunk double-
  complement accumulation is equivalent to one-shot zlib crc32).

**Fix (turn 20, `scripts/rebuild_patched_installers20.py`):**

1. Decompress the solid stream, patch the embedded VersionCheck.dll
   (same 5 gate bytes as turn 19), recompress (identical LZMA filters).
2. Recompute `x2` so `fh+x2-4` = new stream end, write the new x2 into
   the firstheader (the new x2 bytes are INSIDE the CRC region!).
3. Append the CRC32 of `file[512 .. new_stream_end)` as the 4-byte field.
4. Zero the stale Authenticode security-dir entry; keep the cert tail
   (unused by NSIS).
5. Verification: (a) full LZMA round-trip equality; (b) DLL patch bytes
   at the known offsets; (c) stub-check simulation in Python; (d) the
   REAL stub code emulated against the patched exe — `FINAL CRC CHECK:
   computed == stored, match=True`, execution continues past the check.

**New deliverables (download/idong100_p2_usbpcap_kit/software/):**

- `xeyez_setup_patched.exe` — 82,842 B —
  sha256 `b2436be7d4208fcc8e6d2b4304b0d4c324bdcb1169120ff1deaba4c165ded480`
- `xeyez_alone_patched.dat` — 23,638,139 B —
  sha256 `af070bbf0b2afe2581b8985718060bf06c776afc260e7e49b7410863ed85985e`
- Turn-19 versions (76,316 B exe / 23,631,617 B dat) are BROKEN — replace.

**User action:** copy the new kit (or the two patched files) over the
old ones and run `install_patched.bat` (as admin) again. Expect NO
NSIS Error and NO OS dialog. Then README_CAPTURE.md §3 → capture.

## Operational rules (cumulative)

- `git rev-parse --show-toplevel` inside research/idong100 before ANY git op.
- UTF-16LE vs GBK: search PE strings in BOTH encodings (turn-19 lesson).
- NSIS solid repack recipe (turn-20, CORRECTED): find `NullsoftInst` =>
  firstheader F = find-8; x2 = u32@F+24; props = F+28..F+33; stream =
  props + `\xff`*8 + file[F+33:]; FORMAT_ALONE decompress; patch;
  recompress same filters; payload = blob[13:]; new_x2 = (F+33+len
  (payload) - F) + 4; region = file[512:F+24] + pack(new_x2) + props +
  payload; crc = crc32(region); new file = file[:F+24] + pack(new_x2) +
  props + payload + pack(crc) + old_tail; zero PE security dir.
- Unicorn stub-emulation recipe: map image+stack+thunk area; rewrite ALL
  IAT slots (INCLUDING ordinal imports!) to `ret 4N` gadgets; hook the
  gadget addresses for side effects (ReadFile from real file bytes,
  GetFileSize, SetFilePointer — arg order (h, dist, hi, method));
  implement CharPrevA (backward walk!) / CharNextA / lstrcpynA / wsprintfA
  (cdecl, 0-pop) / GetTempPathA / GetModuleFileNameA; start directly at
  loadHeaders (0x2cc4 for this stub build) with a fresh stack.
- pefile + capstone + unicorn need `pip install` after each sandbox recycle.

## Repo state (turn 20)

- main: 77b3872 (turn-20 integrity fix) on top of 27b5d55 (turn 19).
- project/state: 64bd996.
- chat/context: this file.
- Kit: download/idong100_p2_usbpcap_kit.zip (22 files, 166 MB) rebuilt.

## Where the project stands

- Phase 1 (pure core) COMPLETE on main.
- Phase 2: P2.3 capture kit + platform-gate v2 (t18) + OS-gate patched
  installers (t19) + integrity CRC fix (t20). P2.1 read-only probe done;
  P2.2 (udev rule) open.
- NEXT: user re-runs install_patched.bat with the NEW files → full vendor
  install → README_CAPTURE.md §3/§5 capture session → boot.pcapng →
  byte-for-byte gate comparison on the host.

## Environment notes (persistent facts)

- analysis/ trees survived (nsis payloads, reports) — reuse, don't re-extract.
- Uploads in /home/z/my-project/upload/ (read-only intent).
- Rust stable 1.100.0 in ~/.cargo.
