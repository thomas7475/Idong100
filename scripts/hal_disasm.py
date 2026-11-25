#!/usr/bin/env python3
"""hal_disasm.py — disassemble xeyezHal.dll exports and extract the USB command
protocol (packet builders around the 'PU\\x7f\\x7f' magic 0x7f7f5550 and
response magic 'OD\\x7f\\x7f' 0x7f7f444f), string references, and call structure.

Output: analysis/xeyezHal_protocol_notes.txt
"""
import sys
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

DLL = sys.argv[1] if len(sys.argv) > 1 else \
    '/home/z/my-project/analysis/nsis/v2.6.0.6/xeyez_alone/$_6_/xeyezHal.dll'
OUT = sys.argv[2] if len(sys.argv) > 2 else \
    '/home/z/my-project/analysis/xeyezHal_protocol_notes.txt'

data = open(DLL, 'rb').read()
pe = pefile.PE(data=data, fast_load=True)
pe.parse_data_directories(directories=[
    pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_EXPORT'],
    pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_IMPORT'],
])
image_base = pe.OPTIONAL_HEADER.ImageBase
secs = [(s.VirtualAddress, s.VirtualAddress + s.Misc_VirtualSize, s.Name.decode().strip('\0'))
        for s in pe.sections]


def rva2off(rva):
    for va, vsz, _ in [(s.VirtualAddress, s.Misc_VirtualSize, 0) for s in pe.sections]:
        pass
    for s in pe.sections:
        if s.VirtualAddress <= rva < s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData):
            return s.PointerToRawData + (rva - s.VirtualAddress)
    return None


def sec_of(rva):
    for va, vend, name in secs:
        if va <= rva < vend:
            return name
    return '?'


# string table: map file offset of any C string in .rdata/.data
str_by_off = {}
for s in pe.sections:
    if s.Name.decode().strip('\0') in ('.rdata', '.data'):
        blob = data[s.PointerToRawData:s.PointerToRawData + s.SizeOfRawData]
        i = 0
        while i < len(blob):
            j = i
            while j < len(blob) and 0x20 <= blob[j] < 0x7f:
                j += 1
            if j - i >= 4 and j < len(blob) and blob[j] == 0:
                str_by_off[s.PointerToRawData + i] = blob[i:j].decode('ascii', 'replace')
                i = j + 1
            else:
                i = j + 1 if j > i else i + 1

# imports: IAT entries -> name
iat = {}
for entry in getattr(pe, 'DIRECTORY_ENTRY_IMPORT', []):
    dll = entry.dll.decode()
    for imp in entry.imports:
        if imp.name:
            iat[imp.address - image_base] = f'{dll}!{imp.name.decode()}'

exports = {}
for e in pe.DIRECTORY_ENTRY_EXPORT.symbols:
    if e.name:
        exports[e.address] = e.name.decode()
exp_by_name = {v: k for k, v in exports.items()}

md = Cs(CS_ARCH_X86, CS_MODE_32)
md.detail = False

FUNCS_OF_INTEREST = None  # all exports

lines = []
lines.append(f'# xeyezHal.dll protocol disassembly — {DLL}')
lines.append(f'# image base {image_base:#x}, {len(exports)} exports, IAT entries {len(iat)}')
lines.append('')

MAGIC_TX = 0x7f7f5550   # 'PU\x7f\x7f'
MAGIC_RX = 0x7f7f444f   # 'OD\x7f\x7f'

for rva in sorted(exports):
    name = exports[rva]
    off = rva2off(rva)
    if off is None:
        continue
    code = data[off:off + 1400]
    lines.append('=' * 78)
    lines.append(f'### {name}  RVA {rva:#x} ({sec_of(rva)})')
    # quick scan first: does it touch magic or IAT?
    txt_start = len(lines)
    n_instr = 0
    hit_magic = False
    wrote = []
    for ins in md.disasm(code, image_base + rva):
        n_instr += 1
        if n_instr > 260:
            break
        op = ins.op_str
        ann = ''
        # IAT call/jmp
        if ('call' in ins.mnemonic or 'jmp' in ins.mnemonic) and op.startswith('dword ptr ['):
            try:
                tgt = int(op.split('[')[1].rstrip(']'), 16) - image_base
                if tgt in iat:
                    ann = f'  ; {iat[tgt]}'
            except Exception:
                pass
        # direct call into export or internal
        elif ins.mnemonic == 'call':
            try:
                tgt = int(op, 16)
                trva = tgt - image_base
                if trva in exports:
                    ann = f'  ; -> {exports[trva]}'
            except Exception:
                pass
        # immediates of interest
        for imm_name, imm_val in (('MAGIC_TX', MAGIC_TX), ('MAGIC_RX', MAGIC_RX)):
            if f'{imm_val:#x}' in op:
                ann += f'  ; <<< {imm_name} packet builder'
                hit_magic = True
        # mov word ptr [x+4], CMD  → capture the cmd id heuristically
        if ins.mnemonic == 'mov' and op.startswith('word ptr [') and ', 0x' in op:
            try:
                val = int(op.split(', 0x')[1], 16)
                if val >= 1 and val <= 0x20 and hit_magic:
                    ann += f'  ; cmd id {val} at +4'
            except Exception:
                pass
        # string refs: push imm = address in .rdata/.data
        if ins.mnemonic == 'push' and op.startswith('0x'):
            try:
                a = int(op, 16)
                off2 = rva2off(a - image_base)
                if off2 is not None and off2 in str_by_off:
                    ann += f'  ; "{str_by_off[off2]}"'
            except Exception:
                pass
        wrote.append(f'  {ins.address:#010x}  {ins.mnemonic:8s} {op}{ann}')
        if ins.mnemonic == 'ret':
            wrote.append('  ---')
    lines.extend(wrote)
    lines.append('')

open(OUT, 'w').write('\n'.join(lines))
print(f'wrote {OUT}: {len(lines)} lines')
