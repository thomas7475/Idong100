#!/usr/bin/env python3
"""dis46.py - turn-46 disassembler: dump named functions of the v2.6.0.6 engine
DLLs with IAT/export/string annotations.

Usage: python3 dis46.py <dll> <rva-hex> [<rva-hex> ...]
"""
import sys
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

DLL = sys.argv[1]
RVAS = [int(a, 16) for a in sys.argv[2:]]

data = open(DLL, 'rb').read()
pe = pefile.PE(data=data, fast_load=True)
pe.parse_data_directories(directories=[
    pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_EXPORT'],
    pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_IMPORT'],
])
base = pe.OPTIONAL_HEADER.ImageBase

# export map (RVA -> name)
exps = {}
if hasattr(pe, 'DIRECTORY_ENTRY_EXPORT'):
    for e in pe.DIRECTORY_ENTRY_EXPORT.symbols:
        if e.name:
            exps[e.address] = e.name.decode()

# IAT: VA of thunk slot -> dll!name
iat = {}
if hasattr(pe, 'DIRECTORY_ENTRY_IMPORT'):
    for entry in pe.DIRECTORY_ENTRY_IMPORT:
        dll = entry.dll.decode()
        for imp in entry.imports:
            if imp.name:
                iat[imp.address] = f"{dll}!{imp.name.decode()}"

# sections: (rva_start, rva_end, data)
secs = []
for s in pe.sections:
    secs.append((s.VirtualAddress,
                 s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData),
                 s.get_data()))

def rva2off(rva):
    for s in pe.sections:
        if s.VirtualAddress <= rva < s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData):
            off = rva - s.VirtualAddress
            if off < len(s.get_data()):
                return s.PointerToRawData + off
    return None

def str_at(rva, maxlen=64):
    off = rva2off(rva)
    if off is None:
        return None
    raw = data[off:off + maxlen]
    if not raw:
        return None
    # ascii
    n = raw.find(b'\x00')
    if 3 <= n and all(32 <= b < 127 for b in raw[:n]):
        return raw[:n].decode()
    return None

def fnname(rva):
    if rva in exps:
        # demangle lightly: keep it short
        n = exps[rva]
        for p in ('?GetADataFromUSB@xEyeClassManager@@EAEXABUxEye3Ddata@@@Z',):
            pass
        return n
    return None

def disasm(rva, max_instrs=900):
    for (s0, s1, d) in secs:
        if s0 <= rva < s1:
            code = d[rva - s0:]
            break
    else:
        print(f"!! RVA {rva:#x} not in any section")
        return
    md = Cs(CS_ARCH_X86, CS_MODE_32)
    md.detail = False
    out = []
    for i, ins in enumerate(md.disasm(code, base + rva)):
        va = ins.address
        line = f"{va - base:08x}  {ins.bytes.hex():<20} {ins.mnemonic} {ins.op_str}"
        ann = ''
        # e8/e9 rel target
        if ins.mnemonic in ('call', 'jmp') and ins.op_str.startswith('0x'):
            t = int(ins.op_str, 16) - base
            if t in exps:
                ann = f"  ; {exps[t]}"
            elif t in iat.values():
                pass
            else:
                ann = f"  ; ->{t:#x}"
        # ff15 dword ptr [iat]
        if ins.mnemonic.startswith('call') and '[0x' in ins.op_str:
            try:
                a = int(ins.op_str.split('[', 1)[1].rstrip(']'), 16)
                if a in iat:
                    ann = f"  ; {iat[a]}"
            except Exception:
                pass
        # push imm = string?
        if ins.mnemonic == 'push' and ins.op_str.startswith('0x'):
            try:
                v = int(ins.op_str, 16)
                if 0 < v < 0x7fffffff:
                    r = v - base
                    s = str_at(r)
                    if s:
                        ann = f'  ; "{s}"'
            except Exception:
                pass
        # mov reg, imm (string or float const)
        if ', 0x' in ins.op_str and ins.mnemonic in ('mov',):
            try:
                v = int(ins.op_str.split(', 0x')[1], 16)
                if 0 < v < 0x7fffffff:
                    s = str_at(v - base)
                    if s:
                        ann = f'  ; "{s}"'
            except Exception:
                pass
        out.append(line + ann)
        if i >= max_instrs:
            out.append('... max instrs')
            break
        if ins.mnemonic == 'ret':
            break
    print(f"### {DLL} RVA {rva:#x}  {fnname(rva) or ''}")
    print('\n'.join(out))
    print()

for r in RVAS:
    disasm(r)
