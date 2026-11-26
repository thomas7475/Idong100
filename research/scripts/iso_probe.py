#!/usr/bin/env python3
"""Determine USBPcap iso sub-packet structure inside the 63960-B records,
then extract complete video frames and save them as images."""
import struct
import sys
from collections import defaultdict

PCAP = "/home/z/my-project/work/cap8/idong_capture/run1/USBPcap1.pcap"

def u16(b, o): return b[o] | (b[o+1] << 8)
def u32(b, o): return (b[o] | (b[o+1] << 8) | (b[o+2] << 16) | (b[o+3] << 24)) & 0xFFFFFFFF

data = open(PCAP, "rb").read()
pos = 24
records = []
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
        records.append(p)

print(f"iso records: {len(records)}")
r0 = records[0]
print(f"record0 len={len(r0)}")
print("first 64 bytes:", r0[:64].hex())
print("bytes 64..128 :", r0[64:128].hex())

# Hypothesis A: descriptors interleaved: [desc12][data][desc12][data]...
# USBPcap iso descriptor = [u32 offset][u32 length][u32 status]
# scan: offset starts 0, increments by length
def try_parse_A(rec):
    subs = []
    o = 0
    while o + 12 <= len(rec):
        off_v, len_v, st = struct.unpack("<III", rec[o:o+12])
        subs.append((o, off_v, len_v, st))
        o += 12 + len_v
    return subs

subs = try_parse_A(r0)
print("\nHypothesis A ([desc][data] pairs):")
for s in subs[:8]:
    o, off_v, len_v, st = s
    print(f"  desc@{o}: offset={off_v} length={len_v} status={st:#x}")
print(f"  ... {len(subs)} sub-packets, sum data = {sum(s[2] for s in subs)}")

# Hypothesis B: all descriptors at the end
# Hypothesis C: [data][desc] pairs (trailing descriptor)
def try_parse_C(rec, n_expect=None):
    subs = []
    o = 0
    while o < len(rec):
        # desc is the LAST 12 bytes of each block
        block_end = None
        # try: find length from desc at unknown position -> instead scan forward
        # assume desc at rec[o:o+12] describes PREVIOUS data; first block has no desc
        break
    return subs

# Just look for repeating 12-byte windows where u32@+4 (length) is plausible (0..3072)
# and u32@+8 (status) == 0
print("\nscan for descriptor-like positions (status==0, length<=3072, offset plausible):")
hits = 0
for o in range(0, min(len(r0), 63960)):
    if o + 12 > len(r0): break
    a, b, c = struct.unpack("<III", r0[o:o+12])
    if b <= 3072 and b >= 256 and c == 0 and a <= 63960:
        if hits < 30:
            print(f"  @{o}: offset={a} length={b} status={c}")
        hits += 1
print(f"  total hits: {hits}")
