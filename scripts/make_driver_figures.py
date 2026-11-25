#!/usr/bin/env python3
"""make_driver_figures.py - screenshot the two driver-design diagram HTML
files to PNG at 2x device scale (300dpi equivalent) for the ReportLab report."""
from playwright.sync_api import sync_playwright

BASE = '/home/z/my-project/scripts'
OUT = '/home/z/my-project/analysis/figures'
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
