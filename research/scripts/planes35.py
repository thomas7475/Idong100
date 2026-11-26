#!/usr/bin/env python3
"""Continuity across FID frame boundaries + plane relation + stretched plane B."""
import struct
import zlib

raw = open("/home/z/my-project/work/cap8/frames/stream_raw.bin", "rb").read()

def corr(a, b):
    n = min(len(a), len(b))
    if n < 10: return 0
    a = a[:n]; b = b[:n]
    ma = sum(a)/n; mb = sum(b)/n
    num = sum((x-ma)*(y-mb) for x, y in zip(a, b))
    da = sum((x-ma)**2 for x in a); db = sum((y-mb)**2 for y in b)
    return num/((da*db)**0.5) if da > 0 and db > 0 else 0

# 4-byte phase test across the frame0 -> frame1 boundary (offset 190908)
# phase = argmax over p of corr(byte[k], byte[k+4]) restricted... simpler:
# compute corr of 4-byte-phase signature: use bytes at positions ≡ r (mod 4)
def phase_corr(a, b, r):
    return corr(a[r::4], b[r::4])

print("4-byte phase correlation across boundary 190908:")
for r in range(4):
    pre = raw[190908-20000:190908]
    post = raw[190908:190908+20000]
    print(f"  phase {r}: corr(pre, post) = {phase_corr(pre, post, r):+.3f}")

# if continuous: rows at 190720 + 1280k. test corr(rowN, rowN+1) across boundary
b0 = 190908
print("\nrow continuity across boundary:")
for hypo_off in (0, 188, -188):   # candidate: continuous, or restart at 190908
    start = b0 - 1280*10 + hypo_off
    rs = []
    ok = True
    for i in range(12):
        a = raw[start + i*1280: start + (i+1)*1280]
        b = raw[start + (i+1)*1280: start + (i+2)*1280]
        c = corr(a, b)
        rs.append(round(c, 3))
    print(f"  hypo offset {hypo_off:+4d}: row-row corr: {rs}")

# plane relation: A vs B with pixel shifts
f0 = raw[:190908]
A = f0[0::2]; B = f0[1::2]
print("\nplaneA vs planeB shifts (whole-plane, first 40000 B):")
for s in range(-8, 9):
    if s >= 0:
        c = corr(A[:40000], B[s:40000+s])
    else:
        c = corr(A[-s:40000-s], B[:40000])
    print(f"  shift {s:+3d}: {c:+.3f}")

# plane B stats + histogram stretch render
import collections
hist = collections.Counter(B[:95454])
mn = min(k for k, v in hist.items() if v > 50)
mx = max(k for k, v in hist.items() if v > 50)
print(f"\nplaneB byte range (ignoring rare): {mn}..{mx}, peak at 128")
print("planeB histogram (top 12):", hist.most_common(12))

def png_gray(path, w, h, pix):
    out = bytearray()
    for y in range(h):
        out.append(0)
        out += bytes(pix[y*w:(y+1)*w])
    def chunk(typ, payload):
        c = typ + payload
        return struct.pack(">I", len(payload)) + c + struct.pack(">I", zlib.crc32(c) & 0xffffffff)
    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 0, 0, 0, 0)
    with open(path, "wb") as f:
        f.write(sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes(out), 6)) + chunk(b"IEND", b""))

# stretched plane B (contrast-normalized) - 149 rows
rows = 149
Bs = B[:640*rows]
lo, hi = mn, mx
st = bytes(max(0, min(255, (v - lo) * 255 // (hi - lo))) for v in Bs)
png_gray("/home/z/my-project/work/cap8/frames/f0_planeB_stretched.png", 640, rows, st)
print("saved stretched planeB")

# planeA smoothed big view
As = A[:640*rows]
png_gray("/home/z/my-project/work/cap8/frames/f0_planeA_final.png", 640, rows, As)
print("saved planeA final")
