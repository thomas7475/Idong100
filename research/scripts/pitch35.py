#!/usr/bin/env python3
"""Nail the row pitch: per-row best-alignment drift + search for per-line markers."""
import struct

raw = open("/home/z/my-project/work/cap8/frames/stream_raw.bin", "rb").read()
f0 = raw[:190908]

# 1. take the even-byte plane (the image) and find per-row alignment drift
ev = f0[0::2]  # 95454 bytes
# guess row width ~640 (image plane). measure best shift for each row pair
def best_shift(a, b, maxs=40):
    best = None
    for s in range(-maxs, maxs + 1):
        if s >= 0:
            aa = a[s:]; bb = b[:len(b)-s]
        else:
            aa = a[:len(a)+s]; bb = b[-s:]
        n = min(len(aa), len(bb), 600)
        aa = aa[:n]; bb = bb[:n]
        ma = sum(aa)/n; mb = sum(bb)/n
        num = sum((x-ma)*(y-mb) for x, y in zip(aa, bb))
        da = sum((x-ma)**2 for x in aa); db = sum((y-mb)**2 for y in bb)
        r = num/((da*db)**0.5) if da > 0 and db > 0 else 0
        if best is None or r > best[0]:
            best = (r, s)
    return best

W = 640
rows = [ev[i*W:(i+1)*W] for i in range(len(ev)//W)]
print("per-row best shift (even plane, W=640):")
for i in range(min(24, len(rows)-1)):
    r, s = best_shift(rows[i], rows[i+1])
    print(f"  row{i:3d}->row{i+1:3d}: shift={s:+3d}  r={r:.3f}")

# 2. same for odd plane
od = f0[1::2]
rows_o = [od[i*W:(i+1)*W] for i in range(len(od)//W)]
print("\nper-row best shift (odd plane, W=640):")
for i in range(min(12, len(rows_o)-1)):
    r, s = best_shift(rows_o[i], rows_o[i+1])
    print(f"  row{i:3d}->row{i+1:3d}: shift={s:+3d}  r={r:.3f}")

# 3. drift accumulation: cumulative shift over 40 rows
cum = 0
shifts = []
for i in range(min(40, len(rows)-1)):
    r, s = best_shift(rows[i], rows[i+1])
    cum += s
    shifts.append(s)
print(f"\ncumulative shift over {len(shifts)} rows: {cum} (avg {cum/max(len(shifts),1):+.2f}/row)")

# 4. is the odd plane a second image? correlate odd row vs even row
def corr(a, b):
    n = min(len(a), len(b))
    a = a[:n]; b = b[:n]
    ma = sum(a)/n; mb = sum(b)/n
    num = sum((x-ma)*(y-mb) for x, y in zip(a, b))
    da = sum((x-ma)**2 for x in a); db = sum((y-mb)**2 for y in b)
    return num/((da*db)**0.5) if da > 0 and db > 0 else 0
print("\neven-row vs odd-row correlation (same index):")
for i in range(6):
    print(f"  row{i}: {corr(rows[i], rows_o[i]):+.3f}")

# 5. odd plane vs even plane shifted by 1 pixel (u16: [lo][hi] vs [hi][lo] order test)
#    correlate odd row i with even row i shifted left/right by W/2?
print("\nodd row0 vs even rows (cross-row search):")
bests = []
for j in range(0, 60):
    c = corr(rows_o[0][:500], rows[j][:500])
    bests.append((abs(c), j, c))
bests.sort(reverse=True)
for b in bests[:5]:
    print(f"  even row {b[1]}: {b[2]:+.3f}")
