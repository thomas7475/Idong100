#!/usr/bin/env python3
"""Turn 37: reverse xEyeWarrningUI.dll - exports + ShowDeviceIDInvalidWindow."""
import sys
import pefile
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

PATHS = {
    "v26": "/home/z/my-project/work/t36/engine/e26/xeyez_engine/Chs/xEyeWarrningUI.dll",
    "v25": "/home/z/my-project/work/t36/engine/e25/xeyez_pc_v2.5.0.7-inst/Chs/xEyeWarrningUI.dll",
}

for tag, path in PATHS.items():
    print("=" * 70)
    print(f"{tag}: {path}")
    pe = pefile.PE(path)
    image_base = pe.OPTIONAL_HEADER.ImageBase
    print(f"  machine={hex(pe.FILE_HEADER.Machine)} base={hex(image_base)} "
          f"ep={hex(image_base + pe.OPTIONAL_HEADER.AddressOfEntryPoint)}")
    print("  -- exports --")
    exp = {}
    if hasattr(pe, "DIRECTORY_ENTRY_EXPORT"):
        for e in pe.DIRECTORY_ENTRY_EXPORT.symbols:
            if e.name:
                rva = e.address
                exp[rva] = e.name.decode()
                print(f"    {hex(rva):>10} RVA  {hex(image_base + rva):>10} VA  {e.name.decode()}")
    else:
        print("    none")
    # sections
    print("  -- sections --")
    for s in pe.sections:
        print(f"    {s.Name.decode().strip(chr(0)):10} VA {hex(image_base + s.VirtualAddress):>10} "
              f"vsize {hex(s.Misc_VirtualSize):>8} rawsize {hex(s.SizeOfRawData):>8}")

    # disassemble each export, first 96 bytes + find the real epilogue (scan up to 0x2000)
    data = pe.get_memory_mapped_image()
    md = Cs(CS_ARCH_X86, CS_MODE_32)
    for rva, name in sorted(exp.items()):
        print(f"  ---- {name} @ RVA {hex(rva)} ----")
        code = data[rva:rva + 160]
        for ins in list(md.disasm(code, image_base + rva))[:22]:
            print(f"    {ins.address:08x}  {ins.mnemonic:<8} {ins.op_str}")
        # scan forward for the first ret / ret imm in the same section (rough end detection)
        scan = data[rva:rva + 0x2000]
        cnt = 0
        for ins in md.disasm(scan, image_base + rva):
            if ins.mnemonic == "ret":
                print(f"    FIRST ret @ {hex(ins.address - image_base)}: {ins.mnemonic} {ins.op_str}")
                break
            cnt += 1
            if cnt > 4000:
                break
        print()
