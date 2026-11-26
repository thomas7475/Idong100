#!/usr/bin/env python3
"""Find the true raster width by row-to-row correlation, and analyze
the 2-byte pair structure (two interleaved channels?)."""
import struct

raw = open("/home/z/my-project/work/cap8/frames/stream_raw.bin", "rb").read()
f0 = raw[:190908]

def corr(a, b):
    n = len(a)
    ma = sum(a)/n; mb = sum(b)/n
    num = sum((x-ma)*(y-mb) for x, y in zip(a, b))
    da = sum((x-ma)**2 for x in a); db_ = sum((y-mb)**2 for y in b)
    if da == 0 or db_ == 0: return 0
    return num / ((da*db_) ** 0.5)

print("row-shift correlation at candidate widths (8-bit):")
for w in (320, 480, 530, 640, 800, 1060, 1280, 1590, 2120):
    if len(f0) < 2*w: continue
    rows = [f0[i*w:(i+1)*w] for i in range(2, 12)]
    cs = [corr(rows[i], rows[i+1]) for i in range(len(rows)-1)]
    print(f"  w={w:5d}: avg corr = {sum(cs)/len(cs):+.3f}")

print("\nrow-shift correlation, 16-bit MSB plane (odd bytes) / LSB plane (even bytes):")
ev = f0[0::2]; od = f0[1::2]
for w in (320, 480, 530, 640, 1060):
    if len(ev) < 2*w: continue
    re_ = [ev[i*w:(i+1)*w] for i in range(2, 12)]
    ro = [od[i*w:(i+1)*w] for i in range(2, 12)]
    ce = [corr(re_[i], re_[i+1]) for i in range(len(re_)-1)]
    co = [corr(ro[i], ro[i+1]) for i in range(len(ro)-1)]
    print(f"  w={w:5d}: even-plane corr={sum(ce)/len(ce):+.3f}  odd-plane corr={sum(co)/len(co):+.3f}")

# autocorrelation of full stream at byte lags 1..4096 (coarse: step over samples)
import math
N = 200000
s = raw[:N]
mean = sum(s)/N
var = sum((x-mean)**2 for x in s)
print("\nautocorrelation peaks (lags):")
best = []
for lag in range(1, 4200):
    c = sum((s[i]-mean)*(s[i+lag]-mean) for i in range(0, N-lag, 7))
    best.append((abs(c), lag, c))
best.sort(reverse=True)
seen = []
for a, lag, c in best:
    if all(abs(lag-l) > 8 for l in seen):
        seen.append(lag)
        print(f"  lag={lag:5d}  r={c/var*7:.3f}")
    if len(seen) >= 12: break

# pair stats: even vs odd byte histograms
from collections import Counter
ce = Counter(ev[:100000]); co = Counter(od[:100000])
print("\neven-byte plane: mean", sum(k*v for k,v in ce.items())//100000, "top:", ce.most_common(5))
print("odd-byte plane : mean", sum(k*v for k,v in co.items())//100000, "top:", co.most_common(5))
