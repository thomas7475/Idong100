#!/usr/bin/env python3
"""gen_report.py - build the body PDF of the i-Dong stereo camera research
report (turn 12). Cover is rendered separately via html2poster.js and merged
with pypdf afterwards.

Numbering plan (Step 3.5):
  cover  -> (separate PDF, no number)
  toc    -> roman i..
  ch 1   Executive Summary
  ch 2   Sources and Extraction Pipeline
  ch 3   Hardware Architecture
  ch 4   Firmware Images
  ch 5   Register Scripts and Operating Modes
  ch 6   Host Software Stack
  ch 7   Data Flow and Runtime Behavior
  ch 8   Driver Stack Version Comparison
  ch 9   Legal Note
  ch 10  Conclusions and Next Steps
  app A  Key File SHA-256 Manifest
  app B  Mode-Switch Register Scripts
  app C  Extraction Script Usage
"""
import os as _os
_HERE = _os.path.dirname(_os.path.abspath(__file__))  # research/scripts
# research/analysis; override with IDONG_ANALYSIS when the extracted
# installer payloads live somewhere else
ANALYSIS = (_os.environ.get('IDONG_ANALYSIS')
            or _os.path.join(_os.path.dirname(_HERE), 'analysis'))
REPO = _os.path.dirname(_os.path.dirname(_HERE))  # the repository root
import csv
import hashlib
import os
import sys

from PIL import Image as PILImage
from reportlab.lib import colors
from reportlab.lib.enums import TA_CENTER, TA_JUSTIFY, TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.units import inch
from reportlab.lib.styles import ParagraphStyle
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.pdfmetrics import registerFontFamily
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (CondPageBreak, HRFlowable, Image, KeepTogether,
                                PageBreak, Paragraph, SimpleDocTemplate,
                                Spacer, Table, TableStyle, XPreformatted)
from reportlab.platypus.tableofcontents import TableOfContents

sys.path.insert(0, '/home/z/my-project/skills/pdf/scripts')
from pdf import install_font_fallback

BASE = '/home/z/my-project'
OUT = f'{ANALYSIS}/figures/report_body.pdf'
FIG = f'{ANALYSIS}/figures'
MANIFEST = f'{ANALYSIS}/reports/all_extracted_sha256.tsv'
INNO_TSV = f'{ANALYSIS}/manifests/extracted_sportsking.tsv'

# ---------------------------------------------------------------- fonts
FONT_DIR = '/usr/share/fonts'
pdfmetrics.registerFont(TTFont('NotoSerifSC', f'{FONT_DIR}/truetype/noto-serif-sc/NotoSerifSC-Regular.ttf'))
pdfmetrics.registerFont(TTFont('NotoSerifSC-Bold', f'{FONT_DIR}/truetype/noto-serif-sc/NotoSerifSC-Bold.ttf'))
pdfmetrics.registerFont(TTFont('Noto Sans SC', f'{FONT_DIR}/truetype/chinese/NotoSansSC-Regular.ttf'))
pdfmetrics.registerFont(TTFont('Noto Sans SC Bold', f'{FONT_DIR}/truetype/chinese/NotoSansSC-Bold.ttf'))
pdfmetrics.registerFont(TTFont('SarasaMonoSC', f'{FONT_DIR}/truetype/chinese/SarasaMonoSC-Regular.ttf'))
pdfmetrics.registerFont(TTFont('FreeSerif', f'{FONT_DIR}/truetype/freefont/FreeSerif.ttf'))
pdfmetrics.registerFont(TTFont('FreeSerif-Bold', f'{FONT_DIR}/truetype/freefont/FreeSerifBold.ttf'))
pdfmetrics.registerFont(TTFont('FreeSerif-Italic', f'{FONT_DIR}/truetype/freefont/FreeSerifItalic.ttf'))
pdfmetrics.registerFont(TTFont('FreeSerif-BoldItalic', f'{FONT_DIR}/truetype/freefont/FreeSerifBoldItalic.ttf'))
pdfmetrics.registerFont(TTFont('DejaVuSans', f'{FONT_DIR}/truetype/dejavu/DejaVuSansMono.ttf'))
registerFontFamily('NotoSerifSC', normal='NotoSerifSC', bold='NotoSerifSC-Bold')
registerFontFamily('Noto Sans SC', normal='Noto Sans SC', bold='Noto Sans SC Bold')
registerFontFamily('FreeSerif', normal='FreeSerif', bold='FreeSerif-Bold',
                   italic='FreeSerif-Italic', boldItalic='FreeSerif-BoldItalic')
registerFontFamily('DejaVuSans', normal='DejaVuSans', bold='DejaVuSans')
install_font_fallback()

# ------------------------------------------- palette (Template 07 body set)
PAGE_BG = colors.HexColor('#f5f8fc')
SECTION_BG = colors.HexColor('#edf2f9')
CARD_BG = colors.HexColor('#e4ecf5')
TABLE_STRIPE = colors.HexColor('#eef3fa')
HEADER_FILL = colors.HexColor('#1a4a7a')
BORDER = colors.HexColor('#c0d0e2')
ACCENT = colors.HexColor('#2d7ab3')
TEXT_PRIMARY = colors.HexColor('#142840')
TEXT_MUTED = colors.HexColor('#5a7a96')

# ---------------------------------------------------------------- layout
MARGIN = 0.9 * inch
PAGE_W, PAGE_H = A4
AVAIL_W = PAGE_W - 2 * MARGIN
AVAIL_H = PAGE_H - 2 * MARGIN
H1_ORPHAN = AVAIL_H * 0.25

# ---------------------------------------------------------------- styles
S = {}
S['body'] = ParagraphStyle('Body', fontName='FreeSerif', fontSize=10.5,
                           leading=16.5, alignment=TA_JUSTIFY,
                           textColor=TEXT_PRIMARY, spaceAfter=8)
S['bullet'] = ParagraphStyle('Bullet', parent=S['body'], alignment=TA_LEFT,
                             leftIndent=14, bulletIndent=4, spaceAfter=4)
S['h1'] = ParagraphStyle('H1', fontName='FreeSerif-Bold', fontSize=19,
                         leading=24, textColor=HEADER_FILL,
                         spaceBefore=6, spaceAfter=2)
S['h2'] = ParagraphStyle('H2', fontName='FreeSerif-Bold', fontSize=14,
                         leading=18, textColor=HEADER_FILL,
                         spaceBefore=14, spaceAfter=6)
S['h3'] = ParagraphStyle('H3', fontName='FreeSerif-Bold', fontSize=11.5,
                         leading=15, textColor=TEXT_PRIMARY,
                         spaceBefore=10, spaceAfter=5)
S['caption'] = ParagraphStyle('Caption', fontName='FreeSerif', fontSize=8.5,
                              leading=11, alignment=TA_CENTER,
                              textColor=TEXT_MUTED, spaceBefore=3, spaceAfter=6)
S['tbl_h'] = ParagraphStyle('TblH', fontName='FreeSerif-Bold', fontSize=9,
                            leading=11.5, textColor=colors.white,
                            alignment=TA_LEFT)
S['tbl'] = ParagraphStyle('Tbl', fontName='FreeSerif', fontSize=9,
                          leading=11.5, textColor=TEXT_PRIMARY,
                          alignment=TA_LEFT)
S['tbl_mono'] = ParagraphStyle('TblMono', fontName='DejaVuSans', fontSize=7.6,
                               leading=9.6, textColor=TEXT_PRIMARY,
                               alignment=TA_LEFT)
S['mono_sm'] = ParagraphStyle('MonoSm', fontName='DejaVuSans', fontSize=6.6,
                              leading=8.4, textColor=TEXT_PRIMARY,
                              alignment=TA_LEFT)
S['code'] = ParagraphStyle('Code', fontName='DejaVuSans', fontSize=8,
                           leading=10.6, textColor=TEXT_PRIMARY)
S['stat'] = ParagraphStyle('Stat', fontName='FreeSerif-Bold', fontSize=17,
                           leading=21, textColor=ACCENT, alignment=TA_CENTER)
S['stat_label'] = ParagraphStyle('StatLabel', fontName='FreeSerif',
                                 fontSize=8.5, leading=11,
                                 textColor=TEXT_MUTED, alignment=TA_CENTER)
S['toc_title'] = ParagraphStyle('TocTitle', fontName='FreeSerif-Bold',
                                fontSize=19, leading=24,
                                textColor=HEADER_FILL, spaceAfter=14)
S['toc0'] = ParagraphStyle('TOC0', fontName='FreeSerif', fontSize=10.5,
                           leading=17, leftIndent=6, textColor=TEXT_PRIMARY)

# ---------------------------------------------------------------- helpers
def roman(n):
    vals = [(10, 'x'), (9, 'ix'), (5, 'v'), (4, 'iv'), (1, 'i')]
    out = ''
    for v, s in vals:
        while n >= v:
            out += s
            n -= v
    return out


def add_heading(text, style, level=0):
    key = 'h_' + hashlib.md5(text.encode()).hexdigest()[:8]
    p = Paragraph(f'<a name="{key}"/><b>{text}</b>', style)
    p.bookmark_name = key
    p.bookmark_level = level
    p.bookmark_text = text
    p.bookmark_key = key
    return p


def h1(story, text):
    story.append(CondPageBreak(H1_ORPHAN))
    story.append(add_heading(text, S['h1'], 0))
    story.append(HRFlowable(width='100%', thickness=1.4, color=ACCENT,
                            spaceBefore=1, spaceAfter=10))


def h2(story, text):
    story.append(CondPageBreak(H1_ORPHAN * 0.5))
    story.append(Paragraph(f'<b>{text}</b>', S['h2']))


def h3(story, text):
    story.append(Paragraph(f'<b>{text}</b>', S['h3']))


def body(story, text):
    story.append(Paragraph(text, S['body']))


def bullet(story, text):
    story.append(Paragraph(f'•  {text}', S['bullet']))


def safe_keep(elements, max_ratio=0.4):
    total = 0
    for el in elements:
        w, h = el.wrap(AVAIL_W, PAGE_H)
        total += h
    if total <= PAGE_H * max_ratio:
        return [KeepTogether(elements)]
    if len(elements) >= 2:
        return [KeepTogether(elements[:2])] + list(elements[2:])
    return list(elements)


def embed_image(path, max_w=None, max_h=None):
    max_w = max_w or AVAIL_W
    max_h = max_h or PAGE_H * 0.35
    im = PILImage.open(path)
    ow, oh = im.size
    ratio = min(max_w / ow if ow > max_w else 1.0,
                max_h / oh if oh > max_h else 1.0)
    return Image(path, width=ow * ratio, height=oh * ratio)


def figure(story, path, caption, max_w=None, max_h=None):
    img = embed_image(path, max_w, max_h)
    story.append(Spacer(1, 10))
    story.extend(safe_keep([img, Paragraph(caption, S['caption'])]))
    story.append(Spacer(1, 8))


def make_table(story, headers, rows, ratios, caption=None, mono_cols=(),
               cell_style=None, header_style=None, keep=True):
    cs = cell_style or S['tbl']
    hs = header_style or S['tbl_h']
    widths = [r * AVAIL_W for r in ratios]
    assert abs(sum(ratios) - 1.0) < 0.01, 'ratios must sum to 1'
    data = [[Paragraph(f'<b>{h}</b>', hs) for h in headers]]
    for row in rows:
        data.append([Paragraph(str(c), S['tbl_mono'] if i in mono_cols else cs)
                     for i, c in enumerate(row)])
    t = Table(data, colWidths=widths, hAlign='CENTER', repeatRows=1)
    style = [
        ('BACKGROUND', (0, 0), (-1, 0), HEADER_FILL),
        ('VALIGN', (0, 0), (-1, -1), 'MIDDLE'),
        ('GRID', (0, 0), (-1, -1), 0.4, BORDER),
        ('LEFTPADDING', (0, 0), (-1, -1), 5),
        ('RIGHTPADDING', (0, 0), (-1, -1), 5),
        ('TOPPADDING', (0, 0), (-1, -1), 4),
        ('BOTTOMPADDING', (0, 0), (-1, -1), 4),
    ]
    for r in range(1, len(data)):
        style.append(('BACKGROUND', (0, r), (-1, r),
                      TABLE_STRIPE if r % 2 == 0 else colors.white))
    t.setStyle(TableStyle(style))
    story.append(Spacer(1, 12))
    if caption and keep:
        story.extend(safe_keep([t, Paragraph(caption, S['caption'])]))
    else:
        # tall table: let it split across pages (repeatRows=1 keeps the header)
        story.append(t)
        if caption:
            story.append(Paragraph(caption, S['caption']))
    story.append(Spacer(1, 12))


def callout_row(story, stats):
    """stats: list of (big, label) tuples, 3 per row."""
    cells = []
    for big, label in stats:
        inner = Table([[Paragraph(f'<b>{big}</b>', S['stat'])],
                       [Paragraph(label, S['stat_label'])]],
                      colWidths=[AVAIL_W / len(stats) - 12])
        inner.setStyle(TableStyle([
            ('BACKGROUND', (0, 0), (-1, -1), CARD_BG),
            ('BOX', (0, 0), (-1, -1), 1, ACCENT),
            ('TOPPADDING', (0, 0), (-1, 0), 8),
            ('BOTTOMPADDING', (0, -1), (-1, -1), 8),
            ('TOPPADDING', (0, -1), (-1, -1), 2),
            ('VALIGN', (0, 0), (-1, -1), 'MIDDLE'),
        ]))
        cells.append(inner)
    outer = Table([cells], colWidths=[AVAIL_W / len(stats)] * len(stats),
                  hAlign='CENTER')
    outer.setStyle(TableStyle([
        ('VALIGN', (0, 0), (-1, -1), 'MIDDLE'),
        ('LEFTPADDING', (0, 0), (-1, -1), 4),
        ('RIGHTPADDING', (0, 0), (-1, -1), 4),
    ]))
    story.append(Spacer(1, 6))
    story.append(KeepTogether(outer))
    story.append(Spacer(1, 10))


def code_block(story, text, title=None):
    parts = []
    if title:
        parts.append(Paragraph(f'<b>{title}</b>', S['h3']))
    pre = XPreformatted(text, S['code'])
    box = Table([[pre]], colWidths=[AVAIL_W])
    box.setStyle(TableStyle([
        ('BACKGROUND', (0, 0), (-1, -1), SECTION_BG),
        ('LINEBEFORE', (0, 0), (0, -1), 2.4, ACCENT),
        ('LEFTPADDING', (0, 0), (-1, -1), 10),
        ('RIGHTPADDING', (0, 0), (-1, -1), 8),
        ('TOPPADDING', (0, 0), (-1, -1), 6),
        ('BOTTOMPADDING', (0, 0), (-1, -1), 6),
    ]))
    parts.append(box)
    story.extend(safe_keep(parts, max_ratio=0.55))
    story.append(Spacer(1, 8))


# ------------------------------------------------------------- doc + page
class ReportDoc(SimpleDocTemplate):
    def afterFlowable(self, flowable):
        if getattr(flowable, 'is_body_start', False):
            self._body_start = self.page
        if hasattr(flowable, 'bookmark_name'):
            # TOC entries use body-relative page numbers to match the footer
            bs = getattr(self, '_body_start', None)
            page_label = self.page - bs + 1 if (bs and self.page >= bs) else self.page
            self.notify('TOCEntry', (getattr(flowable, 'bookmark_level', 0),
                                     getattr(flowable, 'bookmark_text', ''),
                                     page_label,
                                     getattr(flowable, 'bookmark_key', '')))


def on_page(canvas, doc):
    canvas.saveState()
    canvas.setFont('FreeSerif', 7.5)
    canvas.setFillColor(TEXT_MUTED)
    canvas.drawString(MARGIN, PAGE_H - 38,
                      'Inside the i-Dong Stereo Camera - Hardware Research Report')
    canvas.setStrokeColor(ACCENT)
    canvas.setLineWidth(1.2)
    canvas.line(MARGIN, PAGE_H - 44, PAGE_W - MARGIN, PAGE_H - 44)
    canvas.setStrokeColor(BORDER)
    canvas.setLineWidth(0.5)
    canvas.line(MARGIN, 40, PAGE_W - MARGIN, 40)
    canvas.setFont('FreeSerif', 7.5)
    canvas.setFillColor(TEXT_MUTED)
    canvas.drawString(MARGIN, 28, 'idong100 Reverse-Engineering Project - Turn 12')
    bs = getattr(doc, '_body_start', None)
    if bs is not None and doc.page >= bs:
        label = str(doc.page - bs + 1)
    else:
        label = roman(doc.page)
    canvas.drawRightString(PAGE_W - MARGIN, 28, label)
    canvas.restoreState()


# ------------------------------------------------------------ manifest data
def load_manifest():
    m = {}
    with open(MANIFEST, encoding='utf-8') as f:
        for row in csv.DictReader(f, delimiter='\t'):
            m[row['path']] = (row['sha256'], int(row['size']))
    return m


def sha_of(manifest, suffix):
    for p, (sha, size) in manifest.items():
        if p.endswith(suffix):
            return sha, size, p
    return '-', 0, suffix


MAN = load_manifest()
V6 = 'nsis/xeyez_pc_v2.6.0.6/xeyez_Lobby/$_6_'
V5 = 'nsis/xeyez_pc_v2.5.0.7/xeyez_Lobby/$_6_'
SK = 'extracted_sportsking'


def sha_short(suffix):
    sha, size, _ = sha_of(MAN, suffix)
    return sha[:16] + '...'


# ================================================================ content
story = []

# ---- TOC (front matter, roman numbering) ----
toc = TableOfContents()
toc.levelStyles = [S['toc0']]
story.append(Paragraph('<b>Table of Contents</b>', S['toc_title']))
story.append(toc)
story.append(PageBreak())

# ============================ 1. Executive summary ============================
first = add_heading('1. Executive Summary', S['h1'], 0)
first.is_body_start = True
story.append(first)
story.append(HRFlowable(width='100%', thickness=1.4, color=ACCENT,
                        spaceBefore=1, spaceAfter=10))
body(story,
     'This report documents turn 12 of the idong100 reverse-engineering project, whose '
     'goal is to understand exactly how the Toltech <b>i-Dong Sensor V2.6</b> stereo camera '
     '(the "Sports King" motion console sensor, USB VID 0471, PID 2600 in the field and '
     '2500 at runtime) works, from its silicon to its Windows host software. Because the '
     'sandbox is recycled between sessions, every artifact from earlier turns had to be '
     're-acquired. The vendor download server <b>download.i-dong.com is still alive</b>, so '
     'the complete vendor payload was fetched again over plain HTTP using ranged requests, '
     'and every extracted file was recorded with a SHA-256 hash, as requested.')
body(story,
     'The extraction pipeline itself was the first deliverable of this turn: the Inno Setup '
     'extractor <font name="DejaVuSans" size="8.5">inno_bin_extract.py</font> now prints a '
     'SHA-256 (and the Inno-stored MD5 verification verdict) for every file it extracts and '
     'appends each row to a TSV manifest, and a new tool '
     '<font name="DejaVuSans" size="8.5">fetch_zip_members.py</font> fetches individual '
     'members out of the 1.5 GB vendor ZIP archives without downloading them whole. With '
     'these tools, 2,475 files were re-acquired and hashed: both Windows driver stacks '
     '(v2.5.0.7 and v2.6.0.6, NSIS payloads inside the vendor ZIPs), the 170-file camera and '
     'middleware set from the Sports King installer slice 1, and the raw payload members '
     'themselves.')
body(story,
     'The analysis produced several results that materially advance the project. The FPGA '
     'mode-switch mechanism is now proven from vendor scripts rather than inferred from '
     'disassembly: register 0x02 selects <b>0x0A = blob mode</b> versus <b>0x0B = image '
     'mode</b>, and the image-mode FPGA script is literally named '
     '<i>"2sensor diff"</i>, confirming that the raw-image path streams the difference of '
     'the two sensors. The complete host middleware API surface was mapped from export '
     'tables, including the iDongMarkManager ten-step marker-tracking pipeline and the '
     'iDongAction 3.0 callback interface. All of it is tabulated in this report together '
     'with the SHA-256 manifest of the evidence.')
callout_row(story, [
    ('2,475', 'files extracted and SHA-256 hashed'),
    ('178 / 178', 'Inno extractions MD5-verified, 0 failed'),
    ('61 to 79', 'xeyezHal.dll exports, v2.5.0.7 vs v2.6.0.6'),
])

# ==================== 2. Sources and extraction pipeline ======================
h1(story, '2. Sources and Extraction Pipeline')
body(story,
     'All vendor software reaches the public internet through eight archives on '
     '<font name="DejaVuSans" size="8.5">download.i-dong.com</font>, discovered in earlier '
     'turns and re-verified alive on 2026-11-23. The two archives used this turn are the '
     '"Sports King" PC installer ZIP '
     '(<font name="DejaVuSans" size="8">idong_1.9.04_windows_yundongwang_A_setup.zip</font>, '
     '1,506,761,140 bytes, 99 members) and the "Fashion" PC installer ZIP '
     '(<font name="DejaVuSans" size="8">idong_1.9.02_windows_shishang_A_setup.zip</font>, '
     '1,705,269,409 bytes). Both carry the camera driver payload in a '
     '<font name="DejaVuSans" size="8">dependent/xeyez_pc_*</font> folder - version '
     'v2.6.0.6 in the Sports King archive and v2.5.0.7 in the Fashion archive - which '
     'allowed both driver generations to be re-acquired and compared.')
body(story,
     'The fetcher reads each ZIP central directory with two ranged GETs (a 64 KB tail plus '
     'the directory itself), then downloads only the wanted members in 16 MB chunks with '
     'retry and resume, inflating them on the fly. Downloads totalled roughly 267 MB of '
     'compressed data, dominated by the 196 MB first Inno slice '
     '<font name="DejaVuSans" size="8">idong_setup-1.bin</font>. Every fetched member is '
     'printed with its SHA-256 and MD5 and appended to a manifest. The extraction then '
     'proceeded in three stages, summarised in Table 1 and Figure 1.')
make_table(story,
    ['Stage', 'Tool', 'Input', 'Output'],
    [['1. ZIP member fetch', 'fetch_zip_members.py', 'vendor ZIPs (ranged HTTP)',
      '9 members: setup.exe, both driver payloads, slice 1'],
     ['2. NSIS extraction', '7zz 24.08', 'xeyez_setup.exe + 4 x .dat payloads',
      '2,296 files (both driver stacks)'],
     ['3. Inno extraction', 'inno_bin_extract.py --extract', 'idong_setup.exe + slice 1',
      '178 records, 170 unique files (all MD5-verified)']],
    [0.16, 0.24, 0.30, 0.30],
    caption='Table 1: The three extraction stages and their outputs.')
body(story,
     'The Inno extractor update was the explicit request for this turn. During extraction '
     'each file is now reported as <i>SHA256 <hash> MD5 <hash> MD5_OK <size> '
     '<path></i>, where MD5_OK means the extracted bytes match the checksum that the '
     'Inno data-entry table stores for that file - an end-to-end integrity proof from the '
     'vendor build machine to the analysis sandbox. A new '
     '<font name="DejaVuSans" size="8.5">--include REGEX</font> option extracts only '
     'matching destinations (used to pull just the camera-relevant subset out of a 21,995 '
     'file installer), and <font name="DejaVuSans" size="8.5">--manifest FILE</font> writes '
     'the TSV manifest. All 178 extraction records passed the MD5 check; eight destination '
     'paths appear twice in the custom-build file table with identical content, which is '
     'why 178 writes produced 170 unique files.')
figure(story, f'{FIG}/extraction_stats.png',
       'Figure 1: Files extracted per source; every file carries a recorded SHA-256.',
       max_w=390)

# ======================= 3. Hardware architecture =============================
h1(story, '3. Hardware Architecture')
body(story,
     'The sensor is a three-chip pipeline, established in earlier turns from teardown '
     'photographs, the vendor INF file, firmware strings and the EEPROM dump, and now '
     'confirmed again by the re-acquired artifacts (Figure 2). Two Brigates BG0316 CMOS '
     'sensors capture VGA frames; a Xilinx Spartan-3 FPGA performs the pixel-side '
     'processing; an Analog Devices Blackfin BF527 runs the main firmware and the USB '
     'device stack; and a small 8051-class microcontroller manages sensor registers. Six '
     '850 nm IR LEDs illuminate passive reflective markers worn by the player, and a '
     'T24C16A I2C EEPROM stores the factory stereo calibration.')
figure(story, f'{FIG}/diagram_hardware.png',
       'Figure 2: Device hardware pipeline, with the evidence file for each block.',
       max_w=465)
body(story,
     'The USB lifecycle is the key external behaviour. A freshly powered unit enumerates '
     'as <b>0471:2600 "TOLTech xeyez Bootloader"</b> - a BF527 sitting in its ROM loader '
     'with no strings and full-speed-style 64-byte bulk packets. The Windows stack then '
     'uploads three firmware images over the bulk OUT endpoint (see Chapter 4) and the '
     'device re-enumerates as the runtime composite device <b>0471:2500</b>, whose '
     'interfaces 0-3 are standard UVC/UAC and interface 4 is the vendor bulk interface '
     'driven by <font name="DejaVuSans" size="8.5">xeyez.sys</font> (a retargeted Microsoft '
     'BulkUSB sample, service name XEYEZ, MaximumTransferSize 4096). The boot upload '
     'protocol - 64-byte packets with a per-packet count and CRC announced up front - was '
     'decoded in earlier turns from xEyeUSB.dll and is unchanged in the re-acquired files.')
make_table(story,
    ['Component', 'Part', 'Role (evidence)'],
    [['Image sensors', '2x Brigates BG0316',
      'VGA 640x480 global shutter; register scripts named "0316_Cmos"; cx of the stored calibration is 312-313 px, i.e. 640-wide frames'],
     ['Pixel processor', 'Xilinx Spartan-3 XC3S400-FT256',
      'host-uploaded bitstream A_a_v4.4_BG0313.bit (work_top.ncd, 2010-04-06); mode register 0x02; blob window regs 0x50-0x57'],
     ['Main DSP + USB', 'ADI Blackfin BF527 (400 MHz class)',
      'ens_code.ldr with ep0_handler / usb_init symbols; L1 data-A/B and instruction targets; USB 2.0 OTG peripheral'],
     ['Sensor control', 'M8051-class MCU',
      'app_m8051_2500.hex / app_m8051_2611.hex; register pages data0.dat - data10.dat select via reg 0xF0'],
     ['Calibration store', 'T24C16A EEPROM, 2 KB',
      'serial AB460DC8BA12C6840B49CD5786C329DD; OpenCV-convention stereo pair; baseline exactly 100.00 mm, mean focal 432.33 px'],
     ['Illumination', '6x 850 nm IR LEDs',
      'LED scale registers 0x58-0x6B in Led_Scale_Fpga_v26.txt; reflective marker tracking'],
     ],
    [0.16, 0.24, 0.60],
    caption='Table 2: Hardware components and the artifacts that evidence them.')

# ============================ 4. Firmware images ==============================
h1(story, '4. Firmware Images')
body(story,
     'All three firmware images were re-acquired and hashed, and two of them were parsed '
     'deeper than in previous turns. The FPGA bitstream header parses cleanly and pins the '
     'design identity: design <i>work_top.ncd</i> with '
     '<i>HW_TIMEOUT=FALSE;UserID=0xFFFFFFFF</i>, target part <b>3s400aft256</b>, built '
     '<b>2010-04-06 10:33:41</b>, with a raw payload of 235,820 bytes. Because the '
     'bitstream is uploaded by the host at every boot (BitFile entry in xEyeUsb.ini, '
     'xeyezHal_SetFpgaFile / DownLoadFpga), it is volatile and replaceable with zero '
     'brick risk - the central fact behind the custom-FPGA fallback path of the scanner '
     'roadmap.')
h2(story, '4.1 Blackfin image ens_code.ldr')
body(story,
     'The 54,420-byte Blackfin loader image is the most informative device-side artifact. '
     'Its first block writes 4 bytes to L1 data bank A address 0xFF800040; scanning the '
     'file for plausible block headers yields 66 header-like positions whose targets '
     'cluster overwhelmingly in <b>L1 data bank B (0xFF9xxxxx, 51 hits)</b> against 12 in '
     'bank A and 2 in instruction SRAM, and the image contains 17 four-byte values '
     'pointing into L1 instruction memory between 0xFFA03D1C and 0xFFA0E6BE - a '
     'function-pointer/jump table that bounds the code region to roughly 43 KB. Embedded '
     'strings include the ASCII symbols <i>ep0_handler</i> and <i>usb_init</i> (USB endpoint '
     'handling) and UTF-16LE strings <i>xeyez</i> and <i>TOLtech</i> (device identity), plus '
     'a long descending-value lookup table typical of a gamma or threshold ramp. The exact '
     'block framing of this vendor LDR could not be matched to the standard VisualDSP++ '
     'zerofill/init flag conventions - several flag-bit interpretations were tried and '
     'empirically rejected - so a full block map and disassembly remain open items; the '
     'observed targets and tables above are direct byte-level facts.')
h2(story, '4.2 8051 images and register pages')
body(story,
     'Version 2.6.0.6 ships two Intel-HEX 8051 images, app_m8051_2500.hex (21,110 code '
     'bytes) for the PID 2500 runtime device and app_m8051_2611.hex (21,362 bytes) for the '
     '2611 variant, where v2.5.0.7 shipped a single app_m8051.hex. Both parse to the same '
     'memory map: a 3-byte reset vector at 0x0000, eight 3-byte interrupt vectors spaced '
     'at 0x8000-0x806D, and one contiguous code span at 0x8073-0xD2A6 (0xD3A2 for the '
     '2611 image). The vectors at 0x8000 rather than 0x0003 mark this as a bootloader-'
     'offset 8051 layout, consistent with the image being uploaded rather than mask-ROM '
     'resident. The eleven ASCII files data0.dat through data10.dat are not binary data '
     'but <b>register-page scripts</b> in the same "0xRR,0xVV" text format as the CMOS '
     'scripts: data0.dat selects page 3 via 0xF0,0x03 and then writes page registers '
     '0x00-0x42; the other pages follow the same pattern. Together with the 0xF0 page '
     'selector rule this maps the entire sensor register space that the 8051 masters.')
make_table(story,
    ['Firmware file', 'Size (B)', 'SHA-256 (first 16)', 'Parse result'],
    [['A_a_v4.4_BG0313.bit', f"{sha_of(MAN, 'A_a_v4.4_BG0313.bit')[1]:,}", sha_short('A_a_v4.4_BG0313.bit'),
      'work_top.ncd, 3s400aft256, 2010-04-06 10:33:41, payload 235,820 B'],
     ['ens_code.ldr', f"{sha_of(MAN, V6 + '/ens_code.ldr')[1]:,}", sha_short(V6 + '/ens_code.ldr'),
      'L1 A/B + instruction targets; ep0_handler, usb_init; UTF-16 "xeyez", "TOLtech"'],
     ['app_m8051_2500.hex', f"{sha_of(MAN, 'app_m8051_2500.hex')[1]:,}", sha_short('app_m8051_2500.hex'),
      '21,110 code bytes; vectors 0x8000-0x806D; code 0x8073-0xD2A6'],
     ['app_m8051_2611.hex', f"{sha_of(MAN, 'app_m8051_2611.hex')[1]:,}", sha_short('app_m8051_2611.hex'),
      '21,362 code bytes; same layout, code ends 0xD3A2'],
     ['data0.dat ... data10.dat', '-', '-',
      'ASCII register pages; 0xF0 selects page; e.g. data0 = page 3 regs 0x00-0x42']],
    [0.22, 0.09, 0.20, 0.49],
    caption='Table 3: Firmware inventory of the v2.6.0.6 stack (full hashes in Appendix A).',
    mono_cols=(2,))

# ================== 5. Register scripts and operating modes ===================
h1(story, '5. Register Scripts and Operating Modes')
body(story,
     'The v2.6.0.6 xeyezHal.ini spells out a four-way operating-mode matrix that was only '
     'partially visible before: <b>blob versus image mode, crossed with rolling versus '
     'global shutter</b>, each combining one CMOS script and one FPGA script. The rolling '
     'entries are commented out with "#" and their script files are absent from the '
     'payload, so the shipped configuration is global-shutter only (matching the BG0316); '
     'the rolling variants were presumably for a BG0313-era sensor. All scripts are '
     'applied through command 3 register writes in chunks of at most 8 bytes, with the '
     'two-byte register value pairs fed through verbatim.')
make_table(story,
    ['Config key (xeyezHal.ini)', 'Script pair', 'Meaning'],
    [['Rolling_Blb_0316_*', '#Rolling_0316_Cmos_v26.txt (commented out)', 'rolling blob mode - not shipped'],
     ['Rolling_Img_0316_*', '.\\30fps\\Rolling_0316_Cmos_v26.txt (file absent)', 'rolling image mode - not shipped'],
     ['Global_Blb_0316_Cmos', '.\\30fps\\30fps_Global_0316_Cmos.txt', 'global blob mode, sensor side'],
     ['Global_Blb_0316_Fpga', '.\\30fps\\30fps_Global_blob_1LED.dat', 'global blob mode, FPGA side'],
     ['Global_Img_0316_Cmos', '.\\30fps\\30fps_Global_0316_Cmos.txt', 'global image mode, sensor side (same as blob)'],
     ['Global_Img_0316_Fpga', '.\\30fps\\30fps_Global_image_2sensor_diff_1LED.dat', 'global image mode, FPGA side']],
    [0.26, 0.38, 0.36],
    caption='Table 4: The four operating-mode configurations defined by xeyezHal.ini.')
body(story,
     'Diffing the two FPGA scripts settles the mode-switch question with vendor-authored '
     'evidence. Both scripts share the same common register block (0x20, 0x28, 0x06, '
     '0x08, 0x09, 0x0a, 0x0c, 0x19, 0x1a, 0x1b, 0x1c, 0x1d) and differ in exactly two '
     'places: the first register <b>0x02 is 0x0A in blob mode and 0x0B in image mode</b>, '
     'and the blob script additionally programs registers <b>0x50-0x57</b> and then issues '
     '0x06,0x02 (start). Interpreting 0x51/0x52 and 0x53/0x54 as big-endian 16-bit pairs '
     'gives 0x027C = 636 and 0x01D0 = 464 - a blob search window of 636 x 464 pixels '
     'inside the 640 x 480 frame - with 0x55/0x56 = 0x0005 a small count or area limit. '
     'The image-mode script contains no blob window at all, because in that mode the FPGA '
     'streams pixels instead of detected blobs.')
callout_row(story, [
    ('0x02 = 0x0A', 'FPGA register selecting blob mode'),
    ('0x02 = 0x0B', 'FPGA register selecting image mode'),
    ('636 x 464', 'blob search window, regs 0x50-0x57'),
])
body(story,
     'The image-mode FPGA script is worth reading literally: its file name is '
     '<i>30fps_Global_image_2sensor_diff_1LED.dat</i>. The vendor themselves labelled the '
     'raw-image output as the <b>difference of the two sensors</b>, illuminated by a '
     'single LED group. This confirms the suspicion from the turn-11 disassembly (that '
     'GET_IMG_START / GetImageData might return diff-only frames) and means the planned '
     'dense-stereo scanner phase must either work with differential images or replace the '
     'DSP program to obtain full frames. The CMOS script 30fps_Global_0316_Cmos.txt '
     'programs 38 registers (0x06,0x02 start, 0x07,0x81, exposure and window registers '
     '0x29/0x2b/0x39/0x3b = 0x4F, gain 0x19 = 0xFF, and ends 0x1d,0x01); the LED script '
     'Led_Scale_Fpga_v26.txt zeros the twenty LED-scale registers 0x58-0x6B. Full script '
     'listings are in Appendix B.')

# ========================= 6. Host software stack =============================
h1(story, '6. Host Software Stack')
body(story,
     'The re-extracted driver stacks let the full host-side architecture be restated with '
     'hashes attached (Figure 3). At the bottom, xeyez.sys shuttles bulk data for '
     'EasyUSB.dll; xEyeUSB.dll drives the boot state machine; xeyezHal.dll wraps '
     'everything for the application layer, including register-script replay and (in '
     'v2.6) DRM. Above the transport, xEyeClass.dll runs the reader thread that turns '
     'endpoint 0x82 bytes into xEye3Ddata records and republishes them on named pipes; '
     'the engines and the game middleware consume from there.')
figure(story, f'{FIG}/diagram_hoststack.png',
       'Figure 3: Host software stack from kernel driver to game middleware.',
       max_w=465)
h2(story, '6.1 xeyezHal.dll: 61 to 79 exports')
body(story,
     'The two shipped generations differ sharply. Version 2.5.0.7 carries a single '
     '837,960-byte xeyezHal.dll with 61 exports; version 2.6.0.6 ships a 153,968-byte '
     'DLL with 79 exports - the code was restructured and shrank by a factor of five '
     'while gaining eighteen new entry points. The additions are all security or '
     'hardware-identity related: DRMRead/DRMWrite, SecurityComm, CheckCodeRead/Write, '
     'CheckMatch, Check_Key, CalibNumRead/Write, DeviceCamRead/Write, DeviceClassRead, '
     'HardVersion_Read/Write, BlobAxisRead/Write, SetClassifier and CmosReset. No export '
     'was removed. The practical consequence for the driver project stands: v2.5.0.7 is '
     'the cleaner target for protocol replay, while v2.6.0.6 is what the retail '
     'generation-2 devices expect, and its anti-piracy additions must be fed the genuine '
     'vendor files unmodified.')
h2(story, '6.2 Stereo processing and classification on the host')
body(story,
     'StereoCamera.dll v2.6 grew from 100,680 to 104,816 bytes and now exposes a C++ '
     'bridge class: Get3dSingle (on-host triangulation with the EEPROM calibration), '
     'Get3DObject, BlobFeature2ClassResult (SC_BLOB_FEATURE to SC_RESULT), '
     'RegisterScDataListener, SetBlobFeatureMaxWidth, SetClassifier and SetLens '
     '(SC_LENS_PARAM). It works with classifying.dll, whose three exports - '
     'ClassifyAndTrack, SetLensParam, SetStaticPoint_Param - run blob classification and '
     'tracking, and with nine SVM data files. Those files turn out to be plain text: '
     'SVM_Class_ball.dat is a 4-feature support-vector model (feature values around 1-12 '
     'match blob width, height and derived measures), while the newer V4 files '
     '(2012-04-11) are compact 2-D linear separators with dates in comments - the '
     'classifier was retrained repeatedly between 2011-07 and 2012-04.')
h2(story, '6.3 Middleware and game-facing API')
body(story,
     'The Sports King slice contributed the game-side half of the stack. The four small '
     'engine DLLs show the property-based API: xEyeEngine.dll (InitxEyeEngine, '
     'GetxEyePipeData, BackxEyeData, SetxEyeProperty, ClosexEyeData), xEye2DEngine.dll '
     '(Initxeye2DEngine, GetxEyePipe2DData), xEyeUserParamRebuild.dll (a single export, '
     'xEye3DT2D - 3D-to-2D reprojection), and the 38-export xEyeMsgEngine.dll singleton '
     '(StartHeartBeat/GetHeartBeat/CancelHeartBeat, MouseModeSwitch, UseKeySim, '
     'UseMouseSim, Vibrate, SetxEyeEngineCallBack - the handle-vibration and input-'
     'simulation entry points). iDongAction 3.0 is the action layer: iDongAction.dll '
     '(64 exports) instantiates iDongActionManager and defines the iDongActionCallBack '
     'interface with one virtual method per gesture - action3DMouse, actionCircle, '
     'actionFighting, actionGesturing (hand mark types), actionHammer, actionHead, '
     'actionHitting, actionLining, actionResetPos, actionRunning, plus keyDown/keyUp.')
make_table(story,
    ['Step', 'Function (from export table)', 'Tunable parameters'],
    [['1', 'SetFirstStepMergeData', 'SetMergeThreadHold'],
     ['2', 'Set2ndStepClean', 'missFrameThreadhold'],
     ['3', 'Set3rdStepForeCastFrame', 'SetForeCastRuleType'],
     ['4', 'Set4thStepTracing (MarkTracingBall / MarkTracingLine)', 'SetMaxTracingBallDis / SetMaxTracingLineDis'],
     ['5', 'Set5thStepMarkBasicAdjust', 'SetMaxAdjustLineDis, SetMaxExchangeMarkDis'],
     ['6', 'Set6thStepMarkNowDataToTrace', '-'],
     ['7', 'Set7thStepForeCastAdjust', 'SetForeCastAdjustType'],
     ['8', 'Set8thStepCalculateVelocity', '-'],
     ['9', 'Set9thStepMarkSourceSelect', 'SetSrcSelectType, SetSrcCorrectType, SetTimeForExchange_Left_Right'],
     ['10', 'Set10thFourBallsHandLegYDisThreshold', 'hand/leg Y-distance thresholds']],
    [0.08, 0.52, 0.40],
    caption='Table 5: The iDongMarkManager ten-step marker tracking pipeline, read from its 75 export names.')
body(story,
     'iDongMarkManager.dll is the piece that turns raw target coordinates into game '
     'semantics, and its export table reads as a design document (Table 5): merge '
     'duplicated marks, clean, predict the next frame, trace balls and lines, adjust, '
     'compute velocity, select between left/right marker identities (including a '
     'left-right exchange time constant), and finally apply hand/leg Y-distance '
     'thresholds for the four-marker body configuration. iDongSimulator3.exe imports '
     'iDongAction.dll and xEyeMsgEngine.dll and replays recorded sessions - its shipped '
     'logs show the sequence "Load analysis model[iDong3DMouse..Signal] ball[0]/ball[10]", '
     'WM_IDONG_GETSTANDARDPOS queries and SetProperty(setStandardPos) calls, direct '
     'evidence of how the games prime the tracking pipeline.')
h2(story, '6.4 Game-side consumers and negative findings')
body(story,
     'Around the Lobby, the camera-facing UI modules are XeYeZSetting.dll and '
     'AboutXeYeZDlg.dll (single-interface factories), DistanceSetting.dll '
     '(GetDistanceInterface), HandSignal.dll (BeginGetHandSignals, '
     'SetHandPosCallBackFunc - gesture recognition on marker positions), '
     'VideoWindowHandler.dll (CVideoWindowHandler::TakePhoto and '
     'ConvertPixelCoordinate on _TOL_VideoWindow structures), and IdentifyModule.dll '
     '(a COM in-proc server). Three families that look camera-related are not: '
     'CapStill.dll/exe (OpenCapWnd/SaveCap - screen-window capture for game photos), '
     'Camset.dll (ShowCameraSetDlg/ShowAngleAdjustDlg - settings UI for the separate '
     'UVC webcam in the kit), and the RHCC family (RHCCA = audio codec wrapper, RHCCP = '
     'Borland-built chat-room protocol with video-chat commands, RHCCEXTV = 2.2 MB '
     'codec blob). The Flash local-storage file conf/camera_set/settings.sol confirms '
     'the split: its defaultcamera value is "USB video device" - the UVC webcam, not '
     'the tracking sensor.')

# ==================== 7. Data flow and runtime behaviour ======================
h1(story, '7. Data Flow and Runtime Behaviour')
body(story,
     'Putting the pieces together, the steady-state data path is: the FPGA extracts up to '
     'ten blob centroids per frame at 30 fps sensor rate; the BF527 runs stereo matching '
     'against the EEPROM calibration and streams xEye3Ddata records - a 72-byte header '
     'plus ten 88-byte target slots - to the host at up to 50 fps (xEye.ini: xEyeFrame '
     '= 50, USBComFPS = 35, PipeBufferSize = 9000); xEyeClass.dll validates and republishes '
     'on named pipes; the engines apply properties; iDongMarkManager runs the ten-step '
     'tracking pipeline; iDongAction 3.0 fires game callbacks; and '
     'xEyeVirtualInput.dll (with the vMouse.sys / vKeyboard.sys drivers) injects the '
     'resulting key and mouse events for games that only understand HID input. Host-side '
     'triangulation via StereoCamera.dll and blob classification via classifying.dll '
     'with the SVM models are used in calibration and setup flows rather than in the hot '
     'path.')
body(story,
     'The runtime tunables in the re-extracted xEyeUsb.ini quantify how much the vendor '
     'trusts the optics: disturbance is graded from none to high by counting frames '
     'whose data quality degrades (NoDisturb = 0, LtDisturb = 3, MdDisturb = 7, '
     'MhDisturb = 10, HiDisturb = 11), the link is declared broken after 100 bad frames '
     '(LinkErrCnt) or five missing heartbeat windows (BgCount = 5), and a test-mode '
     'NoiseLevel = 50 can inject synthetic noise of one pixel per 100 units into the '
     'coordinate stream. GreenThreshold = 20 and Yellow/RedThreshold = 35 drive the '
     'traffic-light quality indicator shown by the settings UI, and RingSize = 200 '
     'selects a ring-buffer smoothing filter - larger is more stable but laggier. '
     'EnvAnalyzer.dll (GetEnvDisturbGrade, SetDataListLen) is the module that computes '
     'the disturbance grade from the record stream.')
make_table(story,
    ['Parameter (xEyeUsb.ini)', 'Value', 'Meaning'],
    [['NeedHandleMatch', '1', 'pair 2.4 GHz RF handles at startup'],
     ['MonitorList / BgCount / LinkErrCnt', '10 / 5 / 100', 'watchdog queue, heartbeat windows, link-error limit'],
     ['NoDisturb .. HiDisturb', '0 / 3 / 7 / 10 / 11', 'environment-disturbance grade thresholds'],
     ['GreenThreshold / Yellow / Red', '20 / 35 / 35', 'camera quality traffic-light thresholds'],
     ['NoiseLevel', '50', 'test noise injection (100 units = 1 pixel)'],
     ['RingSize', '200', 'ring-filter size (stability vs latency)'],
     ['DevicePlatform / SimPlatform', '2 / 2', 'hardware generation 2; native gen-2 data format'],
     ['USBComFPS', '35', 'USB reader thread frame limit']],
    [0.34, 0.14, 0.52],
    caption='Table 6: Runtime parameters from xEyeUsb.ini (v2.6.0.6).')

# ================== 8. Driver stack version comparison ========================
h1(story, '8. Driver Stack Version Comparison')
body(story,
     'Beyond xeyezHal.dll, the diff between the two NSIS payloads is narrow but telling. '
     'Of the 145 files in v2.6.0.6 versus 142 in v2.5.0.7, the only additions are the '
     'split 8051 images and the V4 SVM classifiers; nothing was removed. Thirty-nine '
     'files share a name but differ in content, and in almost every case the v2.6.0.6 '
     'file is exactly 40 bytes larger - a rebuild with an additional section or '
     'signature, consistent with the 2012-06-11 link timestamps across the v2.6.0.6 '
     'binaries. The functional deltas concentrate in xEyeUSB.dll (+4,136 bytes, the boot '
     'state machine gained the DRM handshake), StereoCamera.dll (+4,136 bytes, the '
     'bridge class and classifier hooks) and xeyezHal.dll itself (-683,992 bytes, '
     'restructured). Firmware is byte-identical for the FPGA bitstream and the Blackfin '
     'image; only the 8051 side evolved from one image to two.')
make_table(story,
    ['File', 'v2.5.0.7 (B)', 'v2.6.0.6 (B)', 'Note'],
    [['xeyezHal.dll', '837,960', '153,968', '61 -> 79 exports; DRM additions'],
     ['xEyeUSB.dll', '133,448', '137,584', 'boot state machine, security handshake'],
     ['StereoCamera.dll', '100,680', '104,816', 'StereoCameraBridge C++ class'],
     ['EasyUSB.dll', '51,528', '51,568', 'transport, +40 B rebuild only'],
     ['A_a_v4.4_BG0313.bit', '235,930', '235,930', 'identical bitstream'],
     ['ens_code.ldr', '54,420', '54,420', 'identical Blackfin image'],
     ['8051 firmware', 'app_m8051.hex', '2 images', 'split per runtime PID'],
     ['SVM classifiers', 'V1-V3', 'V1-V4', 'V4 linear separators, 2012-04-11']],
    [0.28, 0.17, 0.17, 0.38],
    caption='Table 7: Key differences between the two driver-stack generations.')

# ============================= 9. Legal note ==================================
h1(story, '9. Legal Note')
body(story,
     'The re-extracted payload includes the vendor licence file licence.txt (GBK text), '
     'whose clause 2 prohibits reverse engineering, decompiling and disassembling of the '
     'software, and clause 1 reserves all intellectual-property rights to Shenzhen '
     'Taishan Online Technology ("TaiShan Online", the Toltech consumer brand). This '
     'project is private interoperability research on hardware owned by the researcher: '
     'the artifacts were obtained as plain, unencrypted HTTP downloads from the vendor '
     'public server with no technical protection measure circumvented, the DRM '
     'additions in v2.6.0.6 are documented but not attacked, and nothing is '
     'redistributed. The goal remains a Linux user-space driver and 3D-scanner use of '
     'the owner\'s device, the classic purpose for which interoperability reverse '
     'engineering is permitted in many jurisdictions. The licence text is preserved '
     'verbatim in the extraction archive for reference.')

# ==================== 10. Conclusions and next steps ==========================
h1(story, '10. Conclusions and Next Steps')
body(story,
     'This turn closed the evidence gap left by the sandbox resets and hardened the '
     'pipeline against future ones: everything the project knows is now reproducible '
     'from two live vendor URLs plus the hashed manifests in the turn archive. The two '
     'most consequential new facts for the driver roadmap are (1) the vendor-authored '
     'mode-switch evidence - register 0x02 = 0x0B plus the image FPGA script switches '
     'the sensor pair into a 2-sensor difference streaming mode, reachable through the '
     'existing GetImageBegin/GetImageData path in xeyezHal - and (2) the complete '
     'middleware API map, which turns iDongSimulator3 into a reference oracle for '
     'validating any reimplementation of the tracking pipeline.')
bullet(story, '<b>P0 - driver milestone:</b> replay the vendor boot sequence and register '
       'scripts verbatim against the live device, then attempt the first image-mode dump '
       '(reg 0x02 = 0x0B with 30fps_Global_image_2sensor_diff_1LED.dat, then '
       'GET_IMG_START).')
bullet(story, '<b>8051 disassembly:</b> the two HEX images have a clean vector+code layout; '
       'an 8051 disassembler pass would recover the sensor register protocol end-to-end '
       'and validate the data*.dat page semantics.')
bullet(story, '<b>Blackfin framing:</b> pin down the exact LDR block format of ens_code.ldr '
       '(likely a non-standard VDSP variant), then disassemble ep0_handler and usb_init '
       'to confirm the device-side command table.')
bullet(story, '<b>Classifier re-implementation:</b> the SVM files are plain text; '
       'implementing the 4-feature and V4 linear models host-side is trivial once blob '
       'features stream again.')
bullet(story, '<b>USBPcap capture</b> of a vendor calibration session remains the gate for '
       'any write path beyond register-script replay (standing safety rules: read-only '
       'on EP 0x82 until confirmed, never write the T24C16A EEPROM).')

# ======================= Appendix A: SHA-256 manifest =========================
h1(story, 'Appendix A: Key File SHA-256 Manifest')
body(story,
     'The complete manifest of all 2,475 extracted files ships as '
     '<font name="DejaVuSans" size="8.5">all_extracted_sha256.tsv</font> in the turn '
     'archive. The table below pins the identity of the files this report cites as '
     'evidence, so any future re-extraction can be compared byte-for-byte.')
KEY_FILES = [
    (V6 + '/A_a_v4.4_BG0313.bit', 'FPGA bitstream (Spartan-3)'),
    (V6 + '/ens_code.ldr', 'Blackfin BF527 image'),
    (V6 + '/app_m8051_2500.hex', '8051 image, PID 2500'),
    (V6 + '/app_m8051_2611.hex', '8051 image, PID 2611'),
    (V6 + '/xeyezHal.dll', 'HAL, v2.6.0.6 (79 exports)'),
    (V5 + '/xeyezHal.dll', 'HAL, v2.5.0.7 (61 exports)'),
    (V6 + '/EasyUSB.dll', 'USB transport'),
    (V6 + '/xEyeUSB.dll', 'boot state machine'),
    (V6 + '/xEyeClass.dll', 'reader thread / pipes'),
    (V6 + '/StereoCamera.dll', 'host triangulation bridge'),
    (V6 + '/classifying.dll', 'blob classifier'),
    (V6 + '/MiddleCameraCalib.dll', 'calibration middle layer'),
    (V6 + '/30fps/30fps_Global_blob_1LED.dat', 'FPGA script, blob mode'),
    (V6 + '/30fps/30fps_Global_image_2sensor_diff_1LED.dat', 'FPGA script, image mode'),
    (V6 + '/30fps/30fps_Global_0316_Cmos.txt', 'CMOS script, global shutter'),
    (V6 + '/Led_Scale_Fpga_v26.txt', 'LED scale registers'),
    (V6 + '/data0.dat', 'register page 3 script'),
    (V6 + '/xEyeUsb.ini', 'runtime configuration'),
    (V6 + '/xeyezHal.ini', 'mode configuration'),
    ('remote/idong_setup.exe', 'Inno installer stub (Sports King)'),
    ('remote/idong_setup-1.bin', 'Inno slice 1 (197,873,664 B)'),
    ('remote/dependent/xeyez_pc_v2.6.0.6/xeyez_Lobby.dat', 'NSIS payload v2.6.0.6'),
    ('remote/dependent/xeyez_pc_v2.5.0.7/xeyez_Lobby.dat', 'NSIS payload v2.5.0.7'),
    (SK + '/Dlls/iDongAction3.0/iDongSimulator3.exe', 'replay simulator'),
    (SK + '/Dlls/iDongAction3.0/iDongMark/iDongMarkManager.dll', 'tracking pipeline'),
    (SK + '/Dlls/iDongAction3.0/iDongAction.dll', 'action middleware v3.0'),
    (SK + '/Lobby/xEyeMsgEngine.dll', 'message engine / heartbeat'),
    (SK + '/Lobby/XeYeZSetting.dll', 'sensor settings UI'),
    (SK + '/Lobby/HandSignal.dll', 'gesture recognition UI'),
    (SK + '/Lobby/conf/camera_set/settings.sol', 'Flash camera settings'),
]
rows = []
for suffix, note in KEY_FILES:
    sha, size, path = sha_of(MAN, suffix)
    if sha == '-':
        # try to find by basename
        for p, (s2, z2) in MAN.items():
            if os.path.basename(p) == os.path.basename(suffix):
                sha, size, path = s2, z2
                break
    rows.append([os.path.basename(path), f'{size:,}', note, sha])
make_table(story, ['File', 'Size (B)', 'Role', 'SHA-256'],
           rows, [0.27, 0.09, 0.24, 0.40], mono_cols=(3,), keep=False,
           caption='Table 8: Evidence files and their SHA-256 hashes (full 64-hex digests).',
           cell_style=S['mono_sm'])

# ================= Appendix B: mode-switch register scripts ===================
h1(story, 'Appendix B: Mode-Switch Register Scripts')
body(story,
     'Verbatim contents of the two FPGA mode scripts and the CMOS script from the '
     'v2.6.0.6 stack. Every line is one register write ("0xRR,0xVV"); the extractor '
     'feeds them through command 3 in at most 8-byte chunks.')
BLOB = open(f'{ANALYSIS}/nsis/xeyez_pc_v2.6.0.6/xeyez_Lobby/$_6_/30fps/30fps_Global_blob_1LED.dat').read()
IMAGE = open(f'{ANALYSIS}/nsis/xeyez_pc_v2.6.0.6/xeyez_Lobby/$_6_/30fps/30fps_Global_image_2sensor_diff_1LED.dat').read()
CMOS = open(f'{ANALYSIS}/nsis/xeyez_pc_v2.6.0.6/xeyez_Lobby/$_6_/30fps/30fps_Global_0316_Cmos.txt').read()
code_block(story, BLOB, 'B.1 30fps_Global_blob_1LED.dat (blob mode, FPGA)')
code_block(story, IMAGE, 'B.2 30fps_Global_image_2sensor_diff_1LED.dat (image mode, FPGA)')
code_block(story, CMOS, 'B.3 30fps_Global_0316_Cmos.txt (CMOS, global shutter)')

# ==================== Appendix C: extraction script usage =====================
h1(story, 'Appendix C: Extraction Script Usage')
body(story,
     'Both tools are pure-Python (standard library only) and are included in the turn '
     'archive under scripts/. They keep the project reproducible from the two live '
     'vendor URLs alone.')
code_block(story,
"""# list an Inno installer without its slices
python3 inno_bin_extract.py idong_setup.exe

# extract only camera-relevant files, SHA-256 printed for EVERY file
python3 inno_bin_extract.py idong_setup.exe \\
    --extract outdir \\
    --manifest outdir/manifest.tsv \\
    --include '\\\\Lobby\\\\[^\\\\]+\\\\.(dll|exe|ini|dat|sol|txt|xml)$' \\
    --include '\\\\Dlls\\\\(iDongAction3\\\\.0\\\\|xEye)'

# fetch specific members out of a 1.5 GB remote zip (ranged GETs)
python3 fetch_zip_members.py http://download.i-dong.com/setup/yundongwang-ver/ \\
    idong_1.9.04_windows_yundongwang_A_setup.zip \\
    --member idong_setup.exe \\
    --member 'dependent/xeyez_pc_v2.6.0.6/xeyez_Lobby.dat' \\
    --outdir analysis/remote --manifest analysis/manifests/fetched.tsv""",
'C.1 Commands')
body(story,
     'Sample extraction output, showing the per-file SHA-256 line format introduced '
     'this turn:')
code_block(story,
"""[   2] SHA256 b07916ebfbd97198e657b1c2001a1c31d2e1fa339f2103db3a9818314adf5ea3  \\
      MD5 3a6f6f6c...  MD5_OK     248,136 B  Lobby/AboutXeYeZDlg.dll
[   7] SHA256 220218bba7feee801e317a1120d9996efc2fa8de7b4c42600de6d8fef34e613f  \\
      MD5 07f43d52...  MD5_OK     223,560 B  Lobby/CapStill.dll
[extract] done: 178 extracted, 0 failed, 21776 skipped by filter; a SHA256 was
      printed for every file extracted; manifest: extracted_sportsking.tsv""",
'C.2 Sample output (abridged)')

# ================================================================= build
doc = ReportDoc(OUT, pagesize=A4,
                leftMargin=MARGIN, rightMargin=MARGIN,
                topMargin=68, bottomMargin=56,
                title='Inside the i-Dong Stereo Camera - Hardware Research Report',
                author='Z.ai', creator='Z.ai',
                subject='Extraction and analysis of the Toltech idong Sensor V2.6 '
                        '(idong100 chipset) camera internals')
doc.multiBuild(story, onFirstPage=on_page, onLaterPages=on_page)
print('body built:', OUT)
