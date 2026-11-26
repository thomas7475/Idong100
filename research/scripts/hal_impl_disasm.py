#!/usr/bin/env python3
"""hal_impl_disasm.py — disassemble specific internal functions of xeyezHal.dll
(the gen-2 device class methods) and annotate magic writes, IAT calls, cmd ids.
"""
import os as _os
_HERE = _os.path.dirname(_os.path.abspath(__file__))  # research/scripts
# research/analysis; override with IDONG_ANALYSIS when the extracted
# installer payloads live somewhere else
ANALYSIS = (_os.environ.get('IDONG_ANALYSIS')
            or _os.path.join(_os.path.dirname(_HERE), 'analysis'))
REPO = _os.path.dirname(_os.path.dirname(_HERE))  # the repository root
import sys
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

DLL = ANALYSIS + '/nsis/v2.6.0.6/xeyez_alone/$_6_/xeyezHal.dll'
data = open(DLL, 'rb').read()
pe = pefile.PE(data=data, fast_load=True)
pe.parse_data_directories(directories=[
    pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_EXPORT'],
    pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_IMPORT'],
])
image_base = pe.OPTIONAL_HEADER.ImageBase


def rva2off(rva):
    for s in pe.sections:
        if s.VirtualAddress <= rva < s.VirtualAddress + max(s.Misc_VirtualSize, s.SizeOfRawData):
            return s.PointerToRawData + (rva - s.VirtualAddress)
    return None


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

iat = {}
for entry in getattr(pe, 'DIRECTORY_ENTRY_IMPORT', []):
    dll = entry.dll.decode()
    for imp in entry.imports:
        if imp.name:
            iat[imp.address - image_base] = f'{dll}!{imp.name.decode()}'

exports = {e.address: e.name.decode() for e in pe.DIRECTORY_ENTRY_EXPORT.symbols if e.name}

md = Cs(CS_ARCH_X86, CS_MODE_32)

MAGICS = {0x7f7f5550: "MAGIC_TX 'PU\\x7f\\x7f'", 0x7f7f444f: "MAGIC_RX 'OD\\x7f\\x7f'"}


def dis(va, max_instr=170, label=''):
    rva = va - image_base
    off = rva2off(rva)
    print('=' * 78)
    print(f'### {label or hex(va)}  VA {va:#x}')
    code = data[off:off + max_instr * 8]
    n = 0
    for ins in md.disasm(code, va):
        n += 1
        if n > max_instr:
            break
        op = ins.op_str
        ann = ''
        if ('call' in ins.mnemonic or ins.mnemonic == 'jmp') and 'dword ptr [' in op:
            try:
                tgt = int(op.split('[')[1].rstrip(']'), 16) - image_base
                if tgt in iat:
                    ann = f'  ; {iat[tgt]}'
            except Exception:
                pass
        elif ins.mnemonic in ('call', 'jmp') and op.startswith('0x'):
            try:
                t = int(op, 16)
                if t - image_base in exports:
                    ann = f'  ; -> {exports[t - image_base]}'
            except Exception:
                pass
        for val, nm in MAGICS.items():
            if f'{val:#x}' in op:
                ann += f'  ; <<<< {nm}'
        if ins.mnemonic == 'mov' and 'word ptr [' in op and ', 0x' in op:
            try:
                v = int(op.split(', 0x')[1], 16)
                if 0 < v <= 0x30:
                    ann += f'  ; CMD={v}'
            except Exception:
                pass
        if ins.mnemonic == 'push' and op.startswith('0x'):
            try:
                a = int(op, 16)
                o2 = rva2off(a - image_base)
                if o2 is not None and o2 in str_by_off:
                    ann += f'  ; "{str_by_off[o2]}"'
            except Exception:
                pass
        print(f'  {ins.address:#010x}  {ins.mnemonic:7s} {op}{ann}')
        if ins.mnemonic == 'ret':
            print('  ---')


# gen-2 impl functions discovered from the export wrappers
FUNCS = [
    (0x100039b0, 'SetTransMode impl (gen2)'),
    (0x10007bd0, 'GetImageBegin impl (gen2)'),
    (0x10007b70, 'GetImageData impl (gen2)'),
    (0x100070a0, 'GetGlobalImageSize impl (gen2)'),
    (0x10009f90, 'SetLed_Status impl (gen2)'),
    (0x10003950, 'SetExpMode impl (gen2)'),
    (0x100033c0, 'SetClassifier impl (gen2)'),
]
for va, lab in FUNCS:
    dis(va, 150, lab)
    print()
