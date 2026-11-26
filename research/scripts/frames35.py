#!/usr/bin/env python3
"""Walk ALL UVC sub-payloads, track FID/EOF frame boundaries, and dump frames."""
import struct
from collections import defaultdict

PCAP = "/home/z/my-project/work/cap8/idong_capture/run1/USBPcap1.pcap"

def u16(b, o): return b[o] | (b[o+1] << 8)
def u32(b, o): return (b[o] | (b[o+1] << 8) | (b[o+2] << 16) | (b[o+3] << 24)) & 0xFFFFFFFF

data = open(PCAP, "rb").read()
pos = 24
recs = []
ts_list = []
while pos + 16 <= len(data):
    incl = u32(data, pos + 8)
    if incl < 1 or incl > 8*1024*1024 or pos + 16 + incl > len(data):
        break
    off = pos + 16
    pkt = data[off:off+incl]
    ts = u32(data, pos) + u32(data, pos+4) / 1e6
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
        recs.append(p); ts_list.append(ts)

# walk sub-payloads
subs = []          # (rec_i, sub_i, offset_in_rec, hl, flags, data_len, pts)
for i, r in enumerate(recs):
    o = 0
    k = 0
    while o + 2 <= len(r):
        hl = r[o]
        flags = r[o+1]
        if hl < 2 or o + hl > len(r):
            break
        dlen = min(2400, len(r) - o - hl)
        pts = struct.unpack("<I", r[o+2:o+6])[0] if (flags & 0x04) else None
        subs.append((i, k, o, hl, flags, dlen, pts))
        o += hl + dlen
        k += 1

print(f"records {len(recs)}, sub-payloads {len(subs)}")
per_rec = defaultdict(int)
for s in subs: per_rec[s[0]] += 1
print("sub-payloads per record (first 10):", [per_rec[i] for i in range(10)])
print("flags values seen:", sorted(set(s[4] for s in subs)))
eof_subs = [s for s in subs if s[4] & 0x02]
print(f"sub-payloads with EOF flag: {len(eof_subs)}")
if eof_subs:
    for s in eof_subs[:10]:
        print(f"  rec{s[0]} sub{s[1]} off{s[2]} flags={s[4]:#04x} len={s[5]} pts={s[6]}")

# FID transitions
fid_series = [(s[0], s[1], s[4] & 1, s[6]) for s in subs]
trans = []
prev = fid_series[0][2]
for idx, (ri, si, fid, pts) in enumerate(fid_series):
    if fid != prev:
        trans.append((ri, si, idx, fid, pts))
    prev = fid
print(f"FID transitions: {len(trans)} at (rec,sub) (first 15):")
for t in trans[:15]:
    print(f"  rec{t[0]} sub{t[1]} global-sub{t[2]} -> FID {t[3]} pts={t[4]}")

# frame sizes between FID transitions (in sub-payload data bytes)
frame_bounds = [0] + [t[2] for t in trans] + [len(subs)]
sizes = []
for a, b in zip(frame_bounds, frame_bounds[1:]):
    sz = sum(s[5] for s in subs[a:b])
    sizes.append((a, b, sz))
print(f"frames: {len(sizes)}, first 12 sizes: {[s[2] for s in sizes[:12]]}")
if sizes:
    uniq = defaultdict(int)
    for s in sizes: uniq[s[2]] += 1
    print("frame size histogram:", dict(sorted(uniq.items())))

# PTS per frame
pts_seq = []
for a, b in zip(frame_bounds, frame_bounds[1:]):
    p = [s[6] for s in subs[a:b] if s[6] is not None]
    pts_seq.append(p[0] if p else None)
deltas = [(pts_seq[i+1] - pts_seq[i]) if pts_seq[i] and pts_seq[i+1] else None
          for i in range(len(pts_seq)-1)]
print("PTS deltas (first 15):", deltas[:15])
valid = [d for d in deltas if d]
if valid:
    print(f"PTS delta min/max/avg: {min(valid)}/{max(valid)}/{sum(valid)//len(valid)}")
    print(f"-> frame rate from PTS delta (100ns units): {1e7/ (sum(valid)//len(valid)):.2f} fps")

# save first 6 frames as raw dumps
import os
os.makedirs("/home/z/my-project/work/cap8/frames", exist_ok=True)
for fi, (a, b) in enumerate(list(zip(frame_bounds, frame_bounds[1:]))[:6]):
    buf = bytearray()
    for s in subs[a:b]:
        buf += recs[s[0]][s[2]+s[3]:s[2]+s[3]+s[5]]
    with open(f"/home/z/my-project/work/cap8/frames/frame{fi}_{len(buf)}.raw", "wb") as f:
        f.write(bytes(buf))
    print(f"frame{fi}: {len(buf)} bytes")
