#!/usr/bin/env python3
"""Disassemble exported Init/IsSigned/GetXYZStr of VideoSign.dll and the
Init path of LobbyConn.dll - what makes the lobby consider the sensor connected?"""
import pefile
import capstone
import sys

def load(path):
    pe = pefile.PE(path)
    base = pe.OPTIONAL_HEADER.ImageBase
    exps = {}
    if hasattr(pe, 'DIRECTORY_ENTRY_EXPORT'):
        for e in pe.DIRECTORY_ENTRY_EXPORT.symbols:
            if e.name:
                exps[e.name.decode()] = base + e.address
    imps = {}
    if hasattr(pe, 'DIRECTORY_ENTRY_IMPORT'):
        for entry in pe.DIRECTORY_ENTRY_IMPORT:
            dll = entry.dll.decode().lower()
            for imp in entry.imports:
                if imp.name:
                    imps[imp.address] = f"{dll}!{imp.name.decode()}"
    secs = []
    for sec in pe.sections:
        secs.append((base + sec.VirtualAddress, sec.get_data()))
    return pe, base, exps, imps, secs

def find_calls_to(base, secs, target):
    """find e8/e9 rel32 targeting target; also ff15 for IAT thunk usage"""
    sites = []
    for va, code in secs:
        for i in range(len(code) - 5):
            if code[i] in (0xe8, 0xe9):
                rel = int.from_bytes(code[i+1:i+5], 'little', signed=True)
                if va + i + 5 + rel == target:
                    sites.append((va + i, 'call' if code[i] == 0xe8 else 'jmp'))
    return sites

def disasm_fn(base, secs, start, length, imps, max_lines=200):
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
    for va, code in secs:
        if va <= start < va + len(code):
            off = start - va
            out = []
            for ins in md.disasm(code[off:off+length], start):
                tgt = ""
                if ins.mnemonic in ('call', 'jmp') and ins.op_str.startswith('dword ptr ['):
                    try:
                        a = int(ins.op_str.split('[')[1].rstrip(']'), 16)
                        if a in imps:
                            tgt = f"  ; {imps[a]}"
                    except Exception:
                        pass
                out.append(f"  {ins.address:#x}: {ins.mnemonic} {ins.op_str}{tgt}")
                if len(out) >= max_lines:
                    break
            return out
    return []

path = sys.argv[1]
fns = sys.argv[2:]
pe, base, exps, imps, secs = load(path)
print(f"== {path} exports: {list(exps.items())[:10]}")
for fn in fns:
    if fn not in exps:
        print(f"?? no export {fn}")
        continue
    print(f"\n######## {fn} @ {exps[fn]:#x} ########")
    print("\n".join(disasm_fn(base, secs, exps[fn], 500, imps)))
