#!/usr/bin/env python3
"""turn50: parse the run1 USBPcap1.pcap for 7F 55 session-plane traffic.

USBPcap pcap: linktype USBPCAP (249). Per-packet pseudo-header:
  u16 headerLen; u64 irpId; u32 status; u16 function; u8 info;
  u16 bus; u16 device; u8 endpoint; u8 transfer; u32 dataLength
(control transfers prepend a setup stage with an 8-B SETUP packet).
"""
import struct
import sys
from collections import defaultdict

PCAP = sys.argv[1] if len(sys.argv) > 1 else "/tmp/idong_capture/run1/USBPcap1.pcap"

data = open(PCAP, "rb").read()
magic = data[:4]
assert magic in (b"\xd4\xc3\xb2\xa1", b"\xa1\xb2\xc3\xd4", b"\x4d\x3c\xb2\xa1", b"\xa1\xb2\x3c\x4d"), magic
little = magic in (b"\xd4\xc3\xb2\xa1", b"\x4d\x3c\xb2\xa1")
fmt = "<" if little else ">"

pkts = []
off = 24
n = 0
while off + 16 <= len(data):
    ts_s, ts_us, incl, orig = struct.unpack(fmt + "IIII", data[off:off + 16])
    off += 16
    pkt = data[off:off + incl]
    off += incl
    n += 1
    if len(pkt) < 27:
        continue
    (hlen, irp, status, func, info, bus, dev, ep, xfer, dlen) = struct.unpack(
        "<HQIHBHHBBI", pkt[:27])
    payload = pkt[hlen:hlen + dlen]
    pkts.append((ts_s + ts_us / 1e6, bus, dev, ep, xfer, func, info, payload))

print(f"{n} packets, {len(pkts)} parsed")
sess = defaultdict(int)
fpga_frames = []
big_out = []
session_cmds = defaultdict(list)
t0 = pkts[0][0] if pkts else 0
for t, bus, dev, ep, xfer, func, info, payload in pkts:
    d = payload
    if xfer == 1:  # bulk
        if ep & 0x80 == 0:  # OUT
            sess["bulk_out_bytes"] += len(d)
            if len(d) >= 8 and d[:2] == b"\x7f\x55":
                cmd = struct.unpack("<H", d[2:4])[0]
                session_cmds[("bulk", cmd)].append((round(t - t0, 2), len(d), d[:16].hex()))
            if len(d) > 4096:
                big_out.append((round(t - t0, 2), ep, len(d), d[:16].hex()))
    elif xfer == 2:  # control
        if len(d) >= 8 and d[:2] == b"\x7f\x55" and (ep & 0x7f) == 0:
            cmd = struct.unpack("<H", d[2:4])[0]
            session_cmds[("ctrl", cmd)].append((round(t - t0, 2), len(d), d[:16].hex()))

print(f"bulk OUT bytes total: {sess['bulk_out_bytes']}")
print("\n== 7F 55 commands seen (xfer, u16cmd) -> count, first/last t ==")
for key, lst in sorted(session_cmds.items()):
    print(f"  {key}: {len(lst)}  t {lst[0][0]}..{lst[-1][0]}  e.g. {lst[0][2]}")
print("\n== bulk OUT writes > 4096 B ==")
for row in big_out[:40]:
    print(f"  t={row[0]} ep=0x{row[1]:02x} len={row[2]} head={row[3]}")
