#!/usr/bin/env python3
"""dis50_hal.py - turn-50: disassemble named xeyezHal.dll functions with
export/IAT/string annotation (extends dis46.py with import-name thunks
and GBK strings).
"""
import re
import sys
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

DLL = sys.argv[1]
TARGETS = []
for a in sys.argv[2:]:
    name = None
    if ":" in a:
        a, name = a.split(":", 1)
    TARGETS.append((int(a, 16), name))

data = open(DLL, "rb").read()
pe = pefile.PE(data=data, fast_load=True)
pe.parse_data_directories(directories=[
    pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_EXPORT'],
    pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_IMPORT'],
])
base = pe.OPTIONAL_HEADER.ImageBase

exps = {}
if hasattr(pe, 'DIRECTORY_ENTRY_EXPORT'):
    for e in pe.DIRECTORY_ENTRY_EXPORT.symbols:
        if e.name:
            exps[e.address] = e.name.decode()

iat = {}
if hasattr(pe, 'DIRECTORY_ENTRY_IMPORT'):
    for entry in pe.DIRECTORY_ENTRY_IMPORT:
        for imp in entry.imports:
            nm = imp.name.decode() if imp.name else f"{entry.dll.decode()}!ord{imp.ordinal}"
            iat[imp.address] = f"{entry.dll.decode()}!{nm}"

secs = []
for s in pe.sections:
    secs.append((s.VirtualAddress,
                 s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData),
                 s.PointerToRawData, s.get_data()))

def rva2off(rva):
    for va, end, raw, d in secs:
        if va <= rva < end:
            o = rva - va
            if o < len(d):
                return raw + o
    return None

def str_at(rva, maxlen=96):
    off = rva2off(rva)
    if off is None:
        return None
    raw = data[off:off + maxlen]
    n = raw.find(b'\x00')
    if n > 0:
        s = raw[:n]
        if all(32 <= b < 127 for b in s):
            return s.decode()
        # GBK?
        try:
            g = s.decode("gbk")
            if all(c.isprintable() for c in g):
                return f"GBK:{g}"
        except Exception:
            pass
    return None

def thunk_target(rva):
    off = rva2off(rva)
    if off is None:
        return None
    b = data[off:off + 8]
    if b[:2] == b'\xff\x25':
        addr = int.from_bytes(b[2:6], 'little')
        return iat.get(addr, f"[{addr:#x}]")
    if b[0] == 0xE9:
        rel = int.from_bytes(b[1:5], 'little', signed=True)
        return ("jmp", rva + 5 + rel)
    return None

def disasm(rva, label, max_instrs=2000):
    for va, end, raw, d in secs:
        if va <= rva < end:
            code = d[rva - va:]
            break
    else:
        print(f"!! {rva:#x} not in section")
        return
    md = Cs(CS_ARCH_X86, CS_MODE_32)
    out = []
    n = 0
    for ins in md.disasm(code, base + rva):
        line = f"{ins.address - base:08x}  {ins.bytes.hex():<18} {ins.mnemonic} {ins.op_str}"
        ann = ""
        if ins.mnemonic in ('call', 'jmp') and ins.op_str.startswith('0x'):
            t = int(ins.op_str, 16) - base
            if t in exps:
                ann = f"  ; {exps[t]}"
            else:
                tt = thunk_target(t)
                if isinstance(tt, str) and '!' in tt:
                    ann = f"  ; {tt}"
                elif isinstance(tt, tuple):
                    inner = exps.get(tt[1], f"{tt[1]:#x}")
                    ann = f"  ; thunk -> {inner}"
                else:
                    ann = f"  ; ->{t:#x}"
        if 'call' in ins.mnemonic and '[0x' in ins.op_str:
            try:
                a = int(ins.op_str.split('[', 1)[1].rstrip(']'), 16)
                if a in iat:
                    ann = f"  ; {iat[a]}"
            except Exception:
                pass
        if ins.mnemonic == 'push' and ins.op_str.startswith('0x'):
            try:
                v = int(ins.op_str, 16)
                if base < v < base + 0x100000:
                    s = str_at(v - base)
                    if s:
                        ann += f'  ; "{s}"'
            except Exception:
                pass
        if ', 0x' in ins.op_str and ins.mnemonic == 'mov':
            try:
                v = int(ins.op_str.split(', 0x')[1].split()[0], 16)
                if base < v < base + 0x100000:
                    s = str_at(v - base)
                    if s:
                        ann += f'  ; "{s}"'
            except Exception:
                pass
        out.append(line + ann)
        n += 1
        if n >= max_instrs:
            out.append("... cut")
            break
        if ins.mnemonic == 'ret':
            break
    print(f"### {label} @ RVA {rva:#x}  {exps.get(rva, '')}")
    print("\n".join(out))
    print()

for rva, name in TARGETS:
    disasm(rva, name or "")
