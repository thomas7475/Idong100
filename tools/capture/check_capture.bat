@if (@X)==(@Y) @end /* JScript comment
@echo off
setlocal
set "SELF=%~f0"
set "ARG1=%~1"
set "ARG2=%~2"
setlocal enabledelayedexpansion
title idong100 - check capture
REM ============================================================
REM  check_capture.bat - is the idong sensor session in a USBPcap file?
REM  Turn 32 rewrite. cmd.exe + JScript run by cscript. No PowerShell.
REM
REM  Turn 32 fixes: the ADODB binary reader (VBArray) failed on the
REM  capture machine ("could not read the first 24 bytes" in every
REM  report) - reading now goes through a text stream with charset
REM  iso-8859-1 (1 byte = 1 char), no VBArray at all. The gate now
REM  knows the observed protocol: 0x0008 version replies "2600"/"2619".
REM
REM  Double-click  : scans the newest folder C:\idong_capture\runN
REM  Drag and drop : drop a .pcap file on this script
REM  Command line  : check_capture.bat D:\caps\boot.pcap
REM                   check_capture.bat D:\caps      all .pcap inside
REM
REM  It parses the real USBPcap packets, it does not just count bytes:
REM    7f 55             MCU download and runtime framing (observed)
REM    "2600"/"2619"     version replies of the 0x0008 command
REM    PU/OD 50 55 7f 7f / 4f 44 7f 7f   legacy DLL model, 0 on the wire
REM    "Ready to load program!"          legacy banner, never observed
REM  Gate: PASS = a 2600->2500 cold boot with >= 10000 B on ep 02, OR
REM  a 2500 session with >= 10000 B on ep 02 AND on ep 82 (download +
REM  echo - the shape of the turn 32 captures).
REM  Exit codes: 0 PASS, 2 PARTIAL, 1 FAIL, 3 scanner error.
REM  The window stays open at every exit (user rule, turn 19).
REM ============================================================

set "RC=3"
set "MODE="
set "PREDIRRC=0"
set "PRENOTE="
set "TARGET=!ARG1!"
if /i not "!ARG1!"=="--info" goto :have_mode
set "MODE=--info"
set "TARGET=!ARG2!"
:have_mode
if defined TARGET goto :have_target
rem --- no argument: use the folder predir.bat found (turn 26). It tries
rem --- C:\idong_capture, its repair, then the USERPROFILE, LOCALAPPDATA
rem --- and TEMP fallback folders, and sets CAPROOT and PREDIRRC.
call "%~dp0predir.bat" /called
if "!PREDIRRC!"=="2" goto :no_folder
set "TARGET=!CAPROOT!"
:have_target
if defined MODE goto :after_target
if "!PREDIRRC!"=="1" goto :fallback_note_a
echo.
echo Reading from C:\idong_capture
goto :target_echoed
:fallback_note_a
echo.
echo NOTE: C:\idong_capture is not writable in this window, so the captures
echo       are read from the fallback folder "!CAPROOT!" instead.
echo       !PRENOTE!
echo       Run predir.bat by hand (double-click) for its self-test of the
echo       four candidate folders and the exact errors.
:target_echoed

echo === idong100 - check capture ===
echo.
if not exist "!TARGET!" (
  echo [ERR] not found: "!TARGET!"
  echo       Nothing captured yet? Run run_capture.bat first.
  goto :done
)
goto :after_target
:after_target

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
rem --- no .pcap directly here: take the newest runN subfolder
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
set "M=--info"
set "R="
if defined MODE goto :run_go
set "M=--report"
set "R="!REPORT!\check_report.txt""
:run_go
echo Scanning:
for %%f in (!FILES!) do echo   %%~f
echo.
cscript //nologo //E:JScript "!SELF!" !M! !R! !FILES!
set "RC=!errorlevel!"
if defined MODE goto :done
set "REPORT=!REPORT!\check_report.txt"
if not exist "!REPORT!" goto :done
echo.
echo Report saved: "!REPORT!"
goto :done

:no_files
echo [ERR] no .pcap file found in "!DIR!"
echo       Run run_capture.bat first, or drop one .pcap file on this script.
set "RC=1"
goto :done

:no_folder
echo [ERR] no writable capture folder: none of the four candidates
echo       SystemDrive, USERPROFILE, LOCALAPPDATA, TEMP accepted a file.
echo       Run predir.bat by hand (double-click it in the kit folder):
echo       it prints the exact errors and the rights of C:\idong_capture,
echo       and it stays open. Its text can be copied and sent as it is.
set "RC=3"

:done
echo.
if "!RC!"=="0" echo Result: PASS - the sensor boot is in the capture.
if "!RC!"=="2" echo Result: PARTIAL - some sensor frames, see the lines above.
if "!RC!"=="1" echo Result: FAIL - see the lines above.
if "!RC!"=="3" echo Result: SCANNER ERROR - see the lines above.
echo.
echo Exit code !RC!   0 = PASS   2 = PARTIAL   1 = FAIL   3 = scanner error
echo.
echo To copy this text out of the window (to send it without a photo):
echo   right-click the TITLE BAR - Bearbeiten - Markieren - press Enter,
echo   then paste with Strg+V into any text field. English Windows:
echo   title bar - Edit - Select All - Enter.
echo.
pause
endlocal & exit /b %RC%
*/

// ================= JScript part, run by cscript //E:JScript =================
// ES3 only (JScript 5.8 ships with Windows 7). ASCII output only.
var PU = String.fromCharCode(0x50, 0x55, 0x7f, 0x7f);   // host -> sensor, DSP boot framing
var OD = String.fromCharCode(0x4f, 0x44, 0x7f, 0x7f);   // sensor -> host, DSP boot framing
var RT = String.fromCharCode(0x7f, 0x55);               // MCU download + runtime framing
var READY = "Ready to load program!";
var PCAP_MAGIC = String.fromCharCode(0xd4, 0xc3, 0xb2, 0xa1);
var CHUNK = 1 << 20;
var GATE_PU = 10;
var GATE_OD = 3;

function bin(bytes) {                 // VBArray of bytes -> binary string
    var a, out = [], i, n;
    try { a = new VBArray(bytes).toArray(); } catch (e) { return ""; }
    for (i = 0; i < a.length; i += 4096) {
        n = Math.min(i + 4096, a.length);
        out.push(String.fromCharCode.apply(null, a.slice(i, n)));
    }
    return out.join("");
}
function u16(s, o) { return s.charCodeAt(o) | (s.charCodeAt(o + 1) << 8); }
function u32(s, o) {
    return (s.charCodeAt(o) | (s.charCodeAt(o + 1) << 8) | (s.charCodeAt(o + 2) << 16))
         + s.charCodeAt(o + 3) * 16777216;
}
function count(map, key) {
    var k = String(key);
    if (map[k] === undefined) { map[k] = 1; } else { map[k]++; }
}
function addBytes(map, key, n) {
    var k = String(key);
    if (map[k] === undefined) { map[k] = n; } else { map[k] += n; }
}
function newResult(path) {
    return { path: path, err: "", warn: "", size: 0, packets: 0, data: 0,
             firstTs: 0, lastTs: 0, truncated: 0, cam2600: -1, cam2500: -1,
             pu: 0, od: 0, rtOut: 0, rtIn: 0, ready: 0, ep02Bytes: 0, ep82Bytes: 0,
             v2600: 0, v2619: 0,
             outFrames: {}, inFrames: {}, inBytes: {} };
}

function hasReady(s, d, avail) {
    return s.substr(d, avail).indexOf(READY) >= 0;    // search inside this payload only
}

function processPacket(r, s, off, incl) {             // off = position of the USBPcap header inside s
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
    if (tr === 2) {                                   // control transfer
        if (avail >= 18 && s.charCodeAt(off + 27) === 3 &&
            s.charCodeAt(d) === 0x12 && s.charCodeAt(d + 1) === 0x01 &&
            u16(s, d + 8) === 0x0471) {
            if (u16(s, d + 10) === 0x2600 && r.cam2600 < 0) { r.cam2600 = dev; }
            if (u16(s, d + 10) === 0x2500 && r.cam2500 < 0) { r.cam2500 = dev; }
        }
        return;
    }
    if (tr !== 3 && tr !== 1) { return; }             // only bulk and interrupt carry sensor frames
    if (avail < 2) { return; }
    r.data++;
    head = s.substr(d, 4);
    if ((ep & 0x80) === 0) {                          // OUT: the payload travels in the submit packet
        count(r.outFrames, dev);
        if ((ep & 0x7f) === 0x02) { r.ep02Bytes += avail; }
        if (head === PU) { r.pu++; }
        else if (head.substr(0, 2) === RT && (ep & 0x7f) === 0x02) { r.rtOut++; }
    } else {                                          // IN: the payload travels in the complete packet
        count(r.inFrames, dev);
        addBytes(r.inBytes, dev, avail);
        if ((ep & 0x7f) === 0x02) { r.ep82Bytes += avail; }
        if (head === OD) { r.od++; if (hasReady(s, d, avail)) { r.ready++; } }
        else if (head.substr(0, 2) === RT && (ep & 0x7f) === 0x02) {
            r.rtIn++;
            // 0x0008 version reply: 7f 55 08 00 01 00 00 00 "2600"/"2619"
            if (avail >= 12 && s.charCodeAt(d + 2) === 0x08 &&
                s.charCodeAt(d + 3) === 0x00 && s.charCodeAt(d + 4) === 0x01) {
                var ver = s.substr(d + 8, 4);
                if (ver === "2600") { r.v2600++; }
                else if (ver === "2619") { r.v2619++; }
            }
        }
    }
}

function scanFile(path) {
    var r = newResult(path), b, t, h, s, chunk, carry = "", pos, incl, base = 24, left;
    // Turn 32: the file is read through a TEXT stream with charset
    // iso-8859-1 (one byte = one char). The old VBArray conversion of
    // binary Read() results failed on the capture machine (12 of 12
    // files). No VBArray is used any more.
    try {
        b = new ActiveXObject("ADODB.Stream");
        b.Type = 1;                       // adTypeBinary
        b.Open();
        b.LoadFromFile(path);
        r.size = b.Size;
        if (r.size < 24) {
            r.err = "file is too small (" + r.size + " bytes) - USBPcap did not write a header";
            b.Close();
            return r;
        }
        t = new ActiveXObject("ADODB.Stream");
        t.Type = 2;                       // adTypeText
        t.Charset = "iso-8859-1";         // byte x -> char U+00xx, 1:1
        t.Open();
        b.CopyTo(t);
        b.Close();
        t.Position = 0;
        h = t.ReadText(24);
        if (h.length < 24) {
            r.err = "could not read the first 24 bytes (text stream)";
            t.Close();
            return r;
        }
        if (h.substr(0, 4) !== PCAP_MAGIC) {
            r.err = "not a USBPcap .pcap file (wrong magic number)";
            t.Close();
            return r;
        }
        if (u32(h, 20) !== 249) { r.warn = "link type " + u32(h, 20) + ", expected 249 (USBPcap)"; }
        for (;;) {                       // read in chunks, never past the end of the stream
            left = r.size - t.Position;  // char position == byte position
            if (left <= 0) { break; }
            chunk = t.ReadText(left < CHUNK ? left : CHUNK);
            if (chunk.length === 0) { break; }
            s = carry + chunk;
            pos = 0;
            while (pos + 16 <= s.length) {
                incl = u32(s, pos + 8);
                if (incl < 1 || incl > 4 * CHUNK) {
                    r.err = "corrupt record (length " + incl + ") at offset " + (base + pos);
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
        try { t.Close(); } catch (e3) {}
        try { b.Close(); } catch (e4) {}
        return r;
    }
    if (carry.length > 0) { r.truncated = 1; }
    return r;
}

function verdict(r) {
    if (r.err !== "") { return "ERROR"; }
    if (r.packets === 0) { return "EMPTY"; }
    if (r.pu + r.od + r.rtOut + r.rtIn === 0) {
        if (r.cam2600 >= 0 || r.cam2500 >= 0) { return "ENUM-ONLY"; }
        return "NO-CAMERA";
    }
    // Turn 32: no PU/OD frames exist on the wire (0 in three sessions,
    // ~2900 records). A capture passes when either
    //   - a full cold boot: BOTH 2600 and 2500 descriptors + >= 10000 B
    //     downloaded on ep 02 (the MCU program), or
    //   - a 2500 session: the 2500 descriptor + >= 10000 B on ep 02 AND
    //     >= 10000 B on ep 82 (image download + verified echo).
    if (r.cam2600 >= 0 && r.cam2500 >= 0 && r.ep02Bytes >= 10000) { return "PASS"; }
    if (r.cam2500 >= 0 && r.ep02Bytes >= 10000 && r.ep82Bytes >= 10000) { return "PASS"; }
    return "PARTIAL";
}
function rank(v) {                     // lower = better
    if (v === "PASS") { return 0; }
    if (v === "PARTIAL") { return 1; }
    if (v === "ERROR") { return 3; }
    return 2;
}
function codeOf(v) {
    if (v === "PASS") { return 0; }
    if (v === "PARTIAL") { return 2; }
    if (v === "ERROR") { return 3; }
    return 1;
}
function fmtSize(n) {
    if (n >= 1048576) { return (Math.round(n / 104857.6) / 10) + " MB"; }
    if (n >= 1024) { return Math.round(n / 1024) + " KB"; }
    return n + " bytes";
}
function fmtTime(sec) {
    var d = new Date(sec * 1000), m;
    m = (d.getMonth() + 1) + "/" + d.getDate() + " " + d.getHours() + ":";
    m += (d.getMinutes() < 10 ? "0" : "") + d.getMinutes() + ":";
    m += (d.getSeconds() < 10 ? "0" : "") + d.getSeconds();
    return m;
}

function gateLines(out, r) {
    var v = verdict(r), a = r.cam2600, b = r.cam2500, same = (a >= 0 && a === b);
    out.push("File    : " + r.path);
    if (r.err !== "") {
        out.push("Status  : ERROR - " + r.err);
        out.push("");
        return v;
    }
    out.push("Size    : " + fmtSize(r.size) + ", " + r.packets + " USB packets, " + r.data + " with data");
    if (r.firstTs > 0) {
        out.push("Time    : first packet " + fmtTime(r.firstTs) + ", last packet " +
                 (r.lastTs >= r.firstTs ? r.lastTs - r.firstTs : r.firstTs - r.lastTs) + " s later");
    }
    if (r.warn !== "") { out.push("Note    : " + r.warn); }
    if (r.truncated) {
        out.push("Note    : the last packet is cut off - the capture was still running when the file was read");
    }
    if (a < 0 && b < 0) {
        out.push("Sensor  : no 0471:2600 and no 0471:2500 device descriptor in this file");
    } else {
        out.push("Sensor  : 0471:2600 " + (a < 0 ? "not seen" : "USB address " + a) +
                 ",  0471:2500 " + (b < 0 ? "not seen" : "USB address " + b));
    }
    if (a >= 0) {
        out.push("MCU     : " + (r.outFrames[a] || 0) + " frames host to sensor, " + (r.inFrames[a] || 0) +
                 " frames sensor to host" + (same ? "  (2600 and 2500 share this address, both phases are summed)"
                                                  : "  - the MCU program download on 0471:2600"));
    }
    if (b >= 0 && !same) {
        out.push("DSP     : " + (r.outFrames[b] || 0) + " frames host to sensor, " + (r.inFrames[b] || 0) +
                 " frames sensor to host, " + fmtSize(r.inBytes[b] || 0) +
                 " from the sensor  - the DSP and FPGA upload and the running program on 0471:2500");
    }
    if (a < 0 && b >= 0) {
        out.push("Note    : the capture begins with 0471:2500. The 2600 download happened before the capture started.");
        out.push("          Unplug the sensor and plug it in again while the capture is running.");
    } else if (a >= 0 && b < 0) {
        out.push("Note    : 0471:2500 did not appear inside this capture: the MCU download was not completed.");
        out.push("          Have the vendor program running, wait longer before you stop the capture.");
    }
    out.push("Framing : " + r.pu + " PU / " + r.od + " OD frames (legacy DLL model, 0 expected), " +
             r.rtOut + " + " + r.rtIn + " frames 7f 55 on endpoint 02 and 82");
    out.push("Version : 0x0008 replies \"2600\" x " + r.v2600 + " (image not loaded), " +
             "\"2619\" x " + r.v2619 + " (app running)");
    out.push("Data    : " + fmtSize(r.ep02Bytes) + " host to sensor (ep 02), " + fmtSize(r.ep82Bytes) +
             " sensor to host (ep 82)");
    out.push("Banner  : \"Ready to load program!\" " + (r.ready > 0 ? "FOUND, " + r.ready + " time(s)" : "not found"));
    if (v === "PASS") {
        out.push("Verdict : PASS - a complete sensor session is in this file (a cold boot with");
        out.push("          the download, or a 2500 session with the image download and its echo).");
    } else if (v === "PARTIAL") {
        out.push("Verdict : PARTIAL - sensor frames are there, but no complete session (less than");
        out.push("          Start the capture BEFORE the sensor is plugged in and stop it only after the vendor program is up.");
    } else if (v === "ENUM-ONLY") {
        out.push("Verdict : FAIL - the sensor was enumerated but the vendor software sent no frames.");
        out.push("          Is the vendor program running in this Windows system (service xEyeService and xeyez.exe)?");
    } else if (v === "NO-CAMERA") {
        out.push("Verdict : FAIL - no sensor traffic in this file. The sensor sits on another USB root hub, see the other files.");
    } else if (v === "EMPTY") {
        out.push("Verdict : FAIL - USBPcap wrote only the file header, no USB packet was recorded on this root hub.");
    }
    out.push("");
    return v;
}

function infoLines(out, r) {
    var name = r.path.replace(/^.*\\/, ""), line;
    if (r.err !== "") {
        out.push("ERROR  " + name + "  " + r.err);
    } else {
        line = (r.packets > 0 ? "OK     " : "EMPTY  ") + name + "  " + fmtSize(r.size) + ", " +
               r.packets + " packets, " + r.data + " with data";
        if (r.truncated) { line += " (last packet cut off)"; }
        out.push(line);
    }
    return r.packets > 0 ? 0 : 1;
}

function emit(out, reportPath) {
    var i, fso, f;
    for (i = 0; i < out.length; i++) { WScript.Echo(out[i]); }
    if (reportPath === "") { return; }
    try {
        fso = new ActiveXObject("Scripting.FileSystemObject");
        f = fso.CreateTextFile(reportPath, true, false);
        for (i = 0; i < out.length; i++) { f.WriteLine(out[i]); }
        f.Close();
    } catch (e) { WScript.Echo("[WARN] could not write the report file: " + reportPath); }
}

function main() {
    var args = WScript.Arguments, n = args.length, i, mode = "gate", reportPath = "", first = 0;
    var out = [], results = [], overall = "", any = 1, v;
    if (n > 0 && String(args(0)) === "--info") { mode = "info"; first = 1; }
    else if (n > 0 && String(args(0)) === "--report") {
        if (n > 1) { reportPath = String(args(1)); }
        first = 2;
    }
    if (n <= first) {
        out.push("[ERR] no .pcap file was given to the scanner.");
        emit(out, reportPath);
        WScript.Quit(3);
    }
    for (i = first; i < n; i++) { results.push(scanFile(String(args(i)))); }
    if (mode === "info") {
        for (i = 0; i < results.length; i++) { if (infoLines(out, results[i]) === 0) { any = 0; } }
        emit(out, "");
        WScript.Quit(any);
    }
    for (i = 0; i < results.length; i++) {
        v = gateLines(out, results[i]);
        if (overall === "" || rank(v) < rank(overall)) { overall = v; }
    }
    out.push("OVERALL : " + overall + "  - the best of " + results.length + " file(s). Send the whole folder to the analysis host.");
    emit(out, reportPath);
    WScript.Quit(codeOf(overall));
}

main();
