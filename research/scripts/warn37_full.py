#!/usr/bin/env python3
"""Turn 37: full disassembly of ShowDeviceIDInvalidWindow + its callers' IAT usage."""
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

P26 = "/home/z/my-project/work/t36/engine/e26/xeyez_engine/Chs/xEyeWarrningUI.dll"
pe = pefile.PE(P26)
base = pe.OPTIONAL_HEADER.ImageBase
data = pe.get_memory_mapped_image()
md = Cs(CS_ARCH_X86, CS_MODE_32)

# IAT names for call [imm] resolution
iat = {}
if hasattr(pe, "DIRECTORY_ENTRY_IMPORT"):
    for d in pe.DIRECTORY_ENTRY_IMPORT:
        dll = d.dll.decode()
        for imp in d.imports:
            if imp.address:
                nm = imp.name.decode() if imp.name else f"ord{imp.ordinal}"
                iat[imp.address] = f"{dll}!{nm}"

print("== full ShowDeviceIDInvalidWindow (RVA 0x1f54 .. 0x2090) ==")
start_rva = 0x1f54
code = data[start_rva:start_rva + (0x2090 - start_rva)]
for ins in md.disasm(code, base + start_rva):
    op = ins.op_str
    ann = ""
    if ins.mnemonic == "call" and op.startswith("dword ptr ["):
        try:
            target = int(op.split("[")[1].rstrip("]"), 16)
            if target in iat:
                ann = "   ; " + iat[target]
        except ValueError:
            pass
    print(f"  {ins.address:08x}  {ins.mnemonic:<8} {op}{ann}")

print()
print("== imports ==")
for d in pe.DIRECTORY_ENTRY_IMPORT:
    print(" ", d.dll.decode())
    for imp in d.imports:
        if imp.name:
            print("    ", hex(imp.address), imp.name.decode())
