#!/usr/bin/env python3
"""Turn 37: find the call sites of ShowDeviceIDInvalidWindow in xeyez.exe
(the engine host) - what happens AFTER the box would be suppressed."""
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

EXE = "/home/z/my-project/work/t36/engine/e26/xeyez_engine/Chs/xeyez.exe"
pe = pefile.PE(EXE)
base = pe.OPTIONAL_HEADER.ImageBase
img = pe.get_memory_mapped_image()
md = Cs(CS_ARCH_X86, CS_MODE_32)

print("== xeyez.exe imports from xEyeWarrningUI.dll ==")
thunks = {}
for d in pe.DIRECTORY_ENTRY_IMPORT:
    dll = d.dll.decode().lower()
    if "warrning" in dll:
        print(" ", dll)
        for imp in d.imports:
            nm = imp.name.decode() if imp.name else f"ord{imp.ordinal}"
            print(f"    {hex(imp.address)} (VA)  RVA {hex(imp.address - base)}  {nm}")
            thunks[imp.address] = nm

# IAT names for annotation
iat = {}
for d in pe.DIRECTORY_ENTRY_IMPORT:
    dll = d.dll.decode()
    for imp in d.imports:
        if imp.address and imp.name:
            iat[imp.address] = f"{dll}!{imp.name.decode()}"

# scan .text for `call dword ptr [thunk]` / `jmp dword ptr [thunk]`
text = None
for s in pe.sections:
    if s.Name.decode().strip(chr(0)) == ".text":
        text = s
print()
print("== call sites ==")
code = img
sites = []
for thunk in thunks:
    needle = b"\xff\x15" + thunk.to_bytes(4, "little")  # call [thunk]
    needle2 = b"\xff\x25" + thunk.to_bytes(4, "little")  # jmp [thunk]
    start = 0
    while True:
        i = code.find(needle, start)
        if i < 0:
            break
        sites.append((i, "call", thunk))
        start = i + 1
    start = 0
    while True:
        i = code.find(needle2, start)
        if i < 0:
            break
        sites.append((i, "jmp", thunk))
        start = i + 1

for off, kind, thunk in sorted(sites):
    rva = off  # memory-mapped image index == RVA
    print(f"-- {kind} [{thunks[thunk]}] @ RVA {hex(rva)} (VA {hex(base + rva)}) --")
    lo = max(0, rva - 96)
    lo = lo & ~1
    for ins in md.disasm(img[lo:rva + 80], base + lo):
        if ins.address > base + rva + 64:
            break
        mark = "  <<<" if base + rva <= ins.address < base + rva + 6 else ""
        ann = ""
        if ins.mnemonic in ("call", "jmp") and ins.op_str.startswith("dword ptr ["):
            try:
                t = int(ins.op_str.split("[")[1].rstrip("]"), 16)
                if t in iat:
                    ann = "  ; " + iat[t]
            except ValueError:
                pass
        print(f"  {ins.address:08x}  {ins.mnemonic:<7} {ins.op_str}{ann}{mark}")
    print()
