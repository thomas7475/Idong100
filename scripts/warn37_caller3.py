#!/usr/bin/env python3
"""Turn 37: find the ShowDeviceIDInvalidWindow call site in xeyez.exe."""
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

EXE = "/home/z/my-project/work/t36/engine/e26/xeyez_engine/Chs/xeyez.exe"
pe = pefile.PE(EXE)
base = pe.OPTIONAL_HEADER.ImageBase
img = pe.get_memory_mapped_image()
md = Cs(CS_ARCH_X86, CS_MODE_32)

iat = {}
for d in pe.DIRECTORY_ENTRY_IMPORT:
    dll = d.dll.decode()
    for imp in d.imports:
        if imp.address and imp.name:
            iat[imp.address] = f"{dll}!{imp.name.decode()}"

# find every reference to the string VAs: 0x8498 (dll name #2), 0x84c8 (function)
for target_rva, label in ((0x8498, "dll name str#2"), (0x84c8, "ShowDeviceIDInvalidWindow"), (0x840c, "ShowNoiceWindow#1")):
    va = base + target_rva
    needle = va.to_bytes(4, "little")
    offs = []
    off = 0
    while True:
        i = img.find(needle, off)
        if i < 0:
            break
        offs.append(i)
        off = i + 1
    print(f"{label} (VA {hex(va)}): imm32 refs at", [hex(o) for o in offs])

# also refs to dll-name str#1 0x83dc handled already; disasm each ref site of 0x84c8/0x8498
sites = []
for target_rva in (0x8498, 0x84c8):
    va = base + target_rva
    needle = va.to_bytes(4, "little")
    off = 0
    while True:
        i = img.find(needle, off)
        if i < 0:
            break
        # find containing instruction: step back up to 8 bytes, find a disasm that lands on i
        for back in range(1, 10):
            lo = i - back
            ok = True
            for ins in md.disasm(img[lo:i + 4], base + lo):
                if ins.address + ins.size == base + i + 4:
                    sites.append((lo, ins, target_rva, back))
                    ok = True
                    break
            else:
                continue
            break
        off = i + 1

for lo, ins, target_rva, back in sites:
    print(f"\n===== ref to RVA {hex(target_rva)} in instruction @ {hex(base + ins.address)}: {ins.mnemonic} {ins.op_str} =====")
    start = ins.address - base - 120
    for x in md.disasm(img[start:ins.address - base + 420], base + start):
        if x.address > base + ins.address + 300:
            break
        ann = ""
        if x.op_str.startswith("dword ptr ["):
            try:
                t = int(x.op_str.split("[")[1].rstrip("]"), 16)
                if t in iat:
                    ann = "  ; " + iat[t]
            except ValueError:
                pass
        if x.op_str.startswith("0x"):
            try:
                v = int(x.op_str, 16)
                if base + 0x8400 <= v < base + 0x8500:
                    rva = v - base
                    s = {0x8498: "xEyeWarrningUI.dll", 0x84c8: "ShowDeviceIDInvalidWindow",
                         0x840c: "ShowNoiceWindow", 0x843c: "ShowNoiceWindow2",
                         0x83dc: "xEyeWarrningUI.dll#1"}.get(rva)
                    if s:
                        ann = f"  ; '{s}'"
            except ValueError:
                pass
        mark = " <<<" if x.address == ins.address else ""
        print(f"  {x.address:08x}  {x.mnemonic:<7} {x.op_str}{ann}{mark}")
