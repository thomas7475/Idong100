#!/usr/bin/env python3
"""merge_driver_report.py - insert the Playwright cover as page 0 of the
ReportLab body PDF, normalize page sizes to A4, write the final deliverable."""
import os as _os
_HERE = _os.path.dirname(_os.path.abspath(__file__))  # research/scripts
# research/analysis; override with IDONG_ANALYSIS when the extracted
# installer payloads live somewhere else
ANALYSIS = (_os.environ.get('IDONG_ANALYSIS')
            or _os.path.join(_os.path.dirname(_HERE), 'analysis'))
REPO = _os.path.dirname(_os.path.dirname(_HERE))  # the repository root
from pypdf import PdfReader, PdfWriter

A4_W, A4_H = 595.28, 841.89
COVER = ANALYSIS + '/figures/cover_driver.pdf'
BODY = ANALYSIS + '/figures/driver_body.pdf'
OUT = '/home/z/my-project/download/idong100_RUST_ROS2_DRIVER_DRAFT.pdf'


def normalize(page):
    w, h = float(page.mediabox.width), float(page.mediabox.height)
    if abs(w - A4_W) > 0.1 or abs(h - A4_H) > 0.1:
        page.scale_to(A4_W, A4_H)
    return page


writer = PdfWriter()
writer.add_page(normalize(PdfReader(COVER).pages[0]))
body = PdfReader(BODY)
for p in body.pages:
    writer.add_page(normalize(p))
writer.add_metadata({
    '/Title': 'Idong100 Rust Driver with ROS 2 Output - Implementation Draft',
    '/Author': 'Z.ai',
    '/Creator': 'Z.ai',
    '/Subject': 'Native idiomatic Rust driver and ROS 2 node design for the '
                'Toltech i-Dong Sensor V2.6',
})
with open(OUT, 'wb') as f:
    writer.write(f)
print(f'final: {OUT}  pages: {1 + len(body.pages)}')
