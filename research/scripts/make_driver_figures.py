#!/usr/bin/env python3
"""make_driver_figures.py - screenshot the two driver-design diagram HTML
files to PNG at 2x device scale (300dpi equivalent) for the ReportLab report."""
import os as _os
_HERE = _os.path.dirname(_os.path.abspath(__file__))  # research/scripts
# research/analysis; override with IDONG_ANALYSIS when the extracted
# installer payloads live somewhere else
ANALYSIS = (_os.environ.get('IDONG_ANALYSIS')
            or _os.path.join(_os.path.dirname(_HERE), 'analysis'))
REPO = _os.path.dirname(_os.path.dirname(_HERE))  # the repository root
from playwright.sync_api import sync_playwright

BASE = _HERE
OUT = ANALYSIS + '/figures'
import os
os.makedirs(OUT, exist_ok=True)

JOBS = [
    (f'{BASE}/diagram_driver_ws.html', f'{OUT}/diagram_driver_ws.png'),
    (f'{BASE}/diagram_roadmap.html', f'{OUT}/diagram_roadmap.png'),
]

with sync_playwright() as pw:
    browser = pw.chromium.launch()
    page = browser.new_page(viewport={'width': 1300, 'height': 1400},
                            device_scale_factor=2)
    for src, dst in JOBS:
        page.goto('file://' + src)
        page.wait_for_timeout(300)
        el = page.query_selector('.canvas')
        el.screenshot(path=dst)
        print(f'shot {dst}')
    browser.close()
