#!/usr/bin/env python3
"""Turn-50 research: who consumes xEyeUsb.ini `BitFile` = A_a_v4.4_BG0313.bit?

Scans every PE binary of the v2.6.0.6 app dir for ASCII/GBK strings
mentioning BitFile / BG0313 / .bit / fpga / xilinx / loader keywords,
and reports which module carries them (file + RVA of the string).
"""
import re
import sys
from pathlib import Path

APP = Path("/tmp/vendor/lobby/$_6_")
KEYS = [
    (re.compile(rb"BitFile", re.I), "BitFile"),
    (re.compile(rb"BG0313", re.I), "BG0313"),
    (re.compile(rb"A_a_v4", re.I), "A_a_v4"),
    (re.compile(rb"\.bit\b"), ".bit"),
    (re.compile(rb"FPGA", re.I), "fpga"),
    (re.compile(rb"Xilinx", re.I), "xilinx"),
    (re.compile(rb"ens_code", re.I), "ens_code"),
    (re.compile(rb"\.ldr\b"), ".ldr"),
    (re.compile(rb"xEyeUsb\.ini", re.I), "xEyeUsb.ini"),
    (re.compile(rb"GetPrivateProfileString"), "GetPrivateProfileStringW/A"),
]


def strings(data: bytes, minlen: int = 4):
    """Yield (offset, ascii_string). Also GBK/UTF-16LE runs kept raw."""
    cur = bytearray()
    start = 0
    for i, b in enumerate(data):
        if 0x20 <= b < 0x7F:
            if not cur:
                start = i
            cur.append(b)
        else:
            if len(cur) >= minlen:
                yield start, bytes(cur)
            cur = bytearray()
    if len(cur) >= minlen:
        yield start, bytes(cur)


def gbk_strings(data: bytes, minlen: int = 2):
    """GBK double-byte strings (the vendor's Chinese logs)."""
    out = []
    i = 0
    start = None
    while i < len(data) - 1:
        b = data[i]
        if 0x81 <= b <= 0xFE and 0x40 <= data[i + 1] <= 0xFE:
            if start is None:
                start = i
            i += 2
        else:
            if start is not None and i - start >= minlen:
                try:
                    out.append((start, data[start:i].decode("gbk")))
                except Exception:
                    pass
            start = None
            i += 1
    return out


def main():
    files = sorted(
        p for p in APP.iterdir()
        if p.suffix.lower() in {".dll", ".exe"} and p.is_file()
    )
    for f in files:
        data = f.read_bytes()
        hits = {label: [] for _, label in []}
        found = {}
        for rx, label in KEYS:
            for off, s in strings(data, 4):
                if rx.search(s):
                    found.setdefault(label, []).append((off, s.decode("ascii", "replace")))
        # also raw (non-string-table) pattern hits for BitFile
        for rx, label in KEYS:
            for m in rx.finditer(data):
                found.setdefault(label + " (raw)", []).append((m.start(), ""))
        interesting = {k: v for k, v in found.items() if k.startswith(("BitFile", "BG0313", "A_a_v4", ".bit", "fpga", "xilinx", "ens_code", ".ldr"))}
        if not interesting:
            continue
        print(f"=== {f.name} ({len(data)} B) ===")
        for label, lst in sorted(interesting.items()):
            offs = sorted(set(o for o, _ in lst))[:8]
            print(f"  {label}: {len(lst)} hit(s) at {[hex(o) for o in offs]}")
            if label in ("BitFile", "BG0313", "A_a_v4", ".bit", "ens_code", ".ldr"):
                for off, s in lst[:6]:
                    print(f"    {hex(off)}: {s!r}")
        # GBK strings that name stages around fpga/bit
        for off, s in gbk_strings(data, 4):
            if any(k in s for k in ("加载", "比特", "位流", "配置", "启动", "复位", "下载")):
                pass  # printed selectively below
        # targeted GBK context dump for FPGA words
        for off, s in gbk_strings(data, 2):
            if any(k in s for k in ("FPGA", "fpga", "Fpga")):
                print(f"    gbk {hex(off)}: {s!r}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
