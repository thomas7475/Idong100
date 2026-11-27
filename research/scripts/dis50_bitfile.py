#!/usr/bin/env python3
"""dis50_bitfile.py - turn-50: trace the `BitFile` = A_a_v4.4_BG0313.bit
consumer in xEyeUSB.dll v2.6.0.6.

Steps: map the string file offsets to RVAs, dump the .rdata string
context, locate every push/mov immediate reference from .text, and
disassemble each referencing function body (up to the entry prologue
boundary heuristic: walk back to the nearest push ebp/ret cluster).
"""
import re
import sys
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

DLL = "/tmp/vendor/lobby/$_6_/xEyeUSB.dll"
TARGET_OFFS = {
    "BitFile#1": 0x15B38,
    "BitFile#2": 0x15F74,
    "ens_code.ldr": 0x18798,
    "fpga": 0x1B2F2,
}
CTX = 0x300  # bytes of string-table context around each target

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
            if imp.name:
                iat[imp.address] = f"{entry.dll.decode()}!{imp.name.decode()}"

secs = []
for s in pe.sections:
    secs.append((s.VirtualAddress,
                 s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData),
                 s.PointerToRawData, s.get_data()))

def off2rva(off):
    for va, end, raw, d in secs:
        if raw <= off < raw + len(d):
            return va + (off - raw)
    return None

def rva2off(rva):
    for va, end, raw, d in secs:
        if va <= rva < end:
            o = rva - va
            if o < len(d):
                return raw + o
    return None

def sec_of(rva):
    for i, (va, end, raw, d) in enumerate(secs):
        if va <= rva < end:
            return i, pe.sections[i].Name.decode().rstrip('\x00')
    return None, "?"

def dump_ctx(label, off):
    rva = off2rva(off)
    si, sname = sec_of(rva)
    lo, hi = max(0, off - CTX), min(len(data), off + CTX)
    print(f"--- {label}: file {off:#x} rva {rva:#x} (sec {sname}) strings in +/-{CTX:#x}:")
    blob = data[lo:hi]
    for m in re.finditer(rb"[\x20-\x7e]{4,}", blob):
        s = m.group().decode()
        po = lo + m.start()
        mark = " <<<" if abs(po - off) < 0x40 else ""
        print(f"  {off2rva(po):#08x}  {s!r}{mark}")
    # GBK strings
    for m in re.finditer(rb"(?:[\x81-\xfe][\x40-\xfe]){2,}", blob):
        try:
            s = m.group().decode("gbk")
        except Exception:
            continue
        po = lo + m.start()
        print(f"  {off2rva(po):#08x}  GBK {s!r}")

def find_xrefs(rva):
    """scan .text for push imm32 / mov r,imm32 of this VA."""
    va = base + rva
    pat = re.compile(re.escape(va.to_bytes(4, "little")))
    tsec = secs[0]
    if pe.sections[0].Name.decode().rstrip('\x00') != ".text":
        # find .text
        for i, s in enumerate(pe.sections):
            if s.Name.decode().rstrip('\x00') == ".text":
                tsec = secs[i]
                break
    tva, tend, traw, td = tsec
    hits = []
    for m in pat.finditer(td):
        fo = traw + m.start()
        ro = off2rva(fo)
        # instruction-start guess: the byte before imm should be 0x68 (push) or 0xB8-0xBF/C7
        prev = td[m.start() - 1] if m.start() else 0
        kind = {0x68: "push", 0xB8: "mov eax", 0xB9: "mov ecx", 0xBA: "mov edx",
                0xBE: "mov esi", 0xBF: "mov edi"}.get(prev, f"op{prev:02x}")
        hits.append((ro, kind))
    return hits

def disasm_fn(start_rva, label, max_instrs=1600):
    for va, end, raw, d in secs:
        if va <= start_rva < end:
            code = d[start_rva - va:]
            break
    md = Cs(CS_ARCH_X86, CS_MODE_32)
    lines = []
    n = 0
    for ins in md.disasm(code, base + start_rva):
        line = f"{ins.address - base:08x}  {ins.bytes.hex():<18} {ins.mnemonic} {ins.op_str}"
        ann = ""
        if ins.mnemonic in ('call', 'jmp') and ins.op_str.startswith('0x'):
            t = int(ins.op_str, 16) - base
            if t in exps:
                ann = f"  ; {exps[t]}"
            elif t not in iat:
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
                o = rva2off(v - base) if v > base else None
                if o:
                    raw = data[o:o + 64]
                    s = raw.split(b'\x00')[0]
                    if 3 < len(s) and all(32 <= b < 127 for b in s):
                        ann += f'  ; "{s.decode()}"'
            except Exception:
                pass
        lines.append(line + ann)
        n += 1
        if n >= max_instrs:
            lines.append("... cut")
            break
    print(f"### {label} @ RVA {start_rva:#x}")
    print("\n".join(lines))
    print()

def main():
    for label, off in TARGET_OFFS.items():
        dump_ctx(label, off)
        rva = off2rva(off)
        for ro, kind in find_xrefs(rva):
            print(f"  xref: {kind} at RVA {ro:#x}")
        print()
    return 0

if __name__ == "__main__":
    # mode: "ctx" default; "dis <rva> <label>" disassembles
    if len(sys.argv) > 1 and sys.argv[1] == "dis":
        disasm_fn(int(sys.argv[2], 16), sys.argv[3] if len(sys.argv) > 3 else "fn")
    else:
        main()
