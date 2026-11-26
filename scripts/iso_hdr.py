#!/usr/bin/env python3
"""Find UVC payload headers inside iso records; measure payload period,
FID toggling, frame sizes, and the image data layout."""
import struct
from collections import defaultdict

PCAP = "/home/z/my-project/work/cap8/idong_capture/run1/USBPcap1.pcap"

def u16(b, o): return b[o] | (b[o+1] << 8)
def u32(b, o): return (b[o] | (b[o+1] << 8) | (b[o+2] << 16) | (b[o+3] << 24)) & 0xFFFFFFFF

data = open(PCAP, "rb").read()
pos = 24
recs = []
while pos + 16 <= len(data):
    incl = u32(data, pos + 8)
    if incl < 1 or incl > 8*1024*1024 or pos + 16 + incl > len(data):
        break
    off = pos + 16
    pkt = data[off:off+incl]
    pos += 16 + incl
    if len(pkt) < 27: continue
    hlen = u16(pkt, 0)
    if hlen < 27 or hlen > len(pkt): continue
    ep = pkt[21]; tr = pkt[22]; dlen = u32(pkt, 23)
    d = hlen
    avail = min(len(pkt) - hlen, dlen)
    if avail <= 0: continue
    p = pkt[d:d+avail]
    if tr == 0 and ep & 0x80:
        recs.append(p)

print(f"{len(recs)} iso records")

# scan records for candidate UVC headers: byte0 in 2..12, byte0<=? flags byte1 with
# only known bits, at position 0 and then search for next occurrence of a valid header
r = recs[0]
hdr_positions = []
i = 0
while i < len(r) - 12:
    hl = r[i]
    flags = r[i+1]
    if 2 <= hl <= 12 and (flags & ~0xFF) == 0 and (flags & 0x03) in (0, 1, 2, 3):
        # candidate; require SCR/PTS consistency: if flags has PTS(0x04)+SCR(0x08), hl >= 12
        ok = True
        if (flags & 0x04) and hl < 6: ok = False
        if (flags & 0x08) and hl < 8: ok = False
        if ok and (flags & 0xFC) in (0x8c, 0x8d, 0x04, 0x08, 0x0c, 0x00, 0x02):
            hdr_positions.append((i, hl, flags))
            i += hl + 1016  # guess: skip one payload (header + data)
            continue
    i += 1

print("candidate positions in record 0:", hdr_positions[:20])

# More robust: search raw occurrences of b'\x0c\x8c' and b'\x0c\x8d'
occs_c = []
i = 0
while True:
    j = r.find(b'\x0c\x8c', i)
    if j < 0: break
    occs_c.append(j); i = j + 1
occs_d = []
i = 0
while True:
    j = r.find(b'\x0c\x8d', i)
    if j < 0: break
    occs_d.append(j); i = j + 1
print(f"occurrences of 0c8c: {len(occs_c)} first 12: {occs_c[:12]}")
print(f"occurrences of 0c8d: {len(occs_d)} first 12: {occs_d[:12]}")

# check spacing
if len(occs_c) > 1:
    diffs = [b - a for a, b in zip(occs_c, occs_c[1:])]
    print("0c8c spacings:", diffs[:16])
if len(occs_d) > 1:
    diffs = [b - a for a, b in zip(occs_d, occs_d[1:])]
    print("0c8d spacings:", diffs[:16])

# hypothesis: one UVC payload per pcap record. Check record starts.
starts = set()
fid_seq = []
for r in recs[:200]:
    hl = r[0]
    flags = r[1]
    starts.add((hl, flags))
    fid_seq.append(flags & 1)
print("record-start (hl, flags) values:", sorted(starts))
print("FID sequence (first 60 records):", fid_seq[:60])
fid_toggles = sum(1 for a, b in zip(fid_seq, fid_seq[1:]) if a != b)
print(f"FID toggles in first 200: {fid_toggles}")

# record size constancy
sizes = set(len(r) for r in recs)
print("record sizes:", sorted(sizes)[:10])

# What's after the 12-byte header: look for EOF-marked payload (flags bit1)
eof_records = [i for i, r in enumerate(recs) if r[1] & 0x02]
print(f"records with EOF flag: {len(eof_records)} first: {eof_records[:10]}")

# PTS values from headers across records -> timestamps
pts_vals = []
for i, r in enumerate(recs[:40]):
    hl, flags = r[0], r[1]
    pts = struct.unpack("<I", r[2:6])[0] if flags & 0x04 else None
    scr = r[6:12].hex() if flags & 0x08 else None
    pts_vals.append(pts)
    if i < 12:
        print(f"rec{i:4d} hl={hl} flags={flags:#04x} PTS={pts} SCR={scr} next-bytes={r[hl:hl+8].hex()}")
