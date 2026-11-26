#!/usr/bin/env python3
"""Full timeline of the vendor-interface (ep02/82) 7f55 protocol during streaming."""
import struct
from collections import defaultdict

PCAP = "/home/z/my-project/work/cap8/idong_capture/run1/USBPcap1.pcap"

def u16(b, o): return b[o] | (b[o+1] << 8)
def u32(b, o): return (b[o] | (b[o+1] << 8) | (b[o+2] << 16) | (b[o+3] << 24)) & 0xFFFFFFFF

data = open(PCAP, "rb").read()
pos = 24
evs = []
t0 = None
while pos + 16 <= len(data):
    incl = u32(data, pos + 8)
    if incl < 1 or incl > 8*1024*1024 or pos + 16 + incl > len(data):
        break
    off = pos + 16
    pkt = data[off:off+incl]
    ts = u32(data, pos) + u32(data, pos+4) / 1e6
    if t0 is None: t0 = ts
    pos += 16 + incl
    if len(pkt) < 27: continue
    hlen = u16(pkt, 0)
    if hlen < 27 or hlen > len(pkt): continue
    ep = pkt[21]; tr = pkt[22]; dlen = u32(pkt, 23)
    d = hlen
    avail = min(len(pkt) - hlen, dlen)
    if avail <= 0: continue
    p = pkt[d:d+avail]
    t = ts - t0
    if tr in (1, 3) and (ep & 0x7f) == 2 and avail >= 2:
        head = p[:2]
        if head == b"\x7f\x55":
            if not (ep & 0x80):
                cmd = u16(p, 2)
                evs.append((t, "OUT", cmd, p))
            else:
                cmd = u16(p, 2)
                evs.append((t, "IN", cmd, p))
        else:
            evs.append((t, "OUT?" if not (ep & 0x80) else "IN?", None, p))

print(f"{len(evs)} vendor-interface frames")
hist = defaultdict(int)
for t, d, cmd, p in evs:
    hist[(d, cmd)] += 1
print("histogram:", {f"{d} {cmd:04x}" if cmd else d: c for (d, cmd), c in sorted(hist.items())})

# non-poll frames
print("\nnon-0502 frames:")
for t, d, cmd, p in evs:
    if cmd != 0x0502:
        cs = f"{cmd:04x}" if cmd is not None else "----"
        print(f"  {t:8.2f}s {d} cmd={cs} len={len(p)} {p[:48].hex()}")

# 0502 replies: parse structure
print("\n0502 reply anatomy (first 6 with data):")
shown = 0
last_ctr = None
ctr_series = []
for t, d, cmd, p in evs:
    if d != "IN" or cmd != 0x0502 or len(p) < 16:
        continue
    # fields
    ftype = p[4]
    flags = p[5]
    w0 = u16(p, 6)
    alen = p[8] if len(p) > 8 else None
    ctr = u32(p, 9) if len(p) > 12 else None
    if shown < 6:
        print(f"  {t:7.2f}s full: {p.hex()}")
        print(f"    [7f55][cmd 0502][type {ftype:02x}][u8 {flags:02x}][u16 {w0:04x}]"
              f"[alen {alen}] next-u32 {ctr:08x}" if ctr is not None else "")
        shown += 1
    if ctr is not None:
        ctr_series.append((t, ctr, p.hex()))

print(f"\ncounter series: {len(ctr_series)} samples")
if ctr_series:
    diffs = [(b - a) for (t1, a, _), (t2, b, _) in zip(ctr_series, ctr_series[1:])]
    print("first 15 diffs:", diffs[:15])
    print("min/max diff:", min(diffs), max(diffs))

# rate of 0502 polls
outs = [t for t, d, cmd, p in evs if d == "OUT" and cmd == 0x0502]
if len(outs) > 2:
    print(f"\n0502 OUT polls: {len(outs)}, span {outs[-1]-outs[0]:.1f}s -> {len(outs)/(outs[-1]-outs[0]):.2f}/s")
# last poll time vs stream end
print(f"last OUT poll at {outs[-1]:.1f}s" if outs else "no polls")
