#!/usr/bin/env python3
"""gen_driver_report.py - build the body PDF of the i-Dong Rust ROS 2 driver
implementation draft (turn 13). Cover rendered separately via html2poster.js
and merged with pypdf afterwards.

Numbering plan (Step 3.5):
  cover  -> (separate PDF, no number)
  toc    -> roman i..
  ch 1   Executive Summary
  ch 2   Research: Standard Driver + ROS 2 Rust Landscape
  ch 3   Architecture: Four-Crate Workspace
  ch 4   Crate-by-Crate File Structure
  ch 5   The ROS 2 Node Design
  ch 6   Idiomatic Rust Patterns
  ch 7   Implementation Steps (P0-P4)
  ch 8   Test and Validation Strategy
  ch 9   Safety and Legal Constraints
  ch 10  Deliverables and Next Actions
  app A  Protocol Constants Quick Reference
  app B  Skeleton Cargo.toml and package.xml
"""
import os as _os
_HERE = _os.path.dirname(_os.path.abspath(__file__))  # research/scripts
# research/analysis; override with IDONG_ANALYSIS when the extracted
# installer payloads live somewhere else
ANALYSIS = (_os.environ.get('IDONG_ANALYSIS')
            or _os.path.join(_os.path.dirname(_HERE), 'analysis'))
REPO = _os.path.dirname(_os.path.dirname(_HERE))  # the repository root
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
OUT = f'{ANALYSIS}/figures/driver_body.pdf'
FIG = f'{ANALYSIS}/figures'

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
    story.append(Paragraph(f'•  {text}', S['bullet']))


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
               keep=True):
    widths = [r * AVAIL_W for r in ratios]
    assert abs(sum(ratios) - 1.0) < 0.01, 'ratios must sum to 1'
    data = [[Paragraph(f'<b>{h}</b>', S['tbl_h']) for h in headers]]
    for row in rows:
        data.append([Paragraph(str(c), S['tbl_mono'] if i in mono_cols else S['tbl'])
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
        story.append(t)
        if caption:
            story.append(Paragraph(caption, S['caption']))
    story.append(Spacer(1, 12))


def callout_row(story, stats):
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
                      'Idong100 Rust Driver with ROS 2 Output - Implementation Draft')
    canvas.setStrokeColor(ACCENT)
    canvas.setLineWidth(1.2)
    canvas.line(MARGIN, PAGE_H - 44, PAGE_W - MARGIN, PAGE_H - 44)
    canvas.setStrokeColor(BORDER)
    canvas.setLineWidth(0.5)
    canvas.line(MARGIN, 40, PAGE_W - MARGIN, 40)
    canvas.setFont('FreeSerif', 7.5)
    canvas.setFillColor(TEXT_MUTED)
    canvas.drawString(MARGIN, 28, 'idong100 Reverse-Engineering Project - Turn 13')
    bs = getattr(doc, '_body_start', None)
    if bs is not None and doc.page >= bs:
        label = str(doc.page - bs + 1)
    else:
        label = roman(doc.page)
    canvas.drawRightString(PAGE_W - MARGIN, 28, label)
    canvas.restoreState()


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
     'This document is the implementation draft for the next phase of the idong100 '
     'project: a <b>native, idiomatic Rust driver</b> for the Toltech i-Dong Sensor '
     'V2.6 stereo camera with a <b>ROS 2-compatible output layer</b>. It turns the '
     'protocol knowledge recovered in turns 1-12 (boot framing, firmware upload '
     'sequence, runtime command framing, register scripts, xEye3Ddata records, EEPROM '
     'calibration layout) into a concrete engineering plan: a four-crate Cargo '
     'workspace, a ROS 2 lifecycle node built on <b>rclrs</b>, the idiomatic Rust '
     'patterns that keep the driver maintainable, and a phased implementation '
     'roadmap with test gates.')
body(story,
     'The design follows one load-bearing principle found in every well-maintained '
     'open-source driver we surveyed (ros2_ouster, ros2_v4l2_camera, xtion-rs): '
     '<b>separate the hardware protocol from the robotics framework</b>. The protocol '
     'core is a pure-Rust crate with zero ROS and zero USB dependencies at its data '
     'layer, so it can be unit-tested against recorded golden vectors without '
     'hardware and reused outside ROS. The ROS 2 layer is a thin adapter crate that '
     'owns lifecycle, parameters, QoS and diagnostics. The USB layer is a third, '
     'swappable crate behind a transport trait so a mock transport can replay '
     'recorded sessions for CI.')
body(story,
     'On the ROS 2 side the design targets <b>rclrs</b>, the official Rust client '
     'library (ros2-rust). rclrs 0.5.0/0.8 ships nodes, pub/sub, services, actions, '
     'parameters and timers, is released on crates.io, and custom messages are '
     'generated by <b>rosidl_generator_rs</b> now released on the ROS buildfarm. '
     'One deliberate deviation from the surveyed C++ drivers is documented honestly: '
     'rclrs does not yet implement managed lifecycle nodes, so the node implements '
     'lifecycle semantics itself (an internal state machine plus a '
     '<font name="DejaVuSans" size="8.5">~/transition</font> service) rather than '
     'inheriting them from the framework - the design isolates this so a future '
     'migration to native lifecycle support is a one-crate change.')
callout_row(story, [
    ('4 crates', 'transport / core / msgs / ros2 - one concern each'),
    ('rclrs 0.8', 'official ROS 2 Rust client library target'),
    ('5 phases', 'P0-P4 with hardware safety gate before writes'),
])

# ============ 2. Research: standard driver + ROS 2 Rust landscape ==============
h1(story, '2. Research: What a Good Driver Looks Like')
h2(story, '2.1 Reference drivers surveyed')
body(story,
     'Three drivers were used as the structural benchmark, chosen because they are '
     'maintained, widely deployed, and documented their design decisions. First, '
     '<b>ros2_ouster_drivers</b> (Steve Macenski): its design document explicitly '
     'separates a sensor interface (hardware abstraction) from a data interface '
     '(packet processing and message conversion), builds on a lifecycle node with '
     'preallocated buffers at configure time, registers as a composable component, '
     'and exports a GetMetaData service. Second, <b>ros2_v4l2_camera</b>: its '
     'author\'s guidance in the "What makes a good sensor package" discourse thread - '
     'expose all settings as parameters, reconfigure live, prefer standard message '
     'types, publish diagnostics, stamp at capture - is adopted as the node '
     'checklist. Third, <b>xtion-rs</b> (PrimeSense PS1080 in pure Rust, same '
     'author pattern this project already follows): transport via '
     'rusb/libusb1-sys, a reader thread feeding frame decoders, error types that '
     'pin the failing boot step, golden-vector unit tests, hardware tests behind '
     'an #[ignore] flag.')
make_table(story,
    ['Driver', 'Language', 'Structure takeaway', 'Adopted here as'],
    [
     ['ros2_ouster', 'C++', 'lifecycle node + component; sensor/data interface split; '
      'GetMetaData service; preallocation at configure', 'lifecycle shape, service '
      'surface, buffer preallocation'],
     ['ros2_v4l2_camera', 'C++', 'every control a parameter; live reconfigure; '
      'standard messages; diagnostics; lazy subscription', 'parameter design, '
      'diagnostics, timestamps'],
     ['xtion-rs', 'Rust', 'rusb transport; reader thread; typed errors per boot '
      'step; golden vectors; #[ignore] hardware tests', 'core-crate internals, '
      'transport + stream design'],
    ],
    [0.16, 0.10, 0.42, 0.32],
    caption='Table 1 - Reference drivers and what was adopted from each')
h2(story, '2.2 The Rust-in-ROS-2 landscape (verified 2026-11)')
body(story,
     'rclrs is the official Rust client library for ROS 2 (repository '
     '<font name="DejaVuSans" size="8.5">ros2-rust/ros2_rust</font>). The 0.5.0 '
     'release announcement (August 2025) lists a fully async API compatible with '
     'tokio/async-std, Windows support, and Jazzy/Kilted/Rolling support; the '
     'current line is released on crates.io as rclrs 0.8, works across ROS '
     'distributions, and the README feature list covers publishers/subscriptions '
     'with QoS and loaned messages, services, actions, parameters, timers, guard '
     'conditions, clocks, graph queries, logging and executors. Building from '
     'source uses colcon with the colcon-cargo and colcon-ros-cargo plugins plus '
     'cargo-ament-build. Custom messages are defined in an ordinary rosidl package '
     'and compiled to Rust by rosidl_generator_rs, which is released on the ROS '
     'buildfarm.')
body(story,
     'Two caveats from the same sources shaped the design. The ros2-rust README '
     'states plainly that the library is still rapidly evolving with no stability '
     'guarantees - so the ROS-facing crate is kept thin and everything of value '
     'lives below it. And lifecycle (managed) nodes are absent from the feature '
     'list - so the node crate implements its own transition state machine, '
     'described in section 5.3, instead of relying on framework lifecycle. Both '
     'caveats are contained by the crate boundary.')
make_table(story,
    ['Capability (rclrs 0.8)', 'Status', 'Design consequence'],
    [
     ['Nodes, pub/sub, QoS, loaned msgs', 'supported', 'topics + QoS profiles '
      'map directly; no workaround needed'],
     ['Services / actions', 'supported', 'get_device_info service; image-mode '
      'capture as an action'],
     ['Parameters (+ services)', 'supported', 'all node settings are ROS '
      'parameters; live reconfigure via callbacks'],
     ['Timers (repeating/one-shot)', 'supported', 'keep-alive and diagnostics '
      'timers in the node crate'],
     ['Executors / worker pattern', 'supported', 'reader thread + spin loop '
      'composition documented in 5.2'],
     ['Managed lifecycle nodes', 'not listed', 'internal transition state '
      'machine + ~/transition service (5.3)'],
     ['API stability', 'no guarantees', 'ROS-facing crate kept thin; core '
      'crate has zero rclrs dependency'],
    ],
    [0.34, 0.16, 0.50],
    caption='Table 2 - rclrs capability map used for the node design')

# =================== 3. Architecture: four-crate workspace =====================
h1(story, '3. Architecture: The Four-Crate Workspace')
body(story,
     'The workspace splits the system along the three axes that change at '
     'different rates: the USB transport (changes with platform and libusb), the '
     'device protocol (changes with firmware findings), and the robotics façade '
     '(changes with the framework version). A fourth, tiny crate holds the '
     'message definitions so they can be generated for Rust, C++ and Python from '
     'one set of .msg/.srv files. The dependency arrows point strictly downward: '
     'idong-ros2 depends on idong-core and idong_msgs; idong-core depends on '
     'nothing but the standard library and a few utility crates; '
     'idong-transport depends on rusb and implements idong-core\'s Transport '
     'trait.')
figure(story, f'{FIG}/diagram_driver_ws.png',
       'Figure 1 - Workspace layout (left) and runtime dataflow (right)',
       max_h=PAGE_H * 0.42)
h2(story, '3.1 Why this split is the maintainable shape')
body(story,
     'Every surveyed driver that stayed healthy over years has this property: '
     'the part that talks to hardware can be tested without hardware, and the '
     'part that talks to the framework can be swapped without touching hardware '
     'logic. Concretely, the pure core crate compiles and tests on any machine '
     'with cargo test - no USB device, no ROS installation - because its only '
     'inputs are byte slices. The record/replay MockTransport means CI can run '
     'the full boot and stream state machines against recorded sessions. When '
     'rclrs makes a breaking change - it gave no stability guarantees - only the '
     'thin adapter crate needs a port. When new protocol facts are recovered '
     '(a new frame type, a decoded field), they land in the core crate with a '
     'unit test and nothing else moves.')
make_table(story,
    ['Crate', 'Depends on', 'Owns', 'Tested by'],
    [
     ['idong_msgs', 'rosidl runtime', 'Marker/HandleEvent messages, DeviceInfo '
      'service', 'generated-code compile test'],
     ['idong-core', 'thiserror, byteorder, log', 'frame codec, boot + command '
      'state machines, records parser, calibration, stream assembly, data '
      'model', 'golden vectors, fuzz, MockTransport replay'],
     ['idong-transport', 'rusb, libusb1-sys', 'device discovery, bulk I/O, '
     're-enumeration wait, udev helper', 'hardware-gated tests (#[ignore])'],
     ['idong-ros2', 'rclrs, idong-core, idong_msgs', 'lifecycle node, '
      'parameters, QoS, diagnostics, composition', 'node integration test '
      'against MockTransport'],
    ],
    [0.15, 0.20, 0.42, 0.23],
    caption='Table 3 - Crate responsibilities and their test strategy')
h2(story, '3.2 The data plane inside the core')
body(story,
     'The core exposes one façade type (Idong) and two streams. A reader thread '
     'owns the transport, performs bulk reads of up to 16 KiB, feeds a '
     'resynchronizing frame decoder, answers the 0xFFF keep-alive inline to '
     'bound its latency at one second, and publishes decoded MarkerFrame and '
     'Event values into a bounded crossbeam channel. Backpressure is explicit: '
     'if the consumer is slower than the device (50 frames per second), the '
     'oldest frame is dropped and a counter increments, which is exactly the '
     'behavior the vendor stack implemented with its PipeBufferSize window. '
     'Statistics (drops, resyncs, link quality window of the last 200 frames) '
     'live behind atomics so the ROS layer can read them without locks.')

# ==================== 4. Crate-by-crate file structure =======================
h1(story, '4. File Structure, Crate by Crate')
body(story,
     'The layout below is the draft skeleton: every file is named after one '
     'duty, no file mixes protocol facts with framework code, and every protocol '
     'constant that the disassembly gave us lives in exactly one place '
     '(protocol/mod.rs) with a doc comment citing its source. File names mirror '
     'the vocabulary already used across the research notes so a fact recovered '
     'in a future turn can be filed in seconds.')
code_block(story, '''idong100-rs/                     # cargo workspace root
  Cargo.toml                      # [workspace] members + shared deps
  rustfmt.toml                    # project style: group imports, 100 cols
  README.md                       # status, safety rules, build matrix
  docs/                           # PROTOCOL.md, RECOVERY/ (session backups)
  firmware/                       # NOT redistributed; sha256 manifest + verify
  idong_msgs/                     # rosidl package (plain .msg/.srv)
    package.xml                  #   build_type ament_cargo, rosidl deps
    CMakeLists.txt                #   rosidl_generator_rs registration
    msg/Marker.msg  msg/HandleEvent.msg  srv/DeviceInfo.srv
  idong/                          # crate "idong" = the pure core
    Cargo.toml                   #   no rusb, no rclrs
    src/lib.rs                   #   re-exports: Idong, MarkerFrame, Event
    src/error.rs                 #   IdongError, BootStep, ProtocolError
    src/protocol/mod.rs          #   constants: magics, types, timeouts, PIDs
    src/protocol/frames.rs       #   frame codec + resync decoder (fuzzed)
    src/protocol/boot.rs         #   _InitBlobUpload state machine
    src/protocol/cmd.rs          #   7F 55 runtime command engine (cmd 3/5/6/15)
    src/protocol/records.rs      #   xEye3Ddata parser (0x3BC) + 0x208 pipe form
    src/calib.rs                 #   T24C16A EEPROM layout + checksums
    src/stream.rs                #   reader thread, bounded channel, stats
    src/marker.rs                #   MarkerFrame/Marker/Quality data model
    src/handle.rs                #   RF handle events (TSOL_* acks)
    src/transport.rs             #   trait Transport {read,write,reset}
    src/mock.rs                  #   MockTransport: record/replay for CI
  idong-transport/                # hardware USB crate
    Cargo.toml                   #   rusb, libusb1-sys; feature "async"
    src/lib.rs                   #   list_devices(), DeviceState enum
    src/usb.rs                   #   open bootloader/runtime, bulk I/O
    src/reenum.rs                #   wait for PID 2500 after upload
    src/udev.rs                  #   rule text + install helper
  idong-ros2/                     # ROS 2 adapter crate
    Cargo.toml                   #   rclrs, idong, idong_msgs
    package.xml                  #   ament_cargo; exec deps
    src/lib.rs                   #   rclrs_create_node! registration
    src/node.rs                  #   IdongNode: two threads, channel drain
    src/lifecycle.rs             #   state machine + ~/transition service
    src/params.rs                #   declare/validate/reapply parameters
    src/pubsub.rs                #   topics, QoS profiles, converters
    src/diagnostics.rs           #   stats -> diagnostic_msgs/DiagnosticArray
    launch/idong.launch.py       #   lifecycle-manager bring-up
  examples/                       # in idong crate
    list.rs                      #   enumerate (read-only)
    probe_bootloader.rs          #   EP 0x82 probe (NEVER writes)
    upload_firmware.rs           #   guarded full boot (--i-know-the-risks)
    dump_frames.rs               #   runtime marker dump to JSONL
  tests/                          # in idong crate
    frames_golden.rs  eeprom_decode.rs  mock_replay.rs  hardware.rs  # #[ignore]''',
    title='Workspace skeleton (draft - the tree this document specifies)')
body(story,
     'Two conventions keep this tree honest over time. First, the examples are '
     'executables of the pure core wired to the transport crate, so a tool such '
     'as dump_frames.rs runs identically on the bench and inside CI with '
     '--transport mock. Second, the hardware-gated tests live in the same crate '
     'but behind cargo test -- --ignored, never in the default set: a fresh '
     'clone runs green on a laptop, and the bench machine opts in explicitly.')

# ======================== 5. The ROS 2 node design ============================
h1(story, '5. The ROS 2 Node Design')
h2(story, '5.1 Topics, services and parameters')
body(story,
     'The node publishes one main data topic and one event topic, offers one '
     'service, and mirrors every vendor runtime setting as a ROS parameter. '
     'Marker data has no standard ROS message type (PoseArray loses the '
     'quality/tracking fields and the frame counter), so a custom message in '
     'idong_msgs is the right call - the same decision ros2_ouster made for its '
     'lidar metadata. The QoS split follows the sensor-data guidance: the marker '
     'stream is best-effort keep-last 5 (a late frame is worthless), while handle '
     'button events are reliable (a lost click is a lost user action).')
make_table(story,
    ['Interface', 'Type', 'QoS', 'Content'],
    [
     ['/idong/markers', 'idong_msgs/MarkerArray', 'sensor-data '
      '(best-effort, keep-last 5)', 'up to 10-15 markers: slot, x/y/z mm, '
      'quality, age; header carries receive time + device frame counter'],
     ['/idong/events', 'idong_msgs/HandleEvent', 'reliable, keep-last 10',
      'handle found/lost/timeout, button up/down, pairing state'],
     ['/idong/diagnostics', 'diagnostic_msgs/DiagnosticArray', 'reliable',
      'link quality (green/yellow/red), drops, resyncs, noise level'],
     ['/idong/time_ref', 'sensor_msgs/TimeReference', 'reliable',
      'device frame counter -> host clock pairs for drift estimation'],
     ['get_device_info (service)', 'idong_msgs/DeviceInfo', '-',
      'serial, DSP version, calibration summary, firmware hashes'],
     ['start_image_mode (service)', 'std_srvs/Trigger', '-',
      'switches register script to image mode (P2 feature, guarded)'],
    ],
    [0.24, 0.20, 0.22, 0.34],
    caption='Table 4 - Node interface surface')
make_table(story,
    ['Parameter', 'Type', 'Default', 'Meaning (vendor source)'],
    [
     ['serial_number', 'string', '""', 'expect this device only '
      '(xeyez stack handle-match)'],
     ['frame_id', 'string', 'idong', 'header.frame_id of all outputs'],
     ['publish_hz_min', 'int', '30', 'below this the node flags degraded '
      'link (vendor xEyeFrame=50)'],
     ['quality_threshold', 'int', '50', 'slot quality gate '
      '(xEyeClass filter: quality >= 50)'],
     ['link_quality_window', 'int', '200', 'frames averaged for the '
      'green/yellow/red level (vendor RingSize=200)'],
     ['buffer_depth', 'int', '5', 'bounded channel depth = QoS depth'],
     ['firmware_dir', 'string', '""', 'user-supplied vendor firmware dir '
      '(sha256-verified before any upload)'],
     ['enable_handle_events', 'bool', 'true', 'NeedHandleMatch=1 analog'],
     ['i_know_the_write_risks', 'bool', 'false', 'must be set for any '
      'interface that writes to the device'],
    ],
    [0.24, 0.09, 0.13, 0.54],
    caption='Table 5 - Parameter surface (all reconfigurable where the '
            'hardware allows)')
h2(story, '5.2 Threading model')
body(story,
     'Two threads, one channel - the shape that kept xtion-rs and the vendor '
     'stack simple. The reader thread is owned by idong-core: bulk reads, frame '
     'decoding, keep-alive replies, quality accounting. The executor thread '
     'spins the rclrs node, drains the channel, stamps messages with the '
     'receive time (the device gives a frame counter, not a clock - the '
     'time_ref topic exposes the pairs so downstream nodes can estimate drift, '
     'the pattern recommended in the sensor-package thread when PTP is '
     'unavailable), converts to idong_msgs types and publishes. Parameters are '
     'applied through a shared Arc<Mutex<TrackingConfig>> that the reader '
     'thread snapshots per frame - no locks on the hot path, no atomics '
     'beyond the statistics counters.')
h2(story, '5.3 Lifecycle semantics without framework lifecycle')
body(story,
     'rclrs today has no managed-lifecycle node type. The node therefore '
     'implements the same four states (unconfigured, inactive, active, '
     'finalized) as an internal enum with the standard transition names '
     '(configure, cleanup, activate, deactivate, shutdown), exposed as a '
     '~/transition service and reflected in a /idong/status topic. Allocation '
     'happens at configure (channels, buffers, message capacity), USB discovery '
     'at activate, teardown at shutdown - the ros2_ouster bring-up order. '
     'When rclrs gains native lifecycle support the whole mechanism lives in '
     'lifecycle.rs and swaps out without touching the rest of the node.')
code_block(story, '''// idong-ros2/src/lib.rs (shape, ~40 lines)
rclrs_create_node!(IdongNode, "idong_node", |_ctx, node| {
    let params = Params::declare(&node)?;          // table 5 surface
    let state  = LifecycleState::Unconfigured;      // 5.3 machine
    let (tx, rx) = channel::bounded(params.buffer_depth);
    // /idong/markers, /idong/events, /idong/diagnostics, /idong/time_ref
    let pubs  = PubSub::create(&node)?;
    let svc   = node.create_service::<_, DeviceInfo_>("get_device_info",
                 IdongNode::on_device_info)?;
    // ~/transition service + get_device_info wired; reader thread starts
    // on configure -> open_or_boot(firmware_dir) when i_know_the_write_risks
    Ok(IdongNode { params, state, rx, pubs, svc, core: None })
});

// idong/src/lib.rs (facade, shape)
pub struct Idong { /* transport + reader handle + identity */ }
impl Idong {
    pub fn open_or_boot(fw: &FirmwareSet) -> Result<Self, IdongError>;
    pub fn identity(&self) -> &DeviceIdentity;         // EEPROM + DSP
    pub fn calibration(&self) -> &StereoCalib;         // 28 f32, OpenCV form
    pub fn set_mode(&mut self, m: Mode) -> Result<(), IdongError>; // blob/image
    pub fn frames(&self) -> &Receiver<MarkerFrame>;    // bounded, drop-oldest
    pub fn events(&self) -> &Receiver<Event>;          // handle/link events
    pub fn stats(&self) -> StreamStats;                // atomics snapshot
}''',
    title='Entry-point skeletons (abridged, illustrative - not final code)')

# ========================= 6. Idiomatic Rust patterns =========================
h1(story, '6. Idiomatic Rust Patterns')
body(story,
     'The patterns below are what makes the crate read like modern Rust rather '
     'than translated C. They are listed with the concrete idiom names so the '
     'implementation phase can check each one off.')
make_table(story,
    ['Concern', 'Idiom', 'Applied where'],
    [
     ['Errors', 'thiserror enums; no panics on device data; BootStep '
      'variant pins the failing state', 'error.rs; every decode path'],
     ['Wire formats', 'zerocopy-style plain structs + byteorder reads; '
      'decode returns Result, never unwraps', 'frames.rs, records.rs'],
     ['State machines', 'enum + match in a function taking &mut self; '
      'states are data, transitions are total', 'boot.rs, lifecycle.rs'],
     ['Concurrency', 'one owning reader thread; crossbeam bounded '
      'channel; drop-oldest backpressure; stats via atomics', 'stream.rs'],
     ['Transport', 'small trait object; MockTransport implements the same '
      'trait and records every byte', 'transport.rs, mock.rs'],
     ['API surface', 'small pub surface; re-exports at crate root; '
      'builder-style config structs', 'lib.rs'],
     ['Fuzzing', 'decode entry points as fuzz targets (cargo-fuzz); '
      'arbitrary bytes must never panic', 'frames.rs, records.rs, calib.rs'],
     ['Docs', 'doc comments with the disassembly source for every '
      'constant; #![deny(unsafe_code)] in core', 'protocol/mod.rs'],
     ['Logging', 'log crate facade in core; ROS logging sink wired only '
      'in the node crate', 'whole core crate'],
     ['Testing', 'golden vectors as files under tests/data; hardware '
      'tests #[ignore]; CI runs mock replay', 'tests/'],
    ],
    [0.15, 0.48, 0.37],
    caption='Table 6 - Rust idiom checklist for the implementation')
body(story,
     'One pattern deserves emphasis because the protocol makes it dangerous to '
     'skip: <b>decode functions must be total</b>. Every record the device emits '
     'is attacker-controlled input in the fuzzing sense - the frame assembler '
     'will regularly meet garbage during resynchronization after a USB error. '
     'The decoder therefore returns Result values for every field, validates '
     'the frame counter continuity the same way the vendor did (delta at most '
     'one), and treats a bad magic as a resync event, never as a panic. The '
     'fuzz targets exist precisely to prove that property.')

# ==================== 7. Implementation steps (P0-P4) =========================
h1(story, '7. Implementation Steps')
body(story,
     'The roadmap is phased so that every phase ends with something verifiable, '
     'and the one hardware-safety gate sits before any code is allowed to write '
     'to the device. Phase 1 alone produces a publishable crate (the pure core '
     'with tests); phase 3 is where the camera actually boots under our driver; '
     'phase 4 ends with a tagged release.')
figure(story, f'{FIG}/diagram_roadmap.png',
       'Figure 2 - Phased roadmap P0-P4; the dashed box is the write gate',
       max_h=PAGE_H * 0.42)
h2(story, '7.1 Phase 0 - consolidation (no code)')
bullet(story, '<b>P0.1</b> Write docs/PROTOCOL.md: every wire fact from turns '
       '1-12 with its source citation (which DLL, which RVA), one constants '
       'table - this becomes protocol/mod.rs verbatim.')
bullet(story, '<b>P0.2</b> Assemble golden vectors: recorded OD frames from '
       'iDongSimulator3 sessions, the real EEPROM dump, the vendor register '
       'scripts as test fixtures under tests/data/.')
bullet(story, '<b>P0.3</b> Freeze the v2.6.0.6 firmware file set with SHA-256 '
       'from the turn-12 manifest (analysis/reports/all_extracted_sha256.tsv); '
       'the FirmwareSet verifier pins these hashes.')
h2(story, '7.2 Phase 1 - the pure core (green CI, no hardware)')
bullet(story, '<b>P1.1</b> Scaffold the workspace, rustfmt, clippy in CI; '
       'error.rs + protocol/mod.rs first (constants with citations).')
bullet(story, '<b>P1.2</b> frames.rs codec + resync decoder + fuzz target; '
       'records.rs xEye3Ddata parser against golden frames (unit tests assert '
       'the 10 x 88-byte slot layout, the +0x3B8 mode flag, quality gate 50).')
bullet(story, '<b>P1.3</b> calib.rs: parse the T24C16A dump, verify all '
       'checksums, assert f = 432.33 px mean and |T| = 100.00 mm against the '
       'known-good values.')
bullet(story, '<b>P1.4</b> boot.rs and cmd.rs state machines over MockTransport; '
       'the replay test asserts the exact vendor sequence: DSP-start, '
       'ready-to-load, count+CRC, 64-byte type-3 packets, CRC ack, device-ID '
       'poll, DSP reset; runtime cmd framing 7F 55 with byte-swapped u16.')
bullet(story, '<b>P1.5</b> stream.rs reader thread with the bounded channel; '
       'mock replay drives it at synthetic 50 fps and asserts drop accounting.')
h2(story, '7.3 Phase 2 - read-only bring-up (hardware, still zero writes)')
bullet(story, '<b>P2.1</b> idong-transport crate: list_devices, open bootloader '
       '(0471:2600), read-only probe of EP 0x82 - the Rust port of '
       'idong_probe.py which must never write.')
bullet(story, '<b>P2.2</b> udev rule + README access instructions; verify probe '
       'runs unprivileged.')
bullet(story, '<b>P2.3</b> THE GATE: capture one full vendor boot with USBPcap '
       'on Windows; byte-compare against the decoded _InitBlobUpload sequence; '
       'resolve open items (multi-firmware staging order, CRC variant, '
       'count/CRC framing). Nothing downstream runs on hardware until this '
       'transcript matches.')
h2(story, '7.4 Phase 3 - full lifecycle on hardware')
bullet(story, '<b>P3.1</b> upload_firmware example behind the '
       '--i-know-the-write-risks flag: full boot state machine, firmware '
       'sha256-verified first; abort on CRC mismatch, never blind-retry.')
bullet(story, '<b>P3.2</b> re-enumeration wait to 0471:2500; claim MI_04; '
       'confirm UVC/UAC interfaces appeared (lsusb/uvcvideo log).')
bullet(story, '<b>P3.3</b> runtime: device-ID query, keep-alive answered '
       'within 1 s, first OD record decoded; 60-second stability check, zero '
       'resyncs, frame-counter deltas at most 1.')
bullet(story, '<b>P3.4</b> marker stream soak: reflective ball at 1-4 m, '
       'JSONL dump, quality/drop statistics published; handle events if a '
       'wristband is available.')
h2(story, '7.5 Phase 4 - ROS 2 node and release')
bullet(story, '<b>P4.1</b> idong_msgs rosidl package + idong-ros2 node crate '
       'over MockTransport in CI (colcon build with colcon-ros-cargo).')
bullet(story, '<b>P4.2</b> composable registration, launch file with the '
       'transition bring-up, parameter file with sane defaults.')
bullet(story, '<b>P4.3</b> diagnostics topic, time_ref pairs, live parameter '
       'reapply; image-mode service behind the risk flag (register scripts '
       'replayed verbatim only).')
bullet(story, '<b>P4.4</b> 24-hour soak with replug/recovery, docs.rs build, '
       'changelog, tag v0.1.0; publish crates (idong, idong-transport) and the '
       'ROS package (idong-ros2) for Jazzy/Kilted/Rolling.')

# =================== 8. Test and validation strategy =========================
h1(story, '8. Test and Validation Strategy')
body(story,
     'Each layer gets the test type that actually catches its bugs. The codec '
     'layer is exercised by golden vectors and fuzzing (garbage in, Result out, '
     'never a panic). The state machines are exercised by MockTransport replay '
     'of recorded sessions - including failure injections (timeout at '
     'ready-to-load, CRC mismatch, device-ID poll exhaustion) because those are '
     'precisely the paths the vendor code strings told us about. The node layer '
     'is integration-tested against the same mock, asserting topic rates, QoS '
     'behavior and parameter reapply. The hardware layer runs only on the bench '
     'behind --ignored flags, and its acceptance numbers come from the vendor '
     'configuration (50 fps stream, quality window 200, keep-alive 1 s).')
make_table(story,
    ['Level', 'What runs', 'Acceptance'],
    [
     ['Unit (core)', 'codec golden vectors; EEPROM checksums + known '
      'calibration values; record parser layout', 'cargo test green; '
      'fuzz targets survive 10 min each'],
     ['Replay (core)', 'MockTransport scripts: full boot, each failure '
      'injection, 50 fps stream', 'exact vendor sequence asserted '
      'byte-for-byte; drop accounting exact'],
     ['Integration (node)', 'rclrs node over mock; launch file; parameter '
      'file; service calls', 'marker topic at 50 Hz for 60 s; events '
      'ordered; diagnostics reflect injected stats'],
     ['Hardware (bench)', 'probe (read-only), guarded upload, runtime '
      'claim, soak', 'P3.3/P3.4 acceptance above; zero unexpected writes '
      'in a USBPcap audit'],
     ['Release', '24 h soak with replug; docs.rs; clippy -D warnings',
      'no leaks (RSS flat), no lockups, v0.1.0 tagged'],
    ],
    [0.16, 0.42, 0.42],
    caption='Table 7 - Test matrix by layer')

# ==================== 9. Safety and legal constraints ===========================
h1(story, '9. Safety and Legal Constraints')
body(story,
     'These rules are part of the design, not an afterthought, because the '
     'device is a scarce piece of hardware and two of its memories are not '
     'host-writable without risk. They are enforced structurally: the '
     'write-capable entry points require an explicit opt-in flag, and the '
     'EEPROM write path is simply not implemented in v1.')
bullet(story, 'No speculative writes: the only permitted TX sequences are the '
       'ones decoded from xEyeUSB.dll/xeyezHal.dll; anything else must fail '
       'closed with a ProtocolError.')
bullet(story, 'Read-only until the USBPcap gate (P2.3) is passed; the '
       'write-capable examples refuse to run without --i-know-the-write-risks '
       'and the node parameter i_know_the_write_risks.')
bullet(story, 'Firmware files are SHA-256-pinned to the turn-12 manifest before '
       'upload; a mismatch aborts before the first byte leaves the host.')
bullet(story, 'The T24C16A EEPROM is never written by the driver '
       '(factory calibration lives there); EpromRead is implemented, '
       'EpromWriteInterface is not exposed.')
bullet(story, 'All firmware is host-uploaded volatile state (FPGA bitstream, '
       'Blackfin LDR, 8051 hex): power-cycling reverts any bad experiment, '
       'which is why the risk is contained.')
bullet(story, 'Legal note carried from turn 12: the vendor licence.txt (GBK, '
       'Taishan Online/Toltech) has a no-reverse-engineering clause; this is '
       'private interoperability research on a legally acquired device, the DRM '
       'components are documented but not attacked, and no vendor firmware is '
       'redistributed with the crate - users supply their own files, verified '
       'by hash.')

# ============== 10. Deliverables and next actions ==============================
h1(story, '10. Deliverables of This Draft and Next Actions')
body(story,
     'This document is the contract for the implementation phase: it fixes the '
     'crate split, the file tree, the ROS interface surface, the idiom '
     'checklist and the phase gates. The companion deliverable is the '
     'scaffold archive (idong100_rust_scaffold.zip) containing the workspace '
     'tree exactly as section 4 specifies - Cargo.toml workspace, package.xml '
     'files, skeleton src files with doc-comment stubs, README with the safety '
     'rules, and the launch file - so that the next session can begin at '
     'P1.1 with cargo check (memory limits: cargo check only, never a full '
     'build) and already have the structure in place.')
body(story,
     'The immediate next actions, in order: (1) confirm this draft against '
     'the live rclrs version pinned in the scaffold lock; (2) run P0.1 and '
     'write docs/PROTOCOL.md from the turn-12 FINDINGS; (3) P1.1-P1.2 codec '
     'and parser with golden vectors; (4) obtain the USBPcap capture from the '
     'Windows bench to open the write gate. Everything in this plan is '
     'derived from verified artifacts of the vendor stack - no step depends '
     'on guesses about the device.')

# ================================ Appendix A ==================================
h1(story, 'Appendix A - Protocol Constants Quick Reference')
body(story,
     'Constants the driver will encode in protocol/mod.rs, consolidated from '
     'the turn-9/11/12 disassembly notes. Every row is already verified '
     'against a decompiled vendor binary; the open items (staging order, CRC '
     'variant) are the exact purpose of the P2.3 gate.')
make_table(story,
    ['Constant', 'Value', 'Source'],
    [
     ['USB VID / PID bootloader', '0471:2600', 'xeyez.inf, both driver gens'],
     ['USB PID runtime (vendor MI_04)', '0471:2500', 'xeyez.inf'],
     ['Bulk endpoints', '0x02 OUT / 0x82 IN', 'boot-loader descriptors + '
      'EasyUSB.dll'],
     ['Boot magic H2D / D2H', '0x7f7f5550 / 0x7f7f444f', 'xEyeUSB.dll'],
     ['Boot types', '2 count+CRC; 3 packet/CRC result; 7 DSP reset',
      '_InitBlobUpload'],
     ['Boot packet size / timeouts', '64 B; 3 s ready, 6 s per packet, '
      '300 ms x 10 device-ID poll', 'xEyeUSB.dll timeouts'],
     ['Runtime framing', '7F 55; cmd@[2]; u16 arg@[6..7] byte-swapped; '
      'payload max 56 B', 'xeyezHal builder @0x10003010'],
     ['Runtime commands', '3 reg write; 5 blob read; 6 GET_IMG_START; '
      '15 classifier write', 'xeyezHal call sites'],
     ['Keep-alive', 'RX 0xFFF -> TX 0xFFE within 1 s', 'xEyeClass reader'],
     ['Runtime read chunk', '4096 B (sys) / 16 KiB accumulation',
      'xeyez.sys + EasyUSB.dll'],
     ['Marker record', '0x3BC B: 10 x 88 B slots at +0; quality i32 @ '
      'slot+0x24 (>= 50); mode flag @ +0x3B8', 'xEyeClass GetADataFromUSB'],
     ['Stream rate', '50 fps (xEyeFrame=50, USBComFPS=35)',
      'xEyeUsb.ini'],
     ['EEPROM T24C16A', 'tag ef0123456789abcd; records 0x0100/0x0180/'
      '0x0192/0x0300 mirror +0x400; [payload][sum&0xFF][0xFF]',
      '24c16 dump decode'],
    ],
    [0.30, 0.38, 0.32],
    caption='Table 8 - Constants encoded into protocol/mod.rs (appendix A)',
    keep=False)

# ================================ Appendix B ==================================
h1(story, 'Appendix B - Skeleton Manifests')
body(story,
     'The two manifest files that make the workspace a valid ROS 2 Rust '
     'package set; full contents are in the scaffold archive. The ament_cargo '
     'build type plus the colcon-ros-cargo plugin is the documented way '
     'ros2-rust packages build inside a colcon workspace.')
code_block(story, '''# idong-ros2/package.xml (excerpt)
<package format="3">
  <name>idong_ros2</name>
  <version>0.1.0</version>
  <description>Rust ROS 2 driver node for the Toltech i-Dong sensor</description>
  <maintainer email="maintainer@example.org">idong100 project</maintainer>
  <license>MIT</license>
  <buildtool_depend>ament_cargo</buildtool_depend>
  <build_type>ament_cargo</build_type>
  <depend>rclrs</depend>
  <depend>idong_msgs</depend>
</package>

# Cargo.toml (workspace root, excerpt)
[workspace]
resolver = "2"
members  = ["idong", "idong-transport", "idong-ros2"]

[workspace.package]
edition = "2021"
license = "MIT"

[workspace.dependencies]
thiserror = "2"
byteorder = "1"
crossbeam-channel = "0.5"
log = "0.4"
rusb = "0.9"

# idong/Cargo.toml keeps the core pure:
#   [dependencies] thiserror, byteorder, log, crossbeam-channel
#   (no rusb, no rclrs - enforced by CI dependency audit)''',
    title='package.xml and workspace Cargo.toml (abridged)')

# ------------------------------------------------------------------ build
doc = ReportDoc(OUT, pagesize=A4,
                leftMargin=MARGIN, rightMargin=MARGIN,
                topMargin=0.95 * inch, bottomMargin=0.85 * inch,
                title='Idong100 Rust Driver with ROS 2 Output - '
                      'Implementation Draft',
                author='Z.ai', creator='Z.ai',
                subject='Native idiomatic Rust driver and ROS 2 node design '
                        'for the Toltech i-Dong Sensor V2.6')
doc.multiBuild(story, onFirstPage=on_page, onLaterPages=on_page)
print(f'body: {OUT}')
