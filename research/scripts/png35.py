#!/usr/bin/env python3
"""Correct PNG rendering of the ep85 video stream."""
import struct
import zlib

raw = open("/home/z/my-project/work/cap8/frames/stream_raw.bin", "rb").read()

def png_gray(path, w, h, pix):
    out = bytearray()
    for y in range(h):
        out.append(0)  # filter type 0
        out += bytes(pix[y*w:(y+1)*w])
    def chunk(typ, payload):
        c = typ + payload
        return struct.pack(">I", len(payload)) + c + struct.pack(">I", zlib.crc32(c) & 0xffffffff)
    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 0, 0, 0, 0)
    with open(path, "wb") as f:
        f.write(sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes(out), 6)) + chunk(b"IEND", b""))

# frame0 = first 190,908 B (3 records). Row = 1280 B.
f0 = raw[:190908]
rows = 190908 // 1280
png_gray("/home/z/my-project/work/cap8/frames/f0_1280x149_full.png", 1280, rows, f0[:1280*rows])
print("full 1280-wide:", rows, "rows")

# split planes: pixel = 2 bytes [A][B]
planeA = f0[0::2][:640*rows]   # even bytes
planeB = f0[1::2][:640*rows]   # odd bytes
png_gray("/home/z/my-project/work/cap8/frames/f0_planeA_640.png", 640, rows, planeA)
png_gray("/home/z/my-project/work/cap8/frames/f0_planeB_640.png", 640, rows, planeB)
print("planes A/B: 640 x", rows)

# u16 MSB image (640 wide, 16-bit pixel -> high byte)
msb = f0[1::2][:640*rows]
png_gray("/home/z/my-project/work/cap8/frames/f0_msb_640.png", 640, rows, msb)

# later frame
f960 = raw[960*63636:960*63636+190908]
rows2 = len(f960)//1280
png_gray("/home/z/my-project/work/cap8/frames/f960_1280.png", 1280, rows2, f960[:1280*rows2])
png_gray("/home/z/my-project/work/cap8/frames/f960_planeA_640.png", 640, rows2, f960[0::2][:640*rows2])
png_gray("/home/z/my-project/work/cap8/frames/f960_planeB_640.png", 640, rows2, f960[1::2][:640*rows2])
print("f960: rows", rows2)

# a "5-record" EOF frame from late stream
f1000 = raw[1000*63636:1000*63636+318180]
rows3 = len(f1000)//1280
png_gray("/home/z/my-project/work/cap8/frames/f1000_1280.png", 1280, rows3, f1000[:1280*rows3])
print("f1000 (5 rec, 318180 B): rows", rows3, "(should be 248.6)")
print("done")
