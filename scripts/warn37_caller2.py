#!/usr/bin/env python3
"""Turn 37: xeyez.exe loads xEyeWarrningUI.dll dynamically - find the
LoadLibraryA site and the flow around the ShowDeviceIDInvalidWindow call."""
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

# find the string VAs
def find_str(s):
    out = []
    b = s.encode() + b"\x00"
    off = 0
    while True:
        i = img.find(b, off)
        if i < 0:
            return out
        out.append(i)
        off = i + 1

for s in ("xEyeWarrningUI.dll", "ShowDeviceIDInvalidWindow", "ShowChipIDInvalidWindow", "ShowNoiceWindow"):
    print(f"string {s!r} at RVA(s):", [hex(x) for x in find_str(s)])

dll_rvas = find_str("xEyeWarrningUI.dll")
fn_rvas = find_str("ShowDeviceIDInvalidWindow")

# search code for: push imm32 (dll string VA) followed nearby by call LoadLibraryA
# opcode 68 imm32 where imm = base + rva
hits = []
for rva in dll_rvas:
    needle = b"\x68" + (base + rva).to_bytes(4, "little")
    off = 0
    while True:
        i = img.find(needle, off)
        if i < 0:
            break
        hits.append(i)
        off = i + 1
print("push <dllstring> sites:", [hex(h) for h in hits])

def annotate(ins):
    ann = ""
    if ins.op_str.startswith("dword ptr ["):
        try:
            t = int(ins.op_str.split("[")[1].rstrip("]"), 16)
            if t in iat:
                ann = "  ; " + iat[t]
        except ValueError:
            pass
    if ins.mnemonic == "push" and ins.op_str.startswith("0x"):
        try:
            v = int(ins.op_str, 16) - base
            for rva in dll_rvas + fn_rvas:
                if v == rva:
                    ann = f"  ; '{[s for s in ('xEyeWarrningUI.dll','ShowDeviceIDInvalidWindow') if img.find(s.encode()+b'\\x00'.decode('latin1').encode('latin1'))==rva][0] if False else ''}' RVA"
                    ann = f"  ; string RVA {hex(rva)}"
        except ValueError:
            pass
    return ann

for h in hits:
    print(f"\n===== LoadLibrary site @ RVA {hex(h)} (VA {hex(base+h)}) =====")
    lo = h - 128
    for ins in md.disasm(img[lo:h + 400], base + lo):
        if ins.address > base + h + 320:
            break
        a = annotate(ins)
        mark = " <<<" if ins.address == base + h else ""
        print(f"  {ins.address:08x}  {ins.mnemonic:<7} {ins.op_str}{a}{mark}")
