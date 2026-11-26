#!/usr/bin/env python3
"""Turn 37: verify the xEyeWarrningUI patch targets in BOTH builds.

Checks:
1. .text identical between v2.6.0.6 and v2.5.0.7 builds (byte compare).
2. ShowDeviceIDInvalidWindow + ShowChipIDInvalidWindow end with bare `ret`
   (callee does not pop args -> entry `ret` is stack-safe, cdecl).
3. No relocation entry points into the patched byte ranges.
4. File offsets of the two export entries.
"""
import pefile

P26 = "/home/z/my-project/work/t36/engine/e26/xeyez_engine/Chs/xEyeWarrningUI.dll"
P25 = "/home/z/my-project/work/t36/engine/e25/xeyez_pc_v2.5.0.7-inst/Chs/xEyeWarrningUI.dll"

pe26 = pefile.PE(P26)
pe25 = pefile.PE(P25)

t26 = pe26.sections[0]
t25 = pe25.sections[0]
d26 = open(P26, "rb").read()
d25 = open(P25, "rb").read()

print("== .text compare ==")
print(" v26 .text: VA", hex(t26.VirtualAddress), "vsize", hex(t26.Misc_VirtualSize),
      "raw off", hex(t26.PointerToRawData), "raw size", hex(t26.SizeOfRawData))
print(" v25 .text: VA", hex(t25.VirtualAddress), "vsize", hex(t25.Misc_VirtualSize),
      "raw off", hex(t25.PointerToRawData), "raw size", hex(t25.SizeOfRawData))
b26 = d26[t26.PointerToRawData:t26.PointerToRawData + t26.SizeOfRawData]
b25 = d25[t25.PointerToRawData:t25.PointerToRawData + t25.SizeOfRawData]
print(" .text identical:", b26 == b25, f"(v26 {len(b26)} B, v25 {len(b25)} B)")

# find the function ends: disassemble from each export until we hit a ret that
# follows a leave/pop sequence -- use capstone linear scan and stop at the
# FIRST 'ret' mnemonic whose address is the export start of the NEXT function.
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
md = Cs(CS_ARCH_X86, CS_MODE_32)
img = pe26.get_memory_mapped_image()
base = pe26.OPTIONAL_HEADER.ImageBase

EXPORTS = {"ShowDeviceIDInvalidWindow": 0x1F54, "ShowChipIDInvalidWindow": 0x2085}
ENDS = {"ShowDeviceIDInvalidWindow": 0x2084, "ShowChipIDInvalidWindow": 0x2166}
for name, rva in EXPORTS.items():
    end = ENDS[name]
    # the two bytes before the next export = the epilogue
    tail = img[end - 7:end + 1]
    print(f"== {name} @ RVA {hex(rva)} .. {hex(end)} (len {end - rva + 1}) ==")
    print("  last 8 bytes:", tail.hex(" "))
    for ins in md.disasm(tail, base + end - 7):
        print(f"    {ins.address:08x}  {ins.mnemonic:<6} {ins.op_str}")
    # entry bytes
    entry = img[rva:rva + 8]
    print(f"  entry bytes @ RVA {hex(rva)}:", entry.hex(" "))
    print(f"  file offset of entry: {hex(pe26.get_offset_from_rva(rva))}")
    # v25 same?
    img25 = pe25.get_memory_mapped_image()
    print(f"  v25 entry bytes same:", img25[rva:rva + 8] == entry,
          f"(file off {hex(pe25.get_offset_from_rva(rva))})")
    # epilogue check
    code_end = img[end - 16:end + 1]
    rets = [i for i in md.disasm(code_end, base + end - 16) if i.mnemonic == "ret"]
    assert rets, f"no ret found at end of {name}"
    last = rets[-1]
    print(f"  epilogue ret: {last.mnemonic} {last.op_str}  ->",
          "BARE ret (caller cleans args)" if last.op_str == "" else "RET IMM")

print()
print("== relocation entries overlapping the patch ranges ==")
for name, rva in EXPORTS.items():
    hit = False
    for section in pe26.sections:
        if hasattr(section, "DIRECTORY_ENTRY_BASERELOC"):
            for rel in section.DIRECTORY_ENTRY_BASERELOC:
                pass
    # use the global parse
for rel_block in pe26.DIRECTORY_ENTRY_BASERELOC:
    for e in rel_block.entries:
        if e.type == 0:
            continue
        for name, rva in EXPORTS.items():
            if rva <= e.rva < rva + 3:
                print(f"  !! reloc {hex(e.rva)} type {e.type} INSIDE {name}")
                hit = True
print("  no relocation overlaps" if not hit else "  OVERLAP FOUND")
