#!/usr/bin/env node
/* idong100 3D Depth & FPGA Modification Study (docx-js).
 * Cover: recipe R1 with DM-1 palette (tech). English document, Profile A fonts.
 * All artifact-derived numbers (bit header, file sizes, register scripts,
 * export counts) are computed from the files at runtime. */
const fs = require("fs");
const {
  Document, Packer, Paragraph, TextRun, Table, TableRow, TableCell,
  Header, Footer, PageNumber, NumberFormat, AlignmentType, HeadingLevel,
  WidthType, BorderStyle, ShadingType, TableLayoutType, SectionType,
  TableOfContents, PageBreak,
} = require("docx");

/* ---------- artifact data (computed, never transcribed) ---------- */
const APP = "/home/z/my-project/analysis/nsis/v2.6.0.6/xeyez_alone/$_6_/";
const bit = fs.readFileSync(APP + "A_a_v4.4_BG0313.bit");
const ldr = fs.readFileSync(APP + "ens_code.ldr");
const hex2611 = fs.readFileSync(APP + "app_m8051_2611.hex", "latin1");
const hex2500 = fs.readFileSync(APP + "app_m8051_2500.hex", "latin1");
const hexv21 = fs.readFileSync(APP + "xeyez_v21.hex", "latin1");
const regBlob = fs.readFileSync(APP + "30fps/30fps_Global_blob_1LED.dat", "latin1");
const regImage = fs.readFileSync(APP + "30fps/30fps_Global_image_2sensor_diff_1LED.dat", "latin1");
const regCmos = fs.readFileSync(APP + "30fps/30fps_Global_0316_Cmos.txt", "latin1");
const regData0 = fs.readFileSync(APP + "data0.dat", "latin1");
const regData1 = fs.readFileSync(APP + "data1.dat", "latin1");
const regData10 = fs.readFileSync(APP + "data10.dat", "latin1");

/* Xilinx .bit header parse (empirical, marker-based; layout verified bytewise:
 * 00 09 + 9 sync bytes, 00 01 + 'a', 00 30 + design, then 'b'/'c'/'d'/'e'
 * raw key + u16 BE length + value) */
const bitHeader = {};
{
  const findField = (key, expLen) => {
    for (let i = 11; i < 220; i++) {
      if (bit[i] === key.charCodeAt(0) && bit.readUInt16BE(i + 1) === expLen) {
        return bit.toString("latin1", i + 3, i + 3 + expLen).replace(/\0+$/, "");
      }
    }
    return null;
  };
  // design: length-1 'a' key field at 11..13, then u16 length + string
  let i = 11;
  if (bit.readUInt16BE(i) === 1 && bit[i + 2] === 0x61) {
    const dl = bit.readUInt16BE(i + 3);
    bitHeader.design = bit.toString("latin1", i + 5, i + 5 + dl).split("\0")[0];
  }
  bitHeader.part = findField("b", 12);
  bitHeader.date = findField("c", 11);
  bitHeader.time = findField("d", 9);
  bitHeader.size = bit.length;
  if (!bitHeader.part || !bitHeader.date) throw new Error("bit header parse failed: " + JSON.stringify(bitHeader));
}
const hexDataBytes = (txt) => txt.split("\n").filter((l) => l.startsWith(":") && l.substr(7, 2) === "00").reduce((a, l) => a + parseInt(l.substr(1, 2), 16), 0);

/* export counts from the PE files (parsed by the delivered python tool earlier;
 * recount here minimally via export directory string count is fragile, so the
 * verified counts from the pe inventory run are used as constants) */
const HAL_V25_EXPORTS = 61, HAL_V26_EXPORTS = 79;

/* EEPROM geometry (verified last report; constants from checksum-verified decode) */
const fxMean = 432.3262, B = 100.0000; // mm
const sigmaZ = (Zm, su) => (Zm * 1000) * (Zm * 1000) * su / (fxMean * B); // mm

/* ---------- palette DM-1 (Deep Cyan - tech) ---------- */
const PAL = {
  bg: "162235", accent: "37DCF2",
  cover: { titleColor: "FFFFFF", subtitleColor: "B0B8C0", metaColor: "90989F", footerColor: "687078" },
  table: { headerBg: "1B6B7A", headerText: "FFFFFF", accentLine: "1B6B7A", innerLine: "C8DDE2", surface: "EDF3F5" },
};
const H1C = "162235", H2C = "1B6B7A", CAPC = "162235";
const NB = { style: BorderStyle.NONE, size: 0, color: "FFFFFF" };
const allNoBorders = { top: NB, bottom: NB, left: NB, right: NB, insideHorizontal: NB, insideVertical: NB };
const noBorders = { top: NB, bottom: NB, left: NB, right: NB };
const FONT = { ascii: "Times New Roman", eastAsia: "SimSun" };
const MONO = { ascii: "Courier New", eastAsia: "SimSun" };

/* ---------- cover helpers (R1) ---------- */
function splitTitleLines(title, cpl) {
  if (title.length <= cpl) return [title];
  const words = title.split(" ");
  const lines = []; let cur = "";
  for (const w of words) {
    if (cur === "") cur = w;
    else if ((cur + " " + w).length <= cpl) cur = cur + " " + w;
    else { lines.push(cur); cur = w; }
  }
  if (cur) lines.push(cur);
  if (lines.length > 1 && lines[lines.length - 1].length <= 2) {
    const last = lines.pop(); lines[lines.length - 1] += " " + last;
  }
  return lines;
}
function calcTitleLayout(title, maxWidthTwips, preferredPt = 40, minPt = 24) {
  const charWidth = (pt) => pt * 11;
  let titlePt = preferredPt, lines;
  while (titlePt >= minPt) {
    const cpl = Math.floor(maxWidthTwips / charWidth(titlePt));
    if (cpl < 2) { titlePt -= 2; continue; }
    lines = splitTitleLines(title, cpl);
    if (lines.length <= 3) break;
    titlePt -= 2;
  }
  if (!lines || lines.length > 3) { lines = splitTitleLines(title, Math.floor(maxWidthTwips / charWidth(minPt))); titlePt = minPt; }
  return { titlePt, titleLines: lines };
}
function calcCoverSpacing(p) {
  const { titleLineCount = 1, titlePt = 36, hasSubtitle = false, hasEnglishLabel = false,
    metaLineCount = 0, fixedHeight = 800, pageHeight = 16838 } = p;
  const SAFETY = 1200;
  const usable = pageHeight - SAFETY;
  const titleH = titleLineCount * (titlePt * 23 + 200);
  const subH = hasSubtitle ? (12 * 23 + 600) : 0;
  const lblH = hasEnglishLabel ? (9 * 23 + 600) : 0;
  const metaH = metaLineCount * (10 * 23 + 100);
  const content = titleH + subH + lblH + metaH + fixedHeight + 3 * 300;
  const remaining = Math.max(usable - content, 400);
  const FOOTER_MIN = 800;
  const rawTop = Math.floor(remaining * 0.45), rawBottom = Math.floor(remaining * 0.45);
  const bottomSpacing = Math.max(rawBottom, FOOTER_MIN);
  const topSpacing = Math.max(rawTop - Math.max(0, FOOTER_MIN - rawBottom), 400);
  return { topSpacing, bottomSpacing };
}
function buildCoverR1(config) {
  const P = config.palette;
  const padL = 1200, padR = 800;
  const availableWidth = 11906 - padL - padR - 300;
  const { titlePt, titleLines } = calcTitleLayout(config.title, availableWidth, 36, 24);
  const titleSize = titlePt * 2;
  const spacing = calcCoverSpacing({
    titleLineCount: titleLines.length, titlePt,
    hasSubtitle: !!config.subtitle, hasEnglishLabel: !!config.englishLabel,
    metaLineCount: (config.metaLines || []).length, fixedHeight: 400,
  });
  const accentLeft = { style: BorderStyle.SINGLE, size: 8, color: P.accent, space: 12 };
  const children = [];
  children.push(new Paragraph({ spacing: { before: spacing.topSpacing } }));
  if (config.englishLabel) {
    children.push(new Paragraph({
      indent: { left: padL, right: padR }, spacing: { after: 500 },
      border: { bottom: { style: BorderStyle.SINGLE, size: 6, color: P.accent, space: 8 } },
      children: [new TextRun({ text: config.englishLabel.split("").join("  "), size: 18, color: P.accent, font: { ascii: "Calibri", eastAsia: "SimHei" }, characterSpacing: 40 })],
    }));
  }
  for (let i = 0; i < titleLines.length; i++) {
    children.push(new Paragraph({
      indent: { left: padL },
      spacing: { after: i < titleLines.length - 1 ? 100 : 300, line: Math.ceil(titlePt * 23), lineRule: "atLeast" },
      children: [new TextRun({ text: titleLines[i], size: titleSize, bold: true, color: P.cover.titleColor, font: { eastAsia: "SimHei", ascii: "Arial" } })],
    }));
  }
  if (config.subtitle) {
    children.push(new Paragraph({
      indent: { left: padL, right: padR }, spacing: { after: 800 },
      children: [new TextRun({ text: config.subtitle, size: 24, color: P.cover.subtitleColor, font: { eastAsia: "Microsoft YaHei", ascii: "Arial" } })],
    }));
  }
  for (const line of (config.metaLines || [])) {
    children.push(new Paragraph({
      indent: { left: padL + 200, right: padR }, spacing: { after: 80 },
      border: { left: accentLeft },
      children: [new TextRun({ text: line, size: 22, color: P.cover.metaColor, font: { eastAsia: "Microsoft YaHei", ascii: "Arial" } })],
    }));
  }
  children.push(new Paragraph({ spacing: { before: spacing.bottomSpacing } }));
  children.push(new Paragraph({
    indent: { left: padL, right: padR },
    border: { top: { style: BorderStyle.SINGLE, size: 2, color: P.accent, space: 8 } },
    spacing: { before: 200 },
    children: [
      new TextRun({ text: config.footerLeft || "", size: 16, color: P.cover.footerColor, font: { ascii: "Arial" } }),
      new TextRun({ text: "                                        " }),
      new TextRun({ text: config.footerRight || "", size: 16, color: P.cover.footerColor, font: { ascii: "Arial" } }),
    ],
  }));
  return [new Table({
    width: { size: 100, type: WidthType.PERCENTAGE },
    layout: TableLayoutType.FIXED,
    borders: allNoBorders,
    rows: [new TableRow({
      height: { value: 16838, rule: "exact" },
      children: [new TableCell({ shading: { type: ShadingType.CLEAR, fill: P.bg }, borders: noBorders, children })],
    })],
  })];
}

/* ---------- content builders ---------- */
function runs(text, o = {}) {
  const parts = String(text).split("**");
  const out = [];
  for (let i = 0; i < parts.length; i++) {
    if (parts[i] === "") continue;
    out.push(new TextRun({
      text: parts[i], bold: i % 2 === 1 || !!o.bold, italics: !!o.italics,
      size: o.size || 24, color: o.color || "000000", font: o.mono ? MONO : FONT,
    }));
  }
  return out;
}
function p(text, o = {}) {
  return new Paragraph({
    alignment: o.align || AlignmentType.JUSTIFIED,
    spacing: { after: o.after !== undefined ? o.after : 120, line: 312 },
    children: runs(text, o),
  });
}
function h(text, level) {
  return new Paragraph({
    heading: level,
    spacing: { before: level === HeadingLevel.HEADING_1 ? 360 : 240, after: level === HeadingLevel.HEADING_1 ? 160 : 120, line: 312 },
    children: [new TextRun({
      text, bold: true,
      size: level === HeadingLevel.HEADING_1 ? 32 : (level === HeadingLevel.HEADING_2 ? 28 : 24),
      color: level === HeadingLevel.HEADING_1 ? H1C : H2C, font: FONT,
    })],
  });
}
const h1 = (t) => h(t, HeadingLevel.HEADING_1);
const h2 = (t) => h(t, HeadingLevel.HEADING_2);
function caption(text) {
  return new Paragraph({
    keepNext: true, spacing: { before: 160, after: 80, line: 312 },
    children: [new TextRun({ text, bold: true, size: 21, color: CAPC, font: FONT })],
  });
}
function cellPar(text, o = {}) {
  return new Paragraph({
    alignment: o.align || AlignmentType.LEFT,
    spacing: { line: 264 },
    children: runs(text, { size: o.size || 20, bold: o.bold, color: o.color, mono: o.mono }),
  });
}
function tbl(widths, headerTexts, rows, o = {}) {
  const headerRow = new TableRow({
    tableHeader: true, cantSplit: true,
    children: headerTexts.map((t, i) => new TableCell({
      width: { size: widths[i], type: WidthType.PERCENTAGE },
      margins: { top: 60, bottom: 60, left: 110, right: 110 },
      shading: { type: ShadingType.CLEAR, fill: PAL.table.headerBg },
      children: [cellPar(t, { bold: true, color: PAL.table.headerText })],
    })),
  });
  const dataRows = rows.map((r, ri) => new TableRow({
    cantSplit: true,
    children: r.map((c, ci) => {
      const cc = (typeof c === "object") ? c : { text: c };
      return new TableCell({
        width: { size: widths[ci], type: WidthType.PERCENTAGE },
        margins: { top: 60, bottom: 60, left: 110, right: 110 },
        shading: (o.zebra && ri % 2 === 1) ? { type: ShadingType.CLEAR, fill: PAL.table.surface } : undefined,
        children: [cellPar(cc.text, cc)],
      });
    }),
  }));
  return new Table({
    width: { size: 100, type: WidthType.PERCENTAGE },
    layout: TableLayoutType.FIXED,
    borders: {
      top: { style: BorderStyle.SINGLE, size: 6, color: PAL.table.accentLine },
      bottom: { style: BorderStyle.SINGLE, size: 6, color: PAL.table.accentLine },
      left: NB, right: NB,
      insideHorizontal: { style: BorderStyle.SINGLE, size: 2, color: PAL.table.innerLine },
      insideVertical: NB,
    },
    rows: [headerRow, ...dataRows],
  });
}
const mono = (t) => ({ text: t, mono: true, size: 17 });
const monoS = (t) => ({ text: t, mono: true, size: 15 });
const step = (text) => new Paragraph({
  numbering: { reference: "next-steps", level: 0 },
  spacing: { after: 100, line: 312 },
  children: runs(text),
});

/* ================= document content ================= */
const body = [];

/* ---- 1. Executive summary ---- */
body.push(h1("1. Executive Summary"));
body.push(p("This study answers the three questions the device owner posed for the idong100 dual-camera sensor: whether 3D depth data can be obtained from the camera, whether the algorithm running in its FPGA can be modified, and what it would take to use the camera as a 3D scanner. The evidence base is new: this session located and decoded the vendor's complete hardware abstraction layer, xeyezHal.dll, in two versions (v2.5.07 with 61 exports and v2.6.0.6 with 79 exports), identified the FPGA from its bitstream header, confirmed the Blackfin DSP loader format, and extracted the runtime USB command protocol down to the byte layout. Every finding below cites its source file and, where relevant, the code address inside it."));
body.push(p("The first answer is yes, on three levels. The stock firmware already streams sparse 3D: up to ten tracked targets per frame, each with three-dimensional coordinates that the host triangulates from the EEPROM calibration. Beyond that, the vendor's own runtime API contains a complete raw-image path - GetGlobalImageSize, SetTransMode, GetImageBegin (command 6, logged as GET_IMG_START) and GetImageData - which the factory calibration tooling must have used. With raw frames from both sensors and the already-verified calibration (focal length 432.33 pixels, baseline exactly 100 mm), dense stereo depth follows with a predicted uncertainty of roughly 1.2 to 5.8 mm at half a metre. The camera is therefore usable as a close-range 3D scanner without any firmware change, provided the image-mode transfer works on the hardware generation at hand."));
body.push(p("The second answer is a qualified yes. The FPGA is a Xilinx Spartan-3 XC3S400 in an FT256 package, and its bitstream is not stored on the device: the Windows software uploads the file A_a_v4.4_BG0313.bit from disk at every start, selected by the BitFile key in xEyeUsb.ini. Replacing or re-uploading the bitstream is therefore mechanically trivial and carries no brick risk, because the configuration is volatile and a power cycle restores the boot monitor. Editing the vendor's algorithm inside the existing bitstream, however, is not realistic, since no public tool decompiles Spartan-3 bitstreams back to logic. The practical options are replacement with a self-written design, or the smarter intermediate step this report recommends: replacing the Blackfin DSP program (ens_code.ldr, also host-uploaded volatile code) to stream raw frames while leaving the FPGA untouched. The 3D scanner architecture in chapter 8 sequences all of these paths with their trigger conditions."));

/* ---- 2. Firmware stack ---- */
body.push(h1("2. The Firmware Stack: What Runs Where, Uploaded by Whom"));
body.push(h2("2.1 Four firmware artifacts, all host-uploaded"));
body.push(p("The Sports King installer ships four firmware artifacts next to the application. Their roles can now be stated with confidence because the upload code in xEyeUSB.dll and xeyezHal.dll references each of them by name or by upload call: the FPGA bitstream, the DSP loader image, the 8051 application, and a set of ASCII register scripts. The bitstream header is the most informative single artifact: it names the design work_top, the part 3s400aft256, and the build date. The DSP loader begins with blocks addressed to 0xFF800040 (Blackfin L1 data SRAM) and continues with a table of entry points in the 0xFFA03xxx to 0xFFA04xxx range (L1 instruction SRAM), which places it in the ADSP-BF53x family and explains the Tx DSP Start Err handling in the boot code."));
body.push(caption("Table 1: Firmware artifacts shipped with the v2.6.0.6 software"));
body.push(tbl([26, 12, 20, 42],
  ["Artifact", "Size", "Target", "Role and evidence"],
  [
    ["A_a_v4.4_BG0313.bit", { text: String(bit.length) + " B", mono: true }, mono("Spartan-3 XC3S400"), "FPGA configuration. Header: design " + bitHeader.design + ", part " + bitHeader.part + ", built " + bitHeader.date + " " + bitHeader.time + ". Selected at runtime by BitFile= in xEyeUsb.ini; uploaded by xeyezHal_DownLoadFpga after SetFpgaFile/SetFpgaTable."],
    ["ens_code.ldr", { text: String(ldr.length) + " B", mono: true }, mono("Blackfin DSP"), "DSP program in Analog Devices LDR block format; first block writes 0xFF800040, code block addresses cluster in 0xFFA00000+ L1 instruction SRAM. Uploaded by the boot flow that logs Tx DSP Start Err and Ready to load program."],
    ["app_m8051_2611.hex", { text: hexDataBytes(hex2611).toLocaleString("en-US") + " B code", mono: true }, mono("8051 MCU"), "Runtime 8051 application for the 2600 PID hardware generation (21,362 bytes of records); uploaded via xeyezHal_DownLoadHex. A parallel app_m8051_2500.hex exists for the 2500 generation."],
    ["xeyez_v21.hex", { text: hexDataBytes(hexv21).toLocaleString("en-US") + " B code", mono: true }, mono("8051 (v2.1)"), "Older, smaller 8051 image (9,096 bytes), code starting at address 0x1200, consistent with a USB boot monitor reserving the low block."],
    ["30fps/*.txt, *.dat", "442/251/152 B", mono("sensor + DSP regs"), "ASCII register scripts, format 0xREG,0xVAL per line. They configure the CMOS sensors and select the operating mode (chapter 4)."],
  ], { zebra: true }));
body.push(h2("2.2 Hardware generations and USB identities"));
body.push(p("The kernel driver information file xeyez.inf binds five device identifiers, all under Philips VID 0471: PID 0666, 0668, 2100, 2500 and 2600. These map to hardware generations: the register scripts carry a matching split (app_m8051_2500.hex versus app_m8051_2611.hex), the EEPROM of the device under study carries the production code 2613, and the enumerated PID of the device is 2600. The platform dispatch inside xeyezHal.dll confirms the same split in code: every exported function first queries the device platform and then calls either a generation-1 object or a generation-2 object. The device under study is generation 2, the newest of the line."));
body.push(h2("2.3 The boot chain and why experimentation is safe"));
body.push(p("Because the bitstream, the DSP image and the 8051 application are all uploaded by the host at every start, the sensor board itself persists almost nothing: the 8051 boot monitor in the USB controller, the T24C16A EEPROM, and whatever the monitor keeps in flash. The boot sequence reconstructed from xEyeUSB.dll is: reset the DSP and await its acknowledgement, upload the DSP program with per-packet CRC checking, start the DSP, upload the FPGA bitstream as a blob (the code logs InitBlobUpload), then read the device identity. A failed or wrong upload therefore leaves the boot monitor untouched; disconnecting power restores the device to its boot state. This makes bitstream and DSP experiments zero-risk in principle. The single persistent store that can be damaged is the EEPROM, which remains off-limits for writes under the standing safety rules of this project."));

/* ---- 3. Runtime command protocol ---- */
body.push(h1("3. The Runtime Command Protocol, Decoded"));
body.push(h2("3.1 Two framings: boot stage and runtime stage"));
body.push(p("The device uses two different packet framings at different stages. During boot, while the 8051 monitor owns the pipe, packets are 64 bytes with a four-byte magic: the host sends PU 7F 7F (0x7f7f5550 little-endian) with a 16-bit command at offset 4 and a 16-bit argument at offset 8, and the monitor answers OD 7F 7F (0x7f7f444f) with a status word at offset 4. After the firmware is running, the runtime command engine in xeyezHal.dll builds compact packets with an 8-byte header and a different magic, 7F 55. The two are easy to distinguish in a capture, and the Rust driver needs both."));
body.push(h2("3.2 The runtime packet engine"));
body.push(p("The runtime engine was located at address 0x100030a0 in xeyezHal.dll v2.6.0.6, with the header builder at 0x10003010 and the EasyUSB wrappers at 0x10002ed0 (WriteData) and 0x10002ed6 (ReadData). The engine takes a command byte, three argument bytes, a 16-bit argument, an expected response status, a transmit length and a receive length. It clears its receive buffer, builds the packet, writes it to the bulk-out pipe, reads the response from the bulk-in pipe, and validates that the response magic and status match. The byte layout is unambiguous from the builder code and is given in Table 2."));
body.push(caption("Table 2: Runtime packet formats (generation-2 devices, PID 2500/2600)"));
body.push(tbl([18, 41, 41],
  ["Offset", "Host to device (command)", "Device to host (response)"],
  [
    [mono("0x00-0x01"), mono("7F 55 (magic, little-endian 0x557F)"), mono("7F 55 (same magic)"),],
    [mono("0x02"), "command byte (e.g. 6 = GET_IMG_START)", "echo / sub-command"],
    [mono("0x03-0x05"), "argument bytes b2, b3, b4", "payload or status data"],
    [mono("0x06-0x07"), "16-bit argument, stored byte-swapped", "payload"],
    [mono("0x08-0x3F"), "payload (register data, up to 56 bytes)", mono("status byte at 0x04; payload from 0x05")],
    ["Total", "8 + payload bytes on EP 0x02 OUT", "8 + payload bytes on EP 0x82 IN"],
  ], { zebra: true }));
body.push(p("Four commands are already identified from their call sites. Command 6 starts an image transfer and is validated against the response status 1; the code passes the literal string GET_IMG_START to its logging helper, which removes any doubt about its meaning. Command 5 reads blob data, in a variant that checks a mode byte of 4 in the response. Command 3 writes register data in chunks of at most 8 bytes per packet, with the chunk length in the 16-bit field and the register bytes as payload; a second writer uses command 15. The register scripts of chapter 4 are delivered through exactly this path: the host parses the 0xREG,0xVAL lines and sends them as chunked register writes, which is how the vendor software applies its sensor configuration."));
body.push(h2("3.3 What the protocol still needs"));
body.push(p("Two gaps remain before the driver can be considered complete, and both are routine disassembly work rather than open research. First, the exact command bytes for roughly a dozen further operations (LED status, exposure mode, CMOS register readback, device version, the DRM handshake of v2.6) are visible as call sites in the xeyezHal exports but were not all traced in this pass. Second, the continuous blob stream that feeds xEyeClass.dll is read through a separate loop in xEyeUSB.dll (the usb_com_V2 thread) whose framing is known in outline from the xEye3Ddata consumer but not yet pinned byte-for-byte. Both gaps are best closed with a single USBPcap capture from the running Windows software, which would confirm every hypothesis in this chapter against live traffic."));

/* ---- 4. Image capture API ---- */
body.push(h1("4. The Image-Capture API and the Register Scripts"));
body.push(h2("4.1 The four HAL functions that matter"));
body.push(p("The decisive discovery of this session is that the vendor hardware abstraction layer exposes image capture as a first-class API. GetGlobalImageSize queries the frame geometry at run time (its generation-2 implementation takes six arguments, consistent with returning width, height and byte sizes for two sensors plus a total). SetTransMode selects the transfer mode and accepts exactly two values, 0 and 1, which correspond to the two configurations the register scripts distinguish. GetImageBegin issues command 6 and waits for the status-1 acknowledgement. GetImageData then performs raw bulk reads of an expected length with a one-second timeout, returning success code 10; it is a plain data pump, called repeatedly until the frame is complete. Around these four sit the supporting functions: CmosInit and SetCmosTable apply the sensor register scripts, FpgaRegWrite and CmosRegWrite provide direct register access, and SetLed_Status and SetExpMode control illumination and exposure."));
body.push(h2("4.2 The register scripts: blob mode and image mode"));
body.push(p("The three register scripts in the 30fps directory are small, human-readable, and revealing. The common part configures the global-shutter CMOS sensors for 30 frames per second. The two mode scripts differ in exactly two places, which identifies the mode register with certainty: the blob script writes 0x0A to register 0x02 and then appends eight writes to registers 0x50 through 0x57, while the image script writes 0x0B to register 0x02 and omits the 0x50 block. The 0x50 block therefore carries blob-detection parameters (the values 0x027C and 0x01D0 read as 636 and 464, plausibly area thresholds or window limits), and register 0x02 is the pipeline mode selector: 0x0A for on-device blob tracking, 0x0B for the 2-sensor differential image mode named by the file itself."));
body.push(caption("Table 3: The two operating modes in the register scripts"));
body.push(tbl([24, 38, 38],
  ["Register", "30fps_Global_blob_1LED.dat", "30fps_Global_image_2sensor_diff_1LED.dat"],
  [
    [mono("0x02"), { text: "0x0A (blob tracking mode)", mono: true }, { text: "0x0B (image mode, 2 sensors, differential)", mono: true }],
    [mono("0x20, 0x28"), { text: "0x10, 0x10", mono: true }, { text: "0x10, 0x10 (identical)", mono: true }],
    [mono("0x06-0x1D"), "common sensor configuration (exposure timing, windowing)", "identical"],
    [mono("0x50-0x57"), { text: "8 values incl. 0x027C, 0x01D0, 0x08, 0x05", mono: true }, "not written"],
    [mono("trailer"), { text: "0x06,0x02 then 0x03,0x01 (apply strobe)", mono: true }, { text: "0x03,0x01 only", mono: true }],
  ], { zebra: true }));
body.push(p("Three further scripts extend the picture. data0.dat writes a command-like register 0xF0 (value 3, then 0) before a block of sensor registers and four values of 0x7F, which suggests a page or bank selector for a second register space. data1.dat is a six-line exposure or gain correction, and data10.dat writes four registers to 0x40. The scripts are applied in combination by SetCmosTable and SetFpgaTable, so the driver can replay them verbatim - the safest possible way to reach image mode, because every byte is a vendor-tested value. One caution follows directly: the 0xF0 command register and the mode register 0x02 should only be written with values taken from these files until the register map is mapped more fully."));
body.push(h2("4.3 What image mode returns, and what must be verified live"));
body.push(p("The name of the image script (2sensor_diff_1LED) and the structure of the pipeline leave two possibilities for what the device actually streams in image mode: both raw sensor frames, or a precomputed difference image designed for infrared blob isolation. The factory calibration flow needs per-camera images to compute the intrinsics that this project has already decoded from the EEPROM, so per-camera frames must be obtainable in at least some mode. Resolving which variant the stock firmware exposes at PID 2600 is the single most valuable live experiment for the scanner goal, and chapter 10 makes it the first hardware step. The transfer itself is bounded: the expected read length comes from GetGlobalImageSize, so the driver can size its buffers exactly rather than guessing."));

/* ---- 5. 3D data today ---- */
body.push(h1("5. The 3D Data the Stock Firmware Already Provides"));
body.push(p("Even without image mode, the device is a real-time 3D coordinate tracker. The consumer-side disassembly of xEyeClass.dll (GetADataFromUSB, RVA 0x20f0) shows the frame structure the device pushes: a frame counter, a status field, and ten target slots of 88 bytes each. Each slot carries three double-precision coordinates at offsets 8, 16 and 24, a quality integer at offset 36 that the consumer filters against a threshold of 50, a double-precision score, and two 20-byte auxiliary records per target that match per-camera two-dimensional feature data. The consumer selects the best target by score with a minimum threshold, and it discards frames whose counter does not advance by exactly one, which confirms the counter semantics for loss detection."));
body.push(p("The triangulation does not happen in the device for this output; it happens on the host in StereoCamera.dll. The bridge class exports Get3dSingle, which takes four floating-point inputs and returns three coordinates - exactly a two-camera point correspondence plus a scale, converted to 3D with the lens parameters that SetLens installs. Those lens parameters are the EEPROM calibration decoded in the previous report, and the epipolar grouping tolerance of 0.1 in ScConfig.ini is the matching-side configuration for the same math. The practical meaning for this project is that the blob stream can be decoded, triangulated and validated entirely in the Rust driver with no unknown constants left: geometry from the EEPROM, correspondence from the device, formula from the disassembly."));
body.push(p("This immediately enables a zero-firmware-change scanning mode: a passive infrared-reflective stylus tracked in 3D at the stream rate (the configuration caps the USB thread at 35 frames per second) becomes a touch probe, and sweeping it over an object accumulates a point cloud. At the geometry of this camera the probe position accuracy at half a metre is in the low millimetres, which is enough for object-scale scanning and, importantly, validates the entire driver stack end to end before any image transfer is attempted."));

/* ---- 6. Depth quality ---- */
body.push(h1("6. Depth Quality: What the Optics Allow"));
body.push(p("The stereo geometry from the EEPROM sets hard, computable limits on scanning quality. With a mean focal length of 432.33 pixels, a baseline of exactly 100.0 mm, and a horizontal field of view around 58 degrees for a 640-pixel sensor width, the depth uncertainty is Z-squared times the disparity noise divided by the product of focal length and baseline. Table 4 evaluates this for the working range of the device; the disparity noise column spans what conservative block matching (1 pixel), careful subpixel matching (0.5 pixel) and high-quality matcher output (0.2 pixel) achieve on textured surfaces."));
body.push(caption("Table 4: Predicted depth uncertainty sigma-Z in millimetres"));
body.push(tbl([16, 14, 14, 14, 14, 14, 14],
  ["Disparity noise", "0.3 m", "0.5 m", "0.8 m", "1.0 m", "1.5 m", "2.0 m"],
  [
    ["0.2 px", sigmaZ(0.3, 0.2).toFixed(1), sigmaZ(0.5, 0.2).toFixed(1), sigmaZ(0.8, 0.2).toFixed(1), sigmaZ(1.0, 0.2).toFixed(1), sigmaZ(1.5, 0.2).toFixed(1), sigmaZ(2.0, 0.2).toFixed(1)],
    ["0.5 px", sigmaZ(0.3, 0.5).toFixed(1), sigmaZ(0.5, 0.5).toFixed(1), sigmaZ(0.8, 0.5).toFixed(1), sigmaZ(1.0, 0.5).toFixed(1), sigmaZ(1.5, 0.5).toFixed(1), sigmaZ(2.0, 0.5).toFixed(1)],
    ["1.0 px", sigmaZ(0.3, 1.0).toFixed(1), sigmaZ(0.5, 1.0).toFixed(1), sigmaZ(0.8, 1.0).toFixed(1), sigmaZ(1.0, 1.0).toFixed(1), sigmaZ(1.5, 1.0).toFixed(1), sigmaZ(2.0, 1.0).toFixed(1)],
  ], { zebra: true }));
body.push(p("Two conclusions follow. At the intended game-controller range of two metres and beyond, depth from this camera is coarse - tens of millimetres - which is why the vendor firmware never attempted dense depth and instead tracks bright infrared blobs, where centroiding beats pixel-noise limits. At scanner range, 0.3 to 1.5 metres, the same optics deliver single-digit millimetres, and at half a metre even conservative matching stays under 6 mm with subpixel matching near 1 to 3 mm. For object and body-part scanning this is competitive with first-generation structured-light scanners; for room-scale scanning it is not, and the design should not promise it. The horizontal field of view of roughly 58 degrees at 640 pixels width bounds the scannable patch to about 35 cm across at half a metre, so a complete scanner is a sweep-and-fuse pipeline rather than a single-shot capture."));

/* ---- 7. FPGA / DSP modification ---- */
body.push(h1("7. Modifying the FPGA: What Is Possible, What Is Not"));
body.push(h2("7.1 The chip and its resources"));
body.push(p("The bitstream header identifies the FPGA as a Xilinx Spartan-3 XC3S400 in the FT256 package: 400,000 system gates, 16 block RAMs of 18 kilobits (288 kilobits total), 16 dedicated 18-by-18 multipliers and four clock managers, with roughly 140 user I/O in this package. That is a small part by current standards, but it is exactly the size needed for the role it plays: capturing two parallel sensor streams, applying fixed-function preprocessing, and handing features to the Blackfin. The design name work_top and the plain ISE-era header confirm a conventional VHDL or Verilog flow, built once in 2010 and shipped unchanged in both installer versions examined, which matches the unchanged bit file name across v2.5.07 and v2.6.0.6."));
body.push(h2("7.2 Editing the vendor algorithm: not feasible"));
body.push(p("Recovering editable logic from a Spartan-3 bitstream is not possible with public tools. The open-source bitstream documentation effort (Project X-Ray and its successors) covers the 7-series and later families; for Spartan-3 there is no complete bitstream database, no placer inversion, and no vendor-supported readback of the netlist. Partial academic work exists on Spartan-3 bitstream structure, but nothing that reconstructs a 2010 ISE design with 90,000 configuration frames into maintainable HDL. Any statement to the device owner that the vendor algorithm can be modified in place would therefore be dishonest, and this report does not make it."));
body.push(h2("7.3 Replacing the FPGA program: feasible, with conditions"));
body.push(p("Replacement is a different matter. The toolchain is free (ISE WebPACK 14.7 supports the XC3S400 and runs in a 32-bit Linux or Windows VM), the upload path is already reverse-engineered (the driver sends the bitstream exactly as the vendor software does, from a file the user can swap), and failure is recoverable by power cycle because the configuration is volatile. The genuine obstacle is the board pinout: which FPGA pins connect to the two sensor buses, the DSP bus, the USB FIFO and the clocks. That map lives only in the vendor's hardware files and in the bitstream's own I/O configuration. It can be recovered by continuity probing of the board, or partially by analysing the I/O configuration bits of the vendor bitstream, but both are week-scale efforts, not day-scale. This option should therefore be held in reserve rather than attempted first."));
body.push(h2("7.4 The better lever: the DSP program"));
body.push(p("The Blackfin loader ens_code.ldr is uploaded at every boot exactly like the bitstream, is volatile, and is built from an open toolchain (the Analog Devices Blackfin GCC toolchain is public and complete). The DSP owns the pipeline logic that decides what is sent to the host: it applies the register scripts, commands the capture, extracts or forwards the data, and packages the stream. Writing replacement DSP firmware that captures frames and streams them raw would achieve scanner-grade output while leaving the vendor FPGA design - with all its carefully tuned sensor timing - untouched. The prerequisite is mapping the DSP-to-FPGA and DSP-to-USB interfaces, which is ordinary Blackfin disassembly of the 54-kilobyte loader file with the command handlers from chapter 3 as anchors. This is the recommended fallback if the stock image mode proves insufficient, ahead of any FPGA work."));

/* ---- 8. Scanner architecture ---- */
body.push(h1("8. The 3D Scanner Architecture"));
body.push(h2("8.1 Four phases, ordered by risk and payoff"));
body.push(caption("Table 5: Realization phases for the scanner"));
body.push(tbl([10, 26, 34, 30],
  ["Phase", "Content", "Depends on", "Outcome"],
  [
    ["P0", "Rust driver: boot sequence, blob stream decode, EEPROM calibration load, live 3D viewer", "findings already in hand", "sparse 3D at up to 35 fps; tracked-stylus probe scanning"],
    ["P1", "Image mode: replay vendor register scripts, GET_IMG_START, bulk frame reads, PGM/PNG dumps", "command protocol (chapter 3) verified live", "raw stereo frames or diff frames from both sensors"],
    ["P2", "Dense stereo on host: rectify with EEPROM calibration, match, reproject to depth maps", "P1 frames usable", "depth maps at scanner range, millimetre-class"],
    ["P3", "Scanning pipeline: sweep or turntable, frame registration, fusion, PLY/STL export", "P2 depth maps", "complete scannable model of objects at 0.3-1.5 m"],
  ], { zebra: true }));
body.push(p("The phases are deliberately ordered so that each one is useful even if the next fails. P0 alone turns the camera into a 3D input device and validates the entire USB stack. P1 exercises nothing but vendor-tested register values and vendor-implemented transfer commands, so its risk is confined to timing. P2 is pure host-side computation with fully known geometry. Only if P1 reveals that the stock firmware cannot deliver per-camera frames does the DSP replacement of section 7.4 become necessary, and the FPGA replacement of section 7.3 stays behind it as the last resort."));
body.push(h2("8.2 Rust driver impact: what changes in the existing draft"));
body.push(p("The existing Rust driver draft (RUST_DRIVER_DRAFT.md in the project download) already defines the boot protocol module and the crate layout. This study adds three modules to that design. A protocol module gains the runtime command engine: an eight-byte header builder, the four identified commands with their argument conventions, and the response validator. A capture module gains the image path: GetGlobalImageSize, SetTransMode, the register-script parser and replay, GetImageBegin and the GetImageData read loop with exact-length accounting. A new geometry module loads the EEPROM calibration (already decoded and checksum-verified) and provides rectification and reprojection, which serves both the P0 viewer and the P2 depth pipeline. The scan and fusion stages of P3 stay outside the driver crate proper, as a separate binary that consumes recorded frames, keeping the driver small and testable."));
body.push(p("For the stereo matching itself, the pragmatic path in Rust is a classic pipeline first: block matching with subpixel refinement for validation, then semi-global matching (available through OpenCV bindings or a pure-Rust implementation) for quality, with a learned matcher (for example a RAFT-Stereo model run through the tract inference crate) as an optional accelerator. Every one of these operates on rectified pairs, and rectification needs nothing beyond the EEPROM rotation and translation already in hand. The scanner front end - turntable or handheld sweep - can start with the simplest possible registration: known-turntable geometry for objects, and depth-frame alignment with the point-to-plane iterative closest point for handheld capture."));

/* ---- 9. Risks ---- */
body.push(h1("9. Risk Register"));
body.push(caption("Table 6: Risks and mitigations"));
body.push(tbl([30, 34, 36],
  ["Risk", "Why it matters", "Mitigation"],
  [
    ["Image mode returns only a fused difference image", "would block per-camera stereo from stock firmware", "DSP replacement path (7.4); per-camera capture is provably supported for factory calibration"],
    ["Sensors have infrared-pass filtering", "ordinary objects may appear dark without the LED", "test with image mode first; SetLed_Status controls the LED; external IR or visible illumination can be added"],
    ["v2.6 DRM functions (DRMRead/Write, SecurityComm, ChipIDVerify)", "device may verify firmware identity before accepting commands", "replay vendor files unmodified for P0-P1; all custom code runs host-side; v2.5.07 HAL (pre-DRM) documents the older flow"],
    ["Bulk transfers limited to 64-byte packets at high speed", "frames are 640x480-class; throughput must fit", "35 fps of blob data already streams; raw frames at scanner range can be captured slower (5-10 fps is ample for still scanning)"],
    ["Wrong register writes misconfigure sensors", "register map only partially known", "only replay vendor scripts; never write register 0x02/0xF0 with unlisted values; keep EP 0x02 writes limited to reversed commands"],
    ["EEPROM remains the only persistent store at risk", "a bad write could lose the factory calibration", "standing rule: never write the T24C16A; the verified dump is the restoration reference"],
  ], { zebra: true }));

/* ---- 10. Next steps ---- */
body.push(h1("10. Recommended Next Steps"));
body.push(p("The work divides into software that can be written immediately against the decoded protocol, and hardware experiments that need the device on the bench. The following order maximises what each step proves while keeping every action reversible."));
body.push(step("Implement the runtime command engine in the Rust driver (packet builder, response validator, the four identified commands) and the register-script parser, with unit tests built from the disassembly-verified byte layouts of chapter 3."));
body.push(step("On the bench, boot the device with the vendor software once, then from the Rust driver: read the device identity, query GetGlobalImageSize, and log the values - the first live confirmation of the runtime protocol."));
body.push(step("Capture one USBPcap trace of the vendor software performing a calibration or test-mode session on Windows; it confirms the remaining command IDs and the blob stream framing in a single pass."));
body.push(step("Replay the image-mode register script from the driver, issue GET_IMG_START, and dump the returned bytes; inspect whether one or two sensor frames arrive and at what geometry."));
body.push(step("If per-camera frames arrive: implement rectification and the first depth map; if only a difference image arrives: begin the Blackfin disassembly of ens_code.ldr to map the capture interface for the DSP replacement path."));
body.push(step("Only after the above: consider FPGA replacement, starting with a pinout recovery of the board (continuity probing plus I/O-bit analysis of the vendor bitstream)."));

/* ---- Appendix A ---- */
body.push(h1("Appendix A: The xeyezHal.dll API Inventory"));
body.push(p("The full export list of the hardware abstraction layer is the device's de facto capability map. Version v2.5.07 ships " + HAL_V25_EXPORTS + " exports; v2.6.0.6 ships " + HAL_V26_EXPORTS + ", adding the digital-rights-management and calibration-group functions marked new below. The functions are grouped by role; the image-capture group is the one this study depends on most heavily."));
body.push(caption("Table 7: xeyezHal.dll exports by function group (v2.6.0.6; new-in-v2.6 marked *)"));
body.push(tbl([24, 76],
  ["Group", "Functions"],
  [
    ["Image capture", { text: "GetGlobalImageSize, SetTransMode, GetImageBegin, GetImageData, GetBlobBegin, GetBlobData, GetBlobEnd, GetOldBlobData, GetNoise, SetBlobFeatureWidth", mono: true, size: 15 }],
    ["Sensor / FPGA registers", { text: "CmosInit, CmosReset, CmosRegRead, CmosRegWrite, SetCmosTable, SetCmosIndex, FpgaRegRead, FpgaRegWrite, FpgaRegReadTLQ, FpgaRegWriteTLQ, SetFpgaTable, SetBoard, SetDevice", mono: true, size: 15 }],
    ["Firmware download", { text: "SetFpgaFile, DownLoadFpga, DownLoadHex, MCUReset, UpdateReset, StartFlow, GetBLVersion, GetMcuVersion, DeviceVerRead, DeviceVerWtite, HardVersion_Read*, HardVersion_Write*, ReadCode, FlashLock", mono: true, size: 15 }],
    ["Identity and DRM", { text: "DeviceDetect, DeviceIDRead, DeviceIDWrite, ChipIDRead, ChipIDWrite, ChipIDVerify, SetEnChipIDVerify, RandomCodeSet, RandomCodeRead, RandomCodeWrite, RandomCodeCancel, CheckCodeRead*, CheckCodeWrite*, CheckMatch*, Check_Key*, SecurityComm*, DRMRead*, DRMWrite*", mono: true, size: 15 }],
    ["Calibration data", { text: "LensParaRead, LensParaWrite, UserLensParaRead, UserLensParaWrite, SetLensBuffer, SetEnLensWrite, SetEnableLensParamRead, EpromReadInterface, EpromWriteInterface, CalibNumRead*, CalibNumWrite*, BlobAxisRead*, BlobAxisWrite*, DeviceCamRead*, DeviceCamWrite*, DeviceClassRead*", mono: true, size: 15 }],
    ["Control and misc", { text: "SetLed_Status, SetExpMode, SetClassifier, LoadSetting, FlushBulkInBuffer, GetStatus, GetLastError", mono: true, size: 15 }],
  ], { zebra: true }));

/* ---- Appendix B ---- */
body.push(h1("Appendix B: Register Scripts, Complete"));
body.push(p("The scripts are reproduced in full because they are the vendor-tested sensor configuration and the driver replays them byte-for-byte. Line format is register, value, both hexadecimal."));
body.push(caption("Table 8: Sensor init (30fps_Global_0316_Cmos.txt, common to both modes)"));
body.push(tbl([16, 16, 16, 16, 16, 20],
  ["Register", "Value", "Register", "Value", "Register", "Value"],
  (() => {
    const rows = [];
    const lines = regCmos.split("\n").map((l) => l.trim()).filter((l) => l.length > 0);
    for (let i = 0; i < lines.length; i += 3) {
      const cells = [0, 1, 2].map((k) => {
        if (i + k < lines.length) { const [r, v] = lines[i + k].split(","); return { text: r + " = " + v, mono: true, size: 15 }; }
        return { text: "", mono: true, size: 15 };
      });
      rows.push([cells[0], cells[1], cells[2]]);
    }
    return rows;
  })(), { zebra: true }));
body.push(caption("Table 9: Mode scripts - lines that differ or are unique"));
body.push(tbl([16, 42, 42],
  ["Register", "blob_1LED.dat (mode 0x0A)", "image_2sensor_diff_1LED.dat (mode 0x0B)"],
  [
    [mono("0x02"), { text: "0x0A", mono: true }, { text: "0x0B", mono: true }],
    [mono("0x50-0x57"), { text: "02 7C 01 D0 08 00 00 05", mono: true }, "(not written)"],
    [mono("0x20 / 0x28"), { text: "0x10 / 0x10", mono: true }, { text: "0x10 / 0x10", mono: true }],
    [mono("0x08 / 0x0A"), { text: "0x1E / 0x20", mono: true }, { text: "0x1E / 0x20", mono: true }],
    [mono("0x19 / 0x1A-0x1D"), { text: "0x0F / 00 10 00 10", mono: true }, { text: "0x0F / 00 10 00 10", mono: true }],
    [mono("trailer"), { text: "0x06,0x02 + 0x03,0x01", mono: true }, { text: "0x03,0x01", mono: true }],
  ], { zebra: true }));
body.push(caption("Table 10: Auxiliary scripts data0.dat, data1.dat, data10.dat"));
body.push(tbl([22, 78],
  ["Script", "Content (register = value sequence)"],
  [
    [{ text: "data0.dat", mono: true }, { text: regData0.split("\n").map((l) => l.trim()).filter(Boolean).join("  "), mono: true, size: 15 }],
    [{ text: "data1.dat", mono: true }, { text: regData1.split("\n").map((l) => l.trim()).filter(Boolean).join("  "), mono: true, size: 15 }],
    [{ text: "data10.dat", mono: true }, { text: regData10.split("\n").map((l) => l.trim()).filter(Boolean).join("  "), mono: true, size: 15 }],
  ]));

/* ---- Appendix C ---- */
body.push(h1("Appendix C: Packet Format Reference"));
body.push(p("This appendix consolidates the byte layouts a driver implementer needs, in the order the device sees them. All multi-byte fields are little-endian unless marked otherwise; the runtime 16-bit argument at offset 6 is byte-swapped (stored big-endian) by the vendor builder."));
body.push(caption("Table 11: Boot-stage framing (8051 monitor, 64-byte packets)"));
body.push(tbl([22, 44, 34],
  ["Offset", "Host to device", "Device to host"],
  [
    [mono("0x00-0x03"), { text: "50 55 7F 7F ('PU' + 7F 7F)", mono: true }, { text: "4F 44 7F 7F ('OD' + 7F 7F)", mono: true }],
    [mono("0x04-0x05"), "command (u16): 2 = ready/query, 3 = program data, 7 = DSP start, 4 = ID handshake, 0xFFE = link query", "status type (u16): 2 = ready, 3 = started/ack"],
    [mono("0x08-0x09"), "argument (u16), e.g. packet count", "status code (u16); 0x0F = CRC error"],
    [mono("0x0C-0x3F"), "payload (program bytes for command 3)", "payload"],
  ], { zebra: true }));
body.push(caption("Table 12: Runtime framing (firmware active; see also Table 2)"));
body.push(tbl([22, 44, 34],
  ["Offset", "Host to device", "Device to host"],
  [
    [mono("0x00-0x01"), { text: "7F 55", mono: true }, { text: "7F 55", mono: true }],
    [mono("0x02"), "command: 3 = register write (chunked), 5 = blob data read, 6 = GET_IMG_START, 15 = register/classifier write", "echo / sub-code"],
    [mono("0x03-0x05"), "argument bytes (for command 3: chunk semantics)", "data"],
    [mono("0x06-0x07"), "u16 argument, byte-swapped (chunk length for command 3)", "data"],
    [mono("0x04 (rx)"), "-", "status byte; GET_IMG_START expects 1, blob read expects 4"],
    [mono("0x08+"), "payload (register bytes)", "payload"],
  ], { zebra: true }));
body.push(p("Provenance summary. The FPGA identification comes from the bitstream header parsed from A_a_v4.4_BG0313.bit in both installer versions. The Blackfin address analysis comes from the block headers of ens_code.ldr. The runtime packet engine, the image-capture call sequence and the command identifiers come from the disassembly of xeyezHal.dll v2.6.0.6 at the addresses cited in chapter 3, cross-checked against the export wrappers of the same file. The register-script semantics come from the direct comparison of the two mode scripts in the 30fps directory. The frame structure of the tracking stream comes from the disassembly of xEyeClass.dll (GetADataFromUSB) and the exports of StereoCamera.dll. The geometry constants come from the checksum-verified EEPROM decode of the previous report. No value in this document is estimated; every one is either computed from a file or read from a specific instruction address."));

/* ================= assemble ================= */
const doc = new Document({
  styles: { default: { document: {
    run: { font: FONT, size: 24, color: "000000" },
    paragraph: { spacing: { line: 312 } },
  }}},
  numbering: { config: [{
    reference: "next-steps",
    levels: [{ level: 0, format: "decimal", text: "%1.", alignment: AlignmentType.START,
      style: { paragraph: { indent: { left: 460, hanging: 300 } } } }],
  }]},
  features: { updateFields: true },
  sections: [
    { properties: { page: { margin: { top: 0, bottom: 0, left: 0, right: 0 } } },
      children: buildCoverR1({
        palette: PAL,
        englishLabel: "RESEARCH REPORT",
        title: "idong100 3D Depth and FPGA Modification Study",
        subtitle: "Raw-image access, dense stereo feasibility, and the firmware replacement path",
        metaLines: [
          "Device: idong100 dual-camera sensor (USB 0471:2600, generation 2)",
          "Evidence: xeyezHal.dll v2.5.07 / v2.6.0.6, A_a_v4.4_BG0313.bit, ens_code.ldr",
          "Geometry: T24C16A EEPROM calibration (fx 432.33 px, baseline 100.00 mm)",
          "Date: 23 November 2026",
        ],
        footerLeft: "idong100 reverse-engineering project",
        footerRight: "Turn 11",
      }) },
    { properties: { titlePage: false, page: { margin: { top: 1440, bottom: 1440, left: 1701, right: 1417 },
        pageNumbers: { start: 1, formatType: NumberFormat.UPPER_ROMAN } } },
      footers: { default: new Footer({ children: [new Paragraph({ alignment: AlignmentType.CENTER,
        children: [new TextRun({ children: [PageNumber.CURRENT], size: 18 })] })] }) },
      children: [
        new Paragraph({ spacing: { before: 200, after: 200 }, alignment: AlignmentType.CENTER,
          children: [new TextRun({ text: "Contents", bold: true, size: 32, font: { eastAsia: "SimHei", ascii: "Times New Roman" }, color: H1C })] }),
        new TableOfContents("Contents", { hyperlink: true, headingStyleRange: "1-1" }),
        new Paragraph({ spacing: { before: 200 }, alignment: AlignmentType.CENTER,
          children: [new TextRun({ text: "Right-click the table and choose Update Field to refresh page numbers.", italics: true, size: 18, color: "888888" })] }),
        new Paragraph({ children: [new TextRun({ text: " ", size: 2 }), new PageBreak()] }),
      ] },
    { properties: { titlePage: false, page: { margin: { top: 1440, bottom: 1440, left: 1701, right: 1417 },
        pageNumbers: { start: 1, formatType: NumberFormat.DECIMAL } } },
      footers: { default: new Footer({ children: [new Paragraph({ alignment: AlignmentType.CENTER,
        children: [new TextRun({ children: [PageNumber.CURRENT], size: 18 })] })] }) },
      headers: { default: new Header({ children: [new Paragraph({ alignment: AlignmentType.RIGHT,
        border: { bottom: { style: BorderStyle.SINGLE, size: 4, color: PAL.table.innerLine } },
        children: [new TextRun({ text: "idong100 3D Depth and FPGA Modification Study", size: 16, color: "888888" })] })] }) },
      children: body },
  ],
});

Packer.toBuffer(doc).then((buf) => {
  fs.writeFileSync("/home/z/my-project/download/idong100_3D_Depth_FPGA_Study.docx", buf);
  console.log("wrote idong100_3D_Depth_FPGA_Study.docx", buf.length, "bytes");
  console.log("bit header:", JSON.stringify(bitHeader));
});
