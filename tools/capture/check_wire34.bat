@if (@X)==(@Y) @end /* JScript comment
@echo off
setlocal enabledelayedexpansion
title idong100 - wire phase check, turn 34
REM ============================================================
REM  check_wire34.bat - how far did the engine get this time?
REM  Turn 34. cmd.exe + JScript run by cscript. No PowerShell.
REM
REM  Run this AFTER a capture made with the patched engine
REM  (turn-33/34 unlock). It decodes the PHASES of the session,
REM  not just the bytes:
REM
REM    boot flash   0x0101/335 blocks/0x0201/0x0301/0x0401
REM    session      0x0008 -> "2619", polls 03/04/05/0d/0f04
REM    EEPROM reads 0x0107 with (offset,len) - calib, "000T",
REM                "2613", the DEVICE ID at 0x0300 (33 B)
REM    DSP upload   ens_code.ldr bytes out on ep 02
REM    FPGA upload  A_a_v4.4_BG0313.bit bytes out on ep 02
REM    UVC          probe/commit SET_CUR, SET_INTERFACE alt > 0
REM    video        bulk/isochronous IN traffic on ep 0x85
REM
REM  Double-click : scans the newest folder C:\idong_capture\runN
REM  Command line : check_wire34.bat D:\caps\boot.pcap
REM                 check_wire34.bat D:\caps     all .pcap inside
REM  Exit codes: 0 = video phase reached, 1 = not yet, 3 = error.
REM  The window stays open at every exit (user rule, turn 19).
REM ============================================================

set "RC=3"
set "TARGET=%~1"
if not defined TARGET goto :find_dir
goto :have_target

:find_dir
if exist "C:\idong_capture\" (
  set "TARGET=C:\idong_capture"
  goto :have_target
)
echo.
echo [ERR] No capture folder found. Run run_capture.bat first,
echo       or drag a .pcap file onto this script.
goto :done

:have_target
echo === idong100 - wire phase check, turn 34 ===
echo.
if not exist "!TARGET!" (
  echo [ERR] not found: "!TARGET!"
  goto :done
)

set "DIR="
if exist "!TARGET!\" set "DIR=!TARGET!"
if defined DIR goto :folder
set "FILES="!TARGET!""
set "REPORT=!TARGET!"
for %%f in ("!TARGET!") do set "REPORT=%%~dpf"
if "!REPORT:~-1!"=="\" set "REPORT=!REPORT:~0,-1!"
goto :run

:folder
set /a N=0
for %%f in ("!DIR!\*.pcap") do set /a N+=1
if !N! GTR 0 goto :collect
set "BEST="
for /l %%i in (1,1,999) do if exist "!DIR!\run%%i\" set "BEST=!DIR!\run%%i"
if not defined BEST goto :no_files
set "DIR=!BEST!"
set /a N=0
for %%f in ("!DIR!\*.pcap") do set /a N+=1
if !N! EQU 0 goto :no_files

:collect
set "FILES="
for %%f in ("!DIR!\*.pcap") do set "FILES=!FILES! "%%~ff""
set "REPORT=!DIR!"

:run
echo Scanning:
for %%f in (!FILES!) do echo   %%~f
echo.
cscript //nologo //E:JScript "%~f0" --report "!REPORT!\wire_report34.txt" !FILES!
set "RC=!errorlevel!"
set "REPORT=!REPORT!\wire_report34.txt"
if not exist "!REPORT!" goto :done
echo.
echo Report saved: "!REPORT!"
echo.
type "!REPORT!"
:done
echo.
echo Press any key to close this window...
pause >nul
exit /b !RC!

:no_files
echo [ERR] no .pcap files in "!DIR!"
goto :done
exit /b 3 */

// ==================== JScript part ====================
var RT   = String.fromCharCode(0x7f, 0x55);     // MCU download + runtime framing
var PU   = String.fromCharCode(0x50, 0x55, 0x7f, 0x7f);   // DSP boot framing (host -> sensor)
var OD   = String.fromCharCode(0x4f, 0x44, 0x7f, 0x7f);   // DSP boot framing (sensor -> host)
var LDR  = String.fromCharCode(0x40,0x00,0x80,0xff,0x04,0x00,0x00,0x00,0x12,0x00,0x86,0xd4);
var BIT  = String.fromCharCode(0x00,0x09,0x0f,0xf0,0x0f,0xf0,0x0f,0xf0,0x0f,0xf0,0x00,0x00,0x01,0x61,0x00,0x30);
var PCAP_MAGIC = String.fromCharCode(0xd4, 0xc3, 0xb2, 0xa1);
var CHUNK = 1 << 20;
var SERIAL = "AB460DC8BA12C6840B49CD5786C329DD";   // this unit's device ID

function u16(s, o) { return s.charCodeAt(o) | (s.charCodeAt(o + 1) << 8); }
function u32(s, o) {
    return (s.charCodeAt(o) | (s.charCodeAt(o + 1) << 8) |
            (s.charCodeAt(o + 2) << 16) | (s.charCodeAt(o + 3) << 24)) >>> 0;
}
function fmtSize(n) {
    if (n >= 1048576) { return (n / 1048576).toFixed(1) + " MB"; }
    if (n >= 1024) { return (n / 1024).toFixed(1) + " kB"; }
    return n + " B";
}
function fmtTime(sec) {
    var m = Math.floor(sec / 60), s = sec % 60;
    return m + ":" + (s < 10 ? "0" : "") + s;
}

function newResult(path) {
    return { path: path, size: 0, packets: 0, data: 0, err: "",
        firstTs: 0, lastTs: 0,
        flashOut: 0, flashIn: 0,
        ep02Out: 0, ep82In: 0, pu: 0, od: 0,
        v2600: 0, v2619: 0,
        eeReads: [], devIdRead: 0, devIdReply: 0,
        ldrOut: 0, ldrBytes: 0, bitOut: 0, bitBytes: 0,
        uvcProbe: 0, uvcCommit: 0, setIface: [],
        inByEp: {}, outByEp: {},
        cam2600: -1, cam2500: -1 };
}

function addBytes(map, key, n) {
    var k = String(key);
    if (!map[k]) { map[k] = 0; }
    map[k] += n;
}

function processPacket(r, s, off, incl) {
    var ts = u32(s, off - 16), hlen, dev, ep, tr, dlen, d, avail, head;
    r.packets++;
    if (r.firstTs === 0) { r.firstTs = ts; }
    r.lastTs = ts;
    if (incl < 27) { return; }
    hlen = u16(s, off);
    if (hlen < 27 || hlen > incl) { return; }
    dev = u16(s, off + 19);
    ep = s.charCodeAt(off + 21);
    tr = s.charCodeAt(off + 22);
    dlen = u32(s, off + 23);
    d = off + hlen;
    avail = incl - hlen;
    if (avail > dlen) { avail = dlen; }
    if (avail <= 0) { return; }

    if (tr === 2) {                                       // control transfers
        var b0 = s.charCodeAt(d), b1 = s.charCodeAt(d + 1);
        if (b0 === 0x12 && b1 === 0x01 && avail >= 18 && u16(s, d + 8) === 0x0471) {
            if (u16(s, d + 10) === 0x2600 && r.cam2600 < 0) { r.cam2600 = dev; }
            if (u16(s, d + 10) === 0x2500 && r.cam2500 < 0) { r.cam2500 = dev; }
        }
        if (b0 === 0x01 && b1 === 0x0b) {                  // SET_INTERFACE
            var alt = u16(s, d + 2), iface = u16(s, d + 4);
            if (alt > 0) { r.setIface.push("if" + iface + " alt" + alt); }
        }
        if (b0 === 0x21 && b1 === 0x01) {                  // UVC SET_CUR
            var cs = (u16(s, d + 2) >> 8) & 0xff;
            if (cs === 0x01) { r.uvcProbe++; }
            if (cs === 0x02) { r.uvcCommit++; }
        }
        return;
    }
    if (tr !== 3 && tr !== 1) { return; }
    if (avail < 2) { return; }
    r.data++;
    head = s.substr(d, 4);
    if ((ep & 0x80) === 0) {                              // OUT
        addBytes(r.outByEp, ep & 0x7f, avail);
        if ((ep & 0x7f) === 0x02) {
            r.ep02Out += avail;
            if (head === RT) {
                // 7f 55 <cmd LE16> <flags> 00  00 <alen> <args BE>
                var cmd = u16(s, d + 2), alen = s.charCodeAt(d + 7);
                if (cmd === 0x0107 && alen >= 2 && avail >= 10) {
                    var eo = (s.charCodeAt(d + 8) << 8) | s.charCodeAt(d + 9);
                    var el = (s.charCodeAt(d + 10) << 8) | s.charCodeAt(d + 11);
                    r.eeReads.push("0x" + eo.toString(16) + " len " + el);
                    if (eo === 0x0300 && el === 0x21) { r.devIdRead++; }
                }
                if (cmd === 0x0101) { r.flashOut++; }
            }
            if (s.substr(d, 12) === LDR.substr(0, 12)) { r.ldrOut++; }
            if (s.substr(d, 16) === BIT) { r.bitOut++; }
            if (head === PU) { r.pu++; }
        }
    } else {                                              // IN
        addBytes(r.inByEp, ep & 0x7f, avail);
        if ((ep & 0x7f) === 0x02) {
            r.ep82In += avail;
            if (head === PU) { r.od++; }
            if (head === RT) {
                if (avail >= 12 && s.charCodeAt(d + 2) === 0x08 &&
                    s.charCodeAt(d + 3) === 0x00) {
                    var ver = s.substr(d + 8, 4);
                    if (ver === "2600") { r.v2600++; }
                    else if (ver === "2619") { r.v2619++; }
                }
            }
            if (avail >= 33 && s.substr(d, 4) === SERIAL.substr(0, 4)) {
                if (s.substr(d, 32) === SERIAL) { r.devIdReply++; }
            }
        }
    }
}

function scanFile(path) {
    var r = newResult(path), b, t, h, s, chunk, carry = "", pos, incl, base = 24, left;
    try {
        b = new ActiveXObject("ADODB.Stream");
        b.Type = 1; b.Open(); b.LoadFromFile(path);
        r.size = b.Size;
        if (r.size < 24) {
            r.err = "file is too small"; b.Close(); return r;
        }
        t = new ActiveXObject("ADODB.Stream");
        t.Type = 2; t.Charset = "iso-8859-1"; t.Open();
        b.CopyTo(t); b.Close(); t.Position = 0;
        h = t.ReadText(24);
        if (h.substr(0, 4) !== PCAP_MAGIC) {
            r.err = "not a USBPcap .pcap file"; t.Close(); return r;
        }
        for (;;) {
            left = r.size - t.Position;
            if (left <= 0) { break; }
            chunk = t.ReadText(left < CHUNK ? left : CHUNK);
            if (chunk.length === 0) { break; }
            s = carry + chunk;
            pos = 0;
            while (pos + 16 <= s.length) {
                incl = u32(s, pos + 8);
                if (incl < 1 || incl > 4 * CHUNK) {
                    r.err = "corrupt record at offset " + (base + pos);
                    break;
                }
                if (pos + 16 + incl > s.length) { break; }
                processPacket(r, s, pos + 16, incl);
                pos += 16 + incl;
            }
            if (r.err !== "") { break; }
            base += pos;
            carry = s.substr(pos);
        }
        t.Close();
    } catch (e2) {
        r.err = "read error: " + (e2.message || e2);
        return r;
    }
    return r;
}

function phase(r) {
    // 0 = nothing, 1 = flash, 2 = session, 3 = device-ID read,
    // 4 = UVC negotiation, 5 = video streaming
    if (r.ep82In > 1000 && (r.setIface.length > 0 || r.uvcCommit > 0)) { return 5; }
    if (r.inByEp[0x85] > 10000) { return 5; }
    if (r.setIface.length > 0 || r.uvcCommit > 0 || r.uvcProbe > 0) { return 4; }
    if (r.devIdRead > 0 || r.devIdReply > 0) { return 3; }
    if (r.v2619 > 0 || r.eeReads.length > 0) { return 2; }
    if (r.ep02Out > 10000) { return 1; }
    if (r.packets > 0) { return 0; }
    return -1;
}
var PHASES = ["enum only", "boot flash", "session/polls", "device-ID read",
              "UVC negotiation", "VIDEO STREAMING"];

function report(r, out) {
    out.push("  file      : " + r.path.replace(/^.*\\/, ""));
    out.push("  size/time: " + fmtSize(r.size) + ", " +
             (r.packets ? (r.lastTs - r.firstTs) + " s" : "0 s") +
             ", " + r.packets + " packets");
    if (r.err !== "") { out.push("  ERROR    : " + r.err); return; }
    out.push("  sensor   : " + (r.cam2600 >= 0 ? "2600 seen " : "") +
             (r.cam2500 >= 0 ? "2500 seen" : "") +
             (r.cam2600 < 0 && r.cam2500 < 0 ? "not seen" : ""));
    out.push("  ep02 out : " + fmtSize(r.ep02Out) + "   ep82 in : " + fmtSize(r.ep82In));
    out.push("  versions : 2600 x" + r.v2600 + ", 2619 x" + r.v2619 +
             ", DSP PU x" + r.pu + ", OD x" + r.od);
    var ee = r.eeReads.slice(0, 6);
    out.push("  EEPROM   : " + (ee.length ? ee.join("; ") : "none"));
    if (r.eeReads.length > 6) { out.push("             ... " + r.eeReads.length + " reads total"); }
    out.push("  device-ID: read x" + r.devIdRead + ", serial reply x" + r.devIdReply);
    out.push("  DSP/FPGA : ldr-start x" + r.ldrOut + ", bit-start x" + r.bitOut +
             " (0 = not uploaded in this capture)");
    out.push("  UVC      : probe x" + r.uvcProbe + ", commit x" + r.uvcCommit +
             (r.setIface.length ? ", " + r.setIface.join("; ") : ", no alt set"));
    var eps = [], k, m = r.inByEp;
    for (k in m) { if (m[k] > 1000) { eps.push("ep 0x" + k + " in " + fmtSize(m[k])); } }
    out.push("  IN eps   : " + (eps.length ? eps.join(", ") : "no bulk data"));
    var ph = phase(r); out.push("  PHASE    : " + PHASES[ph < 0 ? 0 : ph]);
}

function main() {
    var args = WScript.Arguments, i, out = [], best = -1, rc = 1;
    var results = [], p = -1;
    var reportPath = "";
    for (i = 0; i < args.length; i++) {
        if (args(i) === "--report") { reportPath = args(i + 1); i++; continue; }
        var r = scanFile(args(i));
        results.push(r);
    }
    out.push("=== idong100 wire phase report, turn 34 ===");
    for (i = 0; i < results.length; i++) {
        out.push("");
        report(results[i], out);
        if (results[i].err === "" && phase(results[i]) > best) {
            best = phase(results[i]);
        }
    }
    out.push("");
    out.push("Verdict: the engine reached \"" + PHASES[best < 0 ? 0 : best] + "\".");
    // (PHASES[0] = "enum only" is the floor for a null capture)
    if (best >= 5) {
        out.push("VIDEO IS STREAMING - capture a longer session and send");
        out.push("the whole run folder. This is the capture we need!");
        rc = 0;
    } else if (best >= 3) {
        out.push("The device-ID stage was reached. If the app still shows");
        out.push("\"connecting\", the engine host is gating on the license");
        out.push("check (turn 34: apply patch_engine34.bat, it also patches");
        out.push("xEyeRight.dll).");
        rc = 2;
    } else if (best >= 1) {
        out.push("The connect still stops early - see the EEPROM reads above.");
        rc = 1;
    } else {
        out.push("No sensor session in this capture.");
        rc = 1;
    }
    if (reportPath !== "") {
        try {
            var fso = new ActiveXObject("Scripting.FileSystemObject"), f;
            f = fso.CreateTextFile(reportPath, true);
            f.Write(out.join("\r\n") + "\r\n");
            f.Close();
        } catch (e5) {}
    }
    WScript.Echo(out.join("\n"));
    WScript.Quit(rc);
}
main();
