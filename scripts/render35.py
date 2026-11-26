#!/usr/bin/env python3
"""Strip UVC headers, render the video stream at several interpretations."""
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

# strip headers -> contiguous stream + per-record data
rec_data = []
for r in recs:
    o = 0
    buf = bytearray()
    while o + 2 <= len(r):
        hl = r[o]; flags = r[o+1]
        if hl < 2 or o + hl > len(r): break
        dlen = min(2400, len(r) - o - hl)
        buf += r[o+hl:o+hl+dlen]
        o += hl + dlen
    rec_data.append(bytes(buf))

print("per-record data bytes:", len(rec_data[0]), "(first 5 recs)",
      [len(x) for x in rec_data[:5]])

full = b"".join(rec_data)
print("total stream:", len(full), "bytes")

# write raw for offline checks
with open("/home/z/my-project/work/cap8/frames/stream_raw.bin", "wb") as f:
    f.write(full)

# PNG writer (pure python: use zlib)
import zlib

def png_gray(path, w, h, pix):
    raw = b"".join(bytes(pix[y*w:(y+1)*w]) for y in range(h))
    def chunk(typ, payload):
        c = typ + payload
        return struct.pack(">I", len(payload)) + c + struct.pack(">I", zlib.crc32(c) & 0xffffffff)
    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 0, 0, 0, 0)
    z = zlib.compress(raw, 6)
    with open(path, "wb") as f:
        f.write(sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", z) + chunk(b"IEND", b""))

def png_gray16be(path, w, h, pix16):
    # pack MSB only (high byte of each u16)
    raw = bytearray()
    for y in range(h):
        row = bytearray()
        for x in range(w):
            v = pix16[y*w+x]
            row.append((v >> 8) & 0xff)
        raw += b"\x00" + bytes(row)  # filter byte 0 per row handled below?
    # simpler: build with filter bytes
    out = bytearray()
    for y in range(h):
        out.append(0)
        for x in range(w):
            v = pix16[y*w+x]
            out.append((v >> 8) & 0xff)
    def chunk(typ, payload):
        c = typ + payload
        return struct.pack(">I", len(payload)) + c + struct.pack(">I", zlib.crc32(c) & 0xffffffff)
    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 0, 0, 0, 0)
    with open(path, "wb") as f:
        f.write(sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes(out), 6)) + chunk(b"IEND", b""))

# Frame A: first 190,908-B frame (3 records) rendered at various widths
f0 = rec_data[0] + rec_data[1] + rec_data[2]   # 190908 B
print("frame0 len", len(f0))

# Interpretation 1: 16-bit LE pixels, 640 wide -> 320 pixels... f0 = 95454 px -> not 640xN
# Interpretation 2: 8-bit, 640 wide -> 298.3 rows... not integral
# Interpretation 3: 8-bit, 1060 wide? 190908/1060 = 180.1 rows!! ~1060x180?
# Interpretation 4: 16-bit, 530 wide: 190908/2/530 = 180.1 rows
# Interesting: 5303*12... 1060 = 2*530. Hmm 190908/1060 = 180.1
for w in (640, 1060, 1280, 530, 318):
    rows = len(f0) // w
    if 40 <= rows <= 960:
        png_gray(f"/home/z/my-project/work/cap8/frames/f0_w{w}_8bit.png", w, rows, f0[:w*rows])
        print(f"saved w={w} h={rows} 8-bit")

# 16-bit high-byte image at 640 wide (skip 2 bytes per px)
hb = f0[1::2][: (len(f0)//2)//640*640]
rows = len(hb) // 640
png_gray("/home/z/my-project/work/cap8/frames/f0_w640_16bitMSB.png", 640, rows, hb)
print(f"saved w=640 h={rows} 16bit-MSB")
lb = f0[0::2][: (len(f0)//2)//640*640]
png_gray("/home/z/my-project/work/cap8/frames/f0_w640_16bitLSB.png", 640, rows, lb)
print(f"saved w=640 h={rows} 16bit-LSB")
# 16-bit as u16 at 640 wide
u16s = struct.unpack(f"<{len(f0)//2}H", f0[:(len(f0)//2)*2])
rows16 = len(u16s) // 640
png_gray16be("/home/z/my-project/work/cap8/frames/f0_w640_u16.png", 640, rows16, u16s[:640*rows16])
print(f"saved w=640 h={rows16} u16-MSB")

# Also render a late-record frame (rec 960-964) to compare content
f960 = rec_data[960] + rec_data[961] + rec_data[962]
hb2 = f960[1::2][:(len(f960)//2)//640*640]
rows2 = len(hb2)//640
png_gray("/home/z/my-project/work/cap8/frames/f960_w640_16bitMSB.png", 640, rows2, hb2)
print(f"saved late frame w=640 h={rows2}")
