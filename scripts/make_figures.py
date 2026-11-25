#!/usr/bin/env python3
"""Screenshot the two diagram HTML files to PNG at 2x device scale (300dpi
equivalent) for embedding into the ReportLab report. Also generate the
extraction-stats bar chart with matplotlib (charts.md rules applied)."""
from playwright.sync_api import sync_playwright

BASE = '/home/z/my-project/scripts'
OUT = '/home/z/my-project/analysis/figures'
import os
os.makedirs(OUT, exist_ok=True)

JOBS = [
    (f'{BASE}/diagram_hardware.html', f'{OUT}/diagram_hardware.png'),
    (f'{BASE}/diagram_hoststack.html', f'{OUT}/diagram_hoststack.png'),
]

with sync_playwright() as pw:
    browser = pw.chromium.launch()
    page = browser.new_page(viewport={'width': 1280, 'height': 900},
                            device_scale_factor=2)
    for src, dst in JOBS:
        page.goto('file://' + src)
        page.wait_for_timeout(300)
        el = page.query_selector('.canvas')
        el.screenshot(path=dst)
        print(f'shot {dst}')
    browser.close()

# ---------------- extraction stats chart (charts.md compliant) ----------------
import matplotlib
matplotlib.use('Agg')
import matplotlib.font_manager as fm
fm.fontManager.addfont('/usr/share/fonts/truetype/chinese/NotoSansSC-Regular.ttf')
fm.fontManager.addfont('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf')
import matplotlib.pyplot as plt
plt.rcParams['font.sans-serif'] = ['DejaVu Sans', 'Noto Sans SC']
plt.rcParams['axes.unicode_minus'] = False

# horizontal bar: files per source (long labels -> horizontal per charts.md 2A)
labels = ['NSIS driver stacks\n(v2.5.0.7 + v2.6.0.6)',
          'Sports King slice 1\n(Inno, camera/middleware set)',
          'Remote zip members\n(setup.exe, payloads, slice)']
values = [2296, 170, 9]

fig, ax = plt.subplots(figsize=(7.4, 2.9), constrained_layout=True)
bars = ax.barh(labels, values, color=['#1a4a7a', '#2d7ab3', '#7a9bb8'],
               height=0.58, edgecolor='none')
for b, v in zip(bars, values):
    ax.text(b.get_width() + max(values) * 0.012, b.get_y() + b.get_height() / 2,
            f'{v:,}', va='center', fontsize=10, color='#142840')
ax.spines['top'].set_visible(False)
ax.spines['right'].set_visible(False)
ax.spines['left'].set_visible(False)
ax.tick_params(axis='y', length=0, labelsize=9.5, colors='#142840')
ax.tick_params(axis='x', labelsize=9, colors='#5a7a96')
ax.grid(axis='x', linestyle='--', alpha=0.2, linewidth=0.5)
ax.set_axisbelow(True)
ax.set_xlim(0, max(values) * 1.14)
ax.set_xlabel('files extracted (SHA-256 recorded)', fontsize=9.5, color='#5a7a96')
fig.savefig(f'{OUT}/extraction_stats.png', dpi=220)
print('chart saved')
