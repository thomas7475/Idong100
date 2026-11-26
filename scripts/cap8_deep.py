#!/usr/bin/env python3
"""Deep analysis of the turn-35 capture: UVC negotiation + ep85 iso video."""
import struct
import sys
from collections import defaultdict

PCAP = "/home/z/my-project/work/cap8/idong_capture/run1/USBPcap1.pcap"

def u16(b, o): return b[o] | (b[o+1] << 8)
def u32(b, o): return (b[o] | (b[o+1] << 8) | (b[o+2] << 16) | (b[o+3] << 24)) & 0xFFFFFFFF

data = open(PCAP, "rb").read()
print(f"file {len(data)} bytes")

pos = 24
ctrl = []           # (ts, summary, payload bytes)
iso = []            # (ts, ep, payload)
bulk = []           # (ts, ep|0x80, dir, payload)
set_iface = []
enum_devs = []
t0 = None

while pos + 16 <= len(data):
    incl = u32(data, pos + 8)
    if incl < 1 or incl > 8*1024*1024 or pos + 16 + incl > len(data):
        break
    ts = u32(data, pos) + u32(data, pos+4) / 1e6
    if t0 is None: t0 = ts
    off = pos + 16
    pkt = data[off:off+incl]
    pos += 16 + incl
    if len(pkt) < 27: continue
    hlen = u16(pkt, 0)
    if hlen < 27 or hlen > len(pkt): continue
    dev = u16(pkt, 19); ep = pkt[21]; tr = pkt[22]; dlen = u32(pkt, 23)
    d = hlen
    avail = min(len(pkt) - hlen, dlen)
    if avail <= 0: continue
    p = pkt[d:d+avail]
    t = ts - t0
    if tr == 2:
        if len(p) >= 2:
            b0, b1 = p[0], p[1]
            if b0 == 0x12 and b1 == 0x01 and len(p) >= 18 and u16(p, 8) == 0x0471:
                enum_devs.append((t, f"desc dev{dev} VID 0471 PID {u16(p,10):04x}"))
            elif b0 == 0x21 and b1 == 0x01:  # SET_CUR
                val = u16(p, 2); idx = u16(p, 4)
                cs = (val >> 8) & 0xff; ent = val & 0xff
                ctrl.append((t, f"SET_CUR cs={cs:02x} ent={ent:02x} if={idx}", p[8:]))
            elif b0 == 0x21 and b1 == 0x81:  # GET_CUR
                val = u16(p, 2); idx = u16(p, 4)
                cs = (val >> 8) & 0xff
                ctrl.append((t, f"GET_CUR cs={cs:02x} if={idx} wlen={u16(p,6)}", b""))
            elif b0 == 0x01 and b1 == 0x0b:
                alt = u16(p, 2); idx = u16(p, 4)
                set_iface.append((t, f"SET_INTERFACE if{idx} alt{alt}"))
            elif b0 == 0x0b and b1 == 0x01:
                # GET_INTERFACE reply maybe
                pass
            elif len(p) >= 8:
                ctrl.append((t, f"CTRL {b0:02x} {b1:02x} val={u16(p,2):04x} idx={u16(p,4):04x} len={avail}", p[8:12]))
    elif tr == 0:
        if ep & 0x80:
            iso.append((t, ep & 0x7f, p))
    elif tr in (1, 3):
        bulk.append((t, ep, p))

print(f"\n--- enum: {len(enum_devs)} ---")
for t, s in enum_devs[:10]: print(f"  {t:8.2f}s {s}")

print(f"\n--- control transfers: {len(ctrl)} (showing all) ---")
for t, s, pl in ctrl:
    hx = pl.hex() if pl else ""
    print(f"  {t:8.2f}s {s}  {hx[:96]}")

print(f"\n--- SET_INTERFACE: {len(set_iface)} ---")
for t, s in set_iface: print(f"  {t:8.2f}s {s}")

print(f"\n--- iso: {len(iso)} packets ---")
if iso:
    by_ep = defaultdict(int)
    for t, ep, p in iso: by_ep[ep] += len(p)
    for ep, n in sorted(by_ep.items()):
        print(f"  ep {ep:02x}: {n} bytes")
    # first few payload headers
    ep5 = [(t, p) for t, ep, p in iso if ep == 5]
    print(f"  ep 0x05 (0x85 IN): {len(ep5)} packets")
    print("  first 12 payloads (hex, up to 48 B):")
    for t, p in ep5[:12]:
        print(f"    {t:8.2f}s len={len(p):4d} {p[:48].hex()}")
    # payload size histogram
    hist = defaultdict(int)
    for t, p in ep5: hist[len(p)] += 1
    print("  payload size histogram:")
    for sz, c in sorted(hist.items())[:20]:
        print(f"    {sz:5d} B x {c}")

print(f"\n--- bulk: {len(bulk)} (showing first 30 non-trivial) ---")
shown = 0
for t, ep, p in bulk:
    if len(p) < 4: continue
    print(f"  {t:8.2f}s ep {ep:02x} {'IN ' if ep & 0x80 else 'OUT'} len={len(p):5d} {p[:40].hex()}")
    shown += 1
    if shown >= 30: break
