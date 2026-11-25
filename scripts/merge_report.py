#!/usr/bin/env python3
"""merge_report.py - insert the Playwright cover as page 0 of the ReportLab
body PDF, normalize page sizes to A4, and write the final deliverable."""
from pypdf import PdfReader, PdfWriter

A4_W, A4_H = 595.28, 841.89
COVER = '/home/z/my-project/analysis/figures/cover.pdf'
BODY = '/home/z/my-project/analysis/figures/report_body.pdf'
OUT = '/home/z/my-project/download/idong100_camera_research_turn12.pdf'


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
    '/Title': 'Inside the i-Dong Stereo Camera - Hardware Research Report',
    '/Author': 'Z.ai',
    '/Creator': 'Z.ai',
    '/Subject': 'Extraction and analysis of the Toltech idong Sensor V2.6 '
                '(idong100 chipset) camera internals',
})
with open(OUT, 'wb') as f:
    writer.write(f)
print(f'final: {OUT}  pages: {1 + len(body.pages)}')
