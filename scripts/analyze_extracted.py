#!/usr/bin/env python3
"""analyze_extracted.py — unified analysis of every file extracted this turn.

Inputs (all under /home/z/my-project/analysis/):
  remote/                    - members fetched straight from download.i-dong.com zips
  nsis/                      - NSIS-extracted xeyez driver stacks (v2.5.0.7 + v2.6.0.6)
  extracted_sportsking/      - Inno-extracted Sports King camera/middleware files

Outputs (under /home/z/my-project/analysis/reports/):
  all_extracted_sha256.tsv   - SHA256 + MD5 + size + path for EVERY extracted file
  pe_inventory.txt           - PE exports/imports/timestamps for every DLL/EXE/SYS
  firmware_report.txt        - FPGA .bit / Blackfin .ldr / 8051 .hex / data*.dat / SVM
  config_dumps.txt           - small config/log/sol files, GBK-decoded
  version_diff.txt           - v2.5.0.7 vs v2.6.0.6 file-set diff + xeyezHal export diff
"""
import hashlib
import os
import struct
import sys

BASE = '/home/z/my-project/analysis'
REPORTS = os.path.join(BASE, 'reports')
os.makedirs(REPORTS, exist_ok=True)

ROOTS = [
    os.path.join(BASE, 'remote'),
    os.path.join(BASE, 'nsis'),
    os.path.join(BASE, 'extracted_sportsking'),
]


def sha_md5(path):
    h1 = hashlib.sha256()
    h2 = hashlib.md5()
    with open(path, 'rb') as f:
        while True:
            b = f.read(1 << 20)
            if not b:
                break
            h1.update(b)
            h2.update(b)
    return h1.hexdigest(), h2.hexdigest()


def safe(s):
    """Paths from os.listdir may carry surrogate escapes (GBK names)."""
    return s.encode('utf-8', 'replace').decode('utf-8')


def build_manifest():
    rows = []
    for root in ROOTS:
        for dirpath, _dirs, files in os.walk(root):
            for fn in files:
                if fn.endswith('.part.deflate'):
                    continue
                p = os.path.join(dirpath, fn)
                sha, md5 = sha_md5(p)
                rows.append((sha, md5, os.path.getsize(p), p))
    rows.sort(key=lambda r: r[3])
    out = os.path.join(REPORTS, 'all_extracted_sha256.tsv')
    with open(out, 'w', encoding='utf-8') as f:
        f.write('sha256\tmd5\tsize\tpath\n')
        for sha, md5, size, p in rows:
            f.write(f'{sha}\t{md5}\t{size}\t{safe(p)}\n')
    print(f'[manifest] {len(rows)} files -> {out}')
    return rows


# ---------------------------------------------------------------- PE inventory
def pe_inventory():
    import pefile
    out_path = os.path.join(REPORTS, 'pe_inventory.txt')
    targets = []
    for root in ROOTS:
        for dirpath, _dirs, files in os.walk(root):
            for fn in files:
                if fn.lower().endswith(('.dll', '.exe', '.sys', '.ocx', '.ax')):
                    targets.append(os.path.join(dirpath, fn))
    targets.sort()
    with open(out_path, 'w', encoding='utf-8') as out:
        for p in targets:
            try:
                pe = pefile.PE(p, fast_load=False)
            except Exception as e:
                out.write(f'== {p}\n   NOT A PE / error: {e}\n\n')
                continue
            sha, _ = sha_md5(p)
            ts = pe.FILE_HEADER.TimeDateStamp
            import datetime
            when = datetime.datetime.utcfromtimestamp(ts).strftime('%Y-%m-%d %H:%M:%S')
            machine = hex(pe.FILE_HEADER.Machine)
            out.write(f'== {os.path.relpath(p, BASE)}\n')
            out.write(f'   sha256={sha}  size={os.path.getsize(p):,}  '
                      f'link={when}  machine={machine}  subsys='
                      f'{pe.OPTIONAL_HEADER.Subsystem}\n')
            # sections
            secs = ', '.join(f'{s.Name.decode(errors="replace").rstrip(chr(0))}:'
                             f'0x{s.VirtualAddress:08x}+0x{s.Misc_VirtualSize:x}'
                             for s in pe.sections)
            out.write(f'   sections: {secs}\n')
            # exports
            exps = []
            if hasattr(pe, 'DIRECTORY_ENTRY_EXPORT'):
                for sym in pe.DIRECTORY_ENTRY_EXPORT.symbols:
                    if sym.name:
                        exps.append(sym.name.decode(errors='replace'))
                    else:
                        exps.append(f'#ord{sym.ordinal}')
            out.write(f'   exports ({len(exps)}): '
                      + ', '.join(exps[:120])
                      + (' ...' if len(exps) > 120 else '') + '\n')
            # imports (dll: count)
            imps = []
            if hasattr(pe, 'DIRECTORY_ENTRY_IMPORT'):
                for entry in pe.DIRECTORY_ENTRY_IMPORT:
                    n = len(entry.imports)
                    imps.append(f'{entry.dll.decode(errors="replace")}({n})')
            out.write(f'   imports: {", ".join(imps[:40])}\n')
            # version info
            if hasattr(pe, 'FileInfo') and pe.FileInfo:
                for fi in pe.FileInfo[0]:
                    if fi.Key.decode(errors='replace') == 'StringFileInfo':
                        for st in fi.StringTable:
                            for k, v in st.entries.items():
                                out.write(f'   ver {k.decode()}={v.decode(errors="replace")}\n')
            out.write('\n')
            pe.close()
    print(f'[pe] {len(targets)} PEs -> {out_path}')


# ------------------------------------------------------------------- firmware
def parse_bit(path):
    """Xilinx Spartan-3 bitstream header: sync 00 09 0f f0.. then key blocks."""
    d = open(path, 'rb').read()
    lines = [f'== {os.path.relpath(path, BASE)} ({len(d):,} B)']
    if d[:4] != b'\x00\x09\x0f\xf0':
        lines.append('   not a Xilinx bitstream (bad sync)')
        return '\n'.join(lines)
    p = 4
    # sync field is 0f f0 0f f0 0f f0 00 00 01
    p = 12  # after 00 09 0f f0 0f f0 0f f0 0f f0 00 00 01 -> actually 13 bytes
    # robust: scan for 00 00 01 'a'
    i = d.find(b'\x00\x00\x01a')
    if i < 0:
        lines.append('   key section not found')
        return '\n'.join(lines)
    p = i + 3  # at 'a'
    keys = {}
    while p < len(d):
        key = d[p:p + 1]
        if not key:
            break
        p += 1
        if key == b'e':
            if p + 4 > len(d):
                break
            val = struct.unpack('>I', d[p:p + 4])[0]
            keys['e'] = f'0x{val:08x} ({val:,} B payload)'
            p += 4
            break
        else:
            if p + 2 > len(d):
                break
            n = struct.unpack('>H', d[p:p + 2])[0]
            p += 2
            val = d[p:p + n].split(b'\x00')[0].decode('latin1', 'replace')
            keys[key.decode()] = val
            p += n
    label = {'a': 'design', 'b': 'part', 'c': 'date', 'd': 'time', 'e': 'length'}
    for k, v in keys.items():
        lines.append(f'   {label.get(k, k)}: {v}')
    return '\n'.join(lines)


def parse_ldr(path):
    """Blackfin .ldr (BF527 boot image). Note: the exact block framing of this
    vendor file could not be matched to the standard [u32 addr][u32 count]
    zerofill/init flag conventions (empirically rejected); we report the
    verified first block, the L1 target histogram, and embedded strings
    (ASCII + UTF-16LE, which carry symbol names)."""
    import re as _re
    d = open(path, 'rb').read()
    lines = [f'== {os.path.relpath(path, BASE)} ({len(d):,} B)',
             f'   first block: addr 0xFF800040 (L1 data bank A), count 4, '
             f'data {d[8:12].hex(" ")}']
    # plausible [addr u32 in Blackfin L1/MMR space][count u32 < 64K] pairs
    plaus = []
    for p in range(0, len(d) - 8):
        addr, cnt = struct.unpack('<II', d[p:p + 8])
        if (0xFF800000 <= addr < 0xFFC00000) and 0 < cnt <= 0xFFFF:
            plaus.append((p, addr, cnt))
    regions = {}
    for _p, addr, _c in plaus:
        r = (addr >> 20) & 0xF
        name = {8: 'L1 data A (0xFF8xxxxx)', 9: 'L1 data B (0xFF9xxxxx)',
                0xA: 'L1 instr (0xFFAxxxxx)', 0xB: 'L1 scratch/other (0xFFBxxxxx)'}.get(r, hex(addr))
        regions[name] = regions.get(name, 0) + 1
    lines.append(f'   {len(plaus)} plausible [L1-addr][count] header positions; '
                 f'target histogram: ' + ', '.join(f'{k}:{v}' for k, v in sorted(regions.items())))
    # pointer table into L1 instruction memory (function pointers)
    ptrs = [struct.unpack('<I', d[p:p + 4])[0] for p in range(0, len(d) - 3, 4)
            if 0xFFA00000 <= struct.unpack('<I', d[p:p + 4])[0] < 0xFFA10000]
    uniq = sorted(set(ptrs))
    lines.append(f'   {len(ptrs)} u32 values pointing into L1 instruction SRAM '
                 f'({len(uniq)} unique, range {hex(min(uniq))}-{hex(max(uniq))}) '
                 f'-> function-pointer/jump tables')
    # ASCII strings
    def ascii_strings(minlen):
        out = []
        cur = bytearray()
        start = 0
        for i, b in enumerate(d):
            if 32 <= b < 127:
                if not cur:
                    start = i
                cur.append(b)
            else:
                if len(cur) >= minlen:
                    out.append((start, cur.decode('latin1')))
                cur = bytearray()
        return out
    astrings = [s for s in ascii_strings(6)
                if _re.match(r'^[A-Za-z_][A-Za-z0-9_ .:;/\\-]*$', s[1]) and len(s[1]) >= 5]
    lines.append(f'   {len(astrings)} clean ASCII strings (identifier-like), all:')
    for off, s in astrings[:80]:
        lines.append(f'     @{off:#08x} {s[:100]}')
    # UTF-16LE strings
    ustrings = [(m.start() // 2, m.group().decode('utf-16-le'))
                for m in _re.finditer(b'(?:[ -~]\x00){4,}', d)]
    lines.append(f'   {len(ustrings)} UTF-16LE strings:')
    for off, s in ustrings[:80]:
        lines.append(f'     @{off * 2:#08x} {s[:100]}')
    return '\n'.join(lines)


def parse_ihex(path):
    """Intel HEX -> memory map + strings (8051 firmware)."""
    mem = {}
    base_hi = 0
    ext = False
    total = 0
    for raw in open(path, 'r', errors='replace'):
        line = raw.strip()
        if not line or not line.startswith(':'):
            continue
        b = bytes.fromhex(line[1:])
        cnt, ahi, alo, rtype = b[0], b[1], b[2], b[3]
        addr = base_hi + (ahi << 8 | alo)
        data = b[4:4 + cnt]
        if rtype == 0:
            for i, v in enumerate(data):
                mem[addr + i] = v
            total += cnt
        elif rtype == 1:
            break
        elif rtype == 2:
            base_hi = ((data[0] << 8 | data[1]) << 4) & 0xFFFF0
            ext = True
        elif rtype == 4:
            base_hi = ((data[0] << 8 | data[1]) << 16) & 0xFFFF0000
            ext = True
    if not mem:
        return f'== {path}\n   no data records'
    lo, hi = min(mem), max(mem)
    # contiguous spans
    spans = []
    s = lo
    prev = lo
    for a in sorted(mem):
        if a > prev + 1:
            spans.append((s, prev))
            s = a
        prev = a
    spans.append((s, prev))
    lines = [f'== {os.path.relpath(path, BASE)}',
             f'   {total:,} data bytes, addresses {lo:#x}..{hi:#x}, {len(spans)} spans:']
    for a, b in spans[:30]:
        lines.append(f'     {a:#08x}-{b:#08x}  ({b - a + 1:,} B)')
    # strings
    blob = bytearray()
    cur = bytearray()
    strings = []
    for a in sorted(mem):
        v = mem[a]
        if 32 <= v < 127:
            if not cur:
                start = a
            cur.append(v)
        else:
            if len(cur) >= 6:
                strings.append((start, cur.decode('latin1')))
            cur = bytearray()
    lines.append(f'   {len(strings)} strings >=6 chars; first 60:')
    for off, s in strings[:60]:
        lines.append(f'     @{off:#08x} {s[:100]}')
    return '\n'.join(lines)


def parse_blob(path, note=''):
    d = open(path, 'rb').read()
    sha, md5 = sha_md5(path)
    head = ' '.join(f'{b:02x}' for b in d[:32])
    lines = [f'== {os.path.relpath(path, BASE)} ({len(d):,} B) {note}',
             f'   sha256={sha}',
             f'   head: {head}']
    cur = bytearray()
    strings = []
    for i, b in enumerate(d):
        if 32 <= b < 127:
            if not cur:
                st = i
            cur.append(b)
        else:
            if len(cur) >= 6:
                strings.append((st, cur.decode('latin1')))
            cur = bytearray()
    for off, s in strings[:25]:
        lines.append(f'     @{off:#08x} {s[:90]}')
    return '\n'.join(lines)


def firmware_report():
    out = []
    v6 = os.path.join(BASE, 'nsis', 'xeyez_pc_v2.6.0.6', 'xeyez_Lobby', '$_6_')
    v5 = os.path.join(BASE, 'nsis', 'xeyez_pc_v2.5.0.7', 'xeyez_Lobby', '$_6_')
    out.append('# Firmware & data files — v2.6.0.6 stack')
    for name in ['A_a_v4.4_BG0313.bit', 'ens_code.ldr', 'app_m8051_2500.hex',
                 'app_m8051_2611.hex']:
        p = os.path.join(v6, name)
        if os.path.exists(p):
            if name.endswith('.bit'):
                out.append(parse_bit(p))
            elif name.endswith('.ldr'):
                out.append(parse_ldr(p))
            else:
                out.append(parse_ihex(p))
    for i in range(11):
        p = os.path.join(v6, f'data{i}.dat')
        if os.path.exists(p):
            out.append(parse_blob(p, '(data*.dat — 8051 page payloads?)'))
    for name in sorted(os.listdir(v6)):
        if name.startswith('SVM_'):
            out.append(parse_blob(os.path.join(v6, name), '(SVM classifier)'))
    # old-gen hexes
    for extra in ['xeyez_v20.hex', 'xeyez_v21.hex']:
        p = os.path.join(v6, extra)
        if os.path.exists(p):
            out.append(parse_blob(p, '(legacy gen hex)'))
    # v5 stack firmware for diff
    out.append('\n# Firmware & data files — v2.5.0.7 stack')
    for name in ['A_a_v4.4_BG0313.bit', 'ens_code.ldr', 'app_m8051_2500.hex',
                 'app_m8051_2611.hex']:
        p = os.path.join(v5, name)
        if os.path.exists(p):
            sha, _ = sha_md5(p)
            out.append(f'== {os.path.relpath(p, BASE)} sha256={sha}')
    out_path = os.path.join(REPORTS, 'firmware_report.txt')
    with open(out_path, 'w', encoding='utf-8') as f:
        f.write('\n\n'.join(out))
    print(f'[firmware] -> {out_path}')


# --------------------------------------------------------------- config dumps
def config_dumps():
    files = []
    sk = os.path.join(BASE, 'extracted_sportsking')
    for rel in ['Lobby/conf/camera_set/settings.sol',
                'Lobby/conf/camera_set/#local/settings.sol',
                'Lobby/data/config.ini', 'Lobby/data/userconfig.ini',
                'Lobby/iDongAction.ini', 'Lobby/iDongAnalysisModels.ini',
                'Lobby/LoadFlash.ini', 'Lobby/configInfo.data',
                'Launcher/update.ini', 'Launcher/iDongSetup.txt', 'Launcher/LoadProc.dat',
                'Dlls/iDongAction3.0/iDongActionLog/Flash.log',
                'Dlls/iDongAction3.0/iDongActionLog/iDongActionFrame.log',
                'Dlls/iDongAction3.0/iDongActionLog/iDongLog.log.log',
                'Dlls/iDongAction3.0/iDongActionLog/iDongSimulator3.log',
                'Dlls/iDongAction3.0/iDongActionLog/iDongSimulator3Manager.log']:
        p = os.path.join(sk, rel)
        if os.path.exists(p):
            files.append(p)
    v6 = os.path.join(BASE, 'nsis', 'xeyez_pc_v2.6.0.6', 'xeyez_Lobby', '$_6_')
    for name in ['xEyeUsb.ini', 'xEye.ini', 'xeyezHal.ini', 'DriverConfig.ini',
                 'Classifying.ini', 'ScConfig.ini', 'Co2D.ini', 'Calculator.txt',
                 'mouse_contrl_config.ini', 'xEyeDriverCheck.ini',
                 'xEyeLanguage_eng.ini', '30fps/30fps_Global_0316_Cmos.txt',
                 'Global_RW_0316_Cmos_v26.txt', 'Rolling_RW_0316_Cmos_v26.txt',
                 'Led_Scale_Fpga_v26.txt']:
        p = os.path.join(v6, name)
        if os.path.exists(p):
            files.append(p)
    out_path = os.path.join(REPORTS, 'config_dumps.txt')
    with open(out_path, 'w', encoding='utf-8') as f:
        for p in files:
            data = open(p, 'rb').read()
            sha, _ = sha_md5(p)
            f.write(f'===== {os.path.relpath(p, BASE)} ({len(data):,} B, sha256={sha})\n')
            try:
                text = data.decode('gbk')
            except UnicodeDecodeError:
                try:
                    text = data.decode('utf-8')
                except UnicodeDecodeError:
                    text = data.decode('latin1')
            f.write(text if text.endswith('\n') else text + '\n')
            f.write('\n\n')
    print(f'[configs] {len(files)} files -> {out_path}')


# ---------------------------------------------------------------- version diff
def version_diff():
    import pefile
    a = os.path.join(BASE, 'nsis', 'xeyez_pc_v2.5.0.7', 'xeyez_Lobby', '$_6_')
    b = os.path.join(BASE, 'nsis', 'xeyez_pc_v2.6.0.6', 'xeyez_Lobby', '$_6_')
    la = {f: sha_md5(os.path.join(a, f))[0] for f in os.listdir(a)
          if os.path.isfile(os.path.join(a, f))}
    lb = {f: sha_md5(os.path.join(b, f))[0] for f in os.listdir(b)
          if os.path.isfile(os.path.join(b, f))}
    out = ['# v2.5.0.7 vs v2.6.0.6 (both from vendor server, NSIS payloads)',
           f'v2.5.0.7: {len(la)} files | v2.6.0.6: {len(lb)} files', '',
           '## only in v2.6.0.6:']
    for f in sorted(set(lb) - set(la)):
        out.append(f'  + {f}  ({os.path.getsize(os.path.join(b, f)):,} B)')
    out.append('\n## only in v2.5.0.7:')
    for f in sorted(set(la) - set(lb)):
        out.append(f'  - {f}  ({os.path.getsize(os.path.join(a, f)):,} B)')
    out.append('\n## same name, different content:')
    for f in sorted(set(la) & set(lb)):
        if la[f] != lb[f]:
            out.append(f'  ~ {f}  ({os.path.getsize(os.path.join(a, f)):,} B '
                       f'vs {os.path.getsize(os.path.join(b, f)):,} B)')
    out.append('\n## xeyezHal.dll export diff:')
    exps = {}
    for tag, root in (('v2.5.0.7', a), ('v2.6.0.6', b)):
        pe = pefile.PE(os.path.join(root, 'xeyezHal.dll'))
        exps[tag] = ([s.name.decode() for s in pe.DIRECTORY_ENTRY_EXPORT.symbols
                      if s.name] if hasattr(pe, 'DIRECTORY_ENTRY_EXPORT') else [])
        pe.close()
    ea, eb = set(exps['v2.5.0.7']), set(exps['v2.6.0.6'])
    out.append(f"  v2.5.0.7: {len(ea)} exports | v2.6.0.6: {len(eb)} exports")
    out.append('  added in v2.6.0.6: ' + ', '.join(sorted(eb - ea)))
    out.append('  removed in v2.6.0.6: ' + (', '.join(sorted(ea - eb)) or '(none)'))
    out_path = os.path.join(REPORTS, 'version_diff.txt')
    with open(out_path, 'w', encoding='utf-8') as f:
        f.write('\n'.join(out) + '\n')
    print(f'[diff] -> {out_path}')


if __name__ == '__main__':
    build_manifest()
    pe_inventory()
    firmware_report()
    config_dumps()
    version_diff()
    print('done.')
