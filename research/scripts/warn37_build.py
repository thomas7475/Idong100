#!/usr/bin/env python3
"""Turn 37: build + prove the xEyeWarrningUI.dll patch.

Patch (both builds share an identical .text):
  ShowDeviceIDInvalidWindow  @ file offset 0x1F54: 55 8B EC (push ebp;mov ebp,esp)
  ShowChipIDInvalidWindow    @ file offset 0x2085: 55 8B EC
      -> 31 C0 C3   (xor eax,eax; ret)

Why this is safe (all verified this turn):
  - both functions end with a BARE ret (caller cleans args);
    the real caller xeyez.exe does `call proc; add esp, 4` (cdecl, 1 arg).
  - the caller ignores the return value and continues its wndproc normally
    (sets its shown-flag and returns 0 - no exit, no retry loop).
  - no relocation entry overlaps the 6 patched bytes.
  - only the 6 bytes differ; machine/EP/imports/exports unchanged (lief).

Emulation proof (unicorn, section-mapped image, IAT pre-filled with stubs
exactly like a loader does):
  - vendor DLL: ShowDeviceIDInvalidWindow calls CreateDialogParamA (stub
    returns a fake hwnd), walks the whole show path and returns to the
    return guard with EAX = 0 (the success path).
  - patched DLL: no import fires, EAX == 0, RIP == return guard, ESP exactly
    one-arg-above entry (stack perfectly balanced, cdecl).
"""
import os
import struct
import pefile
import lief
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UcError, UC_HOOK_CODE
from unicorn.x86_const import (
    UC_X86_REG_EAX, UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_EIP,
)

SRC = {
    "v26": "/home/z/my-project/work/t36/engine/e26/xeyez_engine/Chs/xEyeWarrningUI.dll",
    "v25": "/home/z/my-project/work/t36/engine/e25/xeyez_pc_v2.5.0.7-inst/Chs/xEyeWarrningUI.dll",
}
OUTDIR = "/home/z/my-project/work/t37/patched"
REFDIR = "/home/z/my-project/work/t37/ref"
PATCHES = {"ShowDeviceIDInvalidWindow": 0x1F54, "ShowChipIDInvalidWindow": 0x2085}
NEWBYTES = bytes.fromhex("31c0c3")
RETGUARD = 0x90f000   # unmapped: emu stops only when the function returns to it

os.makedirs(OUTDIR, exist_ok=True)
os.makedirs(REFDIR, exist_ok=True)


def build(tag):
    src = SRC[tag]
    vendor = open(src, "rb").read()
    pe = pefile.PE(src)
    data = bytearray(vendor)
    for name, rva in PATCHES.items():
        off = pe.get_offset_from_rva(rva)
        old = bytes(data[off:off + 3])
        assert old == bytes.fromhex("558bec"), f"{name}: unexpected bytes {old.hex()} at {hex(off)}"
        data[off:off + 3] = NEWBYTES
    patched = bytes(data)
    diffs = [i for i in range(len(vendor)) if vendor[i] != patched[i]]
    assert len(diffs) == 6, diffs
    assert {diffs[0], diffs[3]} == set(PATCHES.values()), [hex(d) for d in diffs]
    out = os.path.join(OUTDIR, f"xEyeWarrningUI_t37{'' if tag == 'v26' else '_v25'}.dll")
    ref = os.path.join(REFDIR, f"xEyeWarrningUI_vendor_{tag}.dll")
    open(out, "wb").write(patched)
    open(ref, "wb").write(vendor)

    a, b = lief.parse(src), lief.parse(out)
    assert a.header.machine == b.header.machine
    assert a.optional_header.addressof_entrypoint == b.optional_header.addressof_entrypoint

    def imp_sig(l):
        out = []
        for lib in l.imports:
            names = sorted((e.name, e.iat_address) for e in lib.entries)
            out.append((lib.name, tuple(names)))
        return sorted(out)

    assert imp_sig(a) == imp_sig(b)
    assert sorted(e.name for e in a.exported_functions) == sorted(e.name for e in b.exported_functions)
    print(f"[{tag}] {os.path.basename(out)} written; diffs at "
          f"{[hex(d) for d in diffs]}; lief: machine/EP/imports/exports unchanged")
    return vendor, patched


NARGS = {
    "CreateDialogParamA": 5, "GetSystemMetrics": 1, "SendMessageA": 4,
    "GetModuleHandleA": 1, "GetPrivateProfileIntA": 4, "SetWindowPos": 7,
    "UpdateWindow": 1, "OutputDebugStringA": 1, "DestroyWindow": 1,
    "GetWindowRect": 2, "DefWindowProcA": 4,
}
RETVALS = {
    "GetModuleHandleA": 0x400000,
    "CreateDialogParamA": 0x1337,   # non-null dialog hwnd
    "GetSystemMetrics": 1024,
    "GetPrivateProfileIntA": 0,     # != 2 -> centered-placement path
}


def emulate(blob, entry_rva, tag):
    """Run one export; IAT pre-filled with loader-like stubs; return summary."""
    pe = pefile.PE(data=blob)
    base = pe.OPTIONAL_HEADER.ImageBase
    uc = Uc(UC_ARCH_X86, UC_MODE_32)
    uc.mem_map(base, 0x100000)
    for s in pe.sections:
        raw = pe.get_data(s.VirtualAddress, s.Misc_VirtualSize)
        uc.mem_write(base + s.VirtualAddress, raw)

    fired = []
    iat = {}
    for d in pe.DIRECTORY_ENTRY_IMPORT:
        for imp in d.imports:
            if imp.name:
                iat[imp.address] = imp.name.decode()

    # stub region + ret region + fake singleton object
    uc.mem_map(0x909000, 0x8000)         # bare `ret` at 0x909000 (vcall targets)
    uc.mem_write(0x909000, b"\xc3")
    STUB = 0x920000                        # one stub per import
    uc.mem_map(STUB, 0x1000)
    uc.mem_map(0x500000, 0x20000)          # fake engine singleton
    uc.mem_write(0x500000, struct.pack("<I", 0x510000))
    uc.mem_write(0x510008, struct.pack("<I", 0x909000))
    uc.mem_write(0x510028, struct.pack("<I", 0x909000))

    for i, (addr, name) in enumerate(sorted(iat.items())):
        stub_addr = STUB + i * 16
        val = RETVALS.get(name, 0)
        if name.startswith("?getSingleton") or name.startswith("getSingleton"):
            val = 0x500000
        n = NARGS.get(name, 0)
        stub = b"\xb8" + struct.pack("<I", val) + b"\xc2" + struct.pack("<H", n * 4)
        uc.mem_write(stub_addr, stub)
        uc.mem_write(addr, struct.pack("<I", stub_addr))   # fill the IAT

    # trace import calls via code hook
    stub_names = {STUB + i * 16: name for i, (addr, name) in enumerate(sorted(iat.items()))}

    def on_stub(uc, address, size, user_data):
        if address in stub_names:
            fired.append(stub_names[address])

    uc.hook_add(UC_HOOK_CODE, on_stub, begin=STUB, end=STUB + 0x1000)

    # stack + cdecl args
    uc.mem_map(0x210000, 0x20000)
    esp = 0x228000
    uc.mem_write(esp - 4, struct.pack("<I", 0x66))      # the hwnd argument
    uc.mem_write(esp - 8, struct.pack("<I", RETGUARD))   # cdecl return address
    uc.reg_write(UC_X86_REG_ESP, esp - 8)
    uc.reg_write(UC_X86_REG_EBP, 0x22f000)

    try:
        uc.emu_start(base + entry_rva, RETGUARD, timeout=5_000_000)
        status = "reached return guard"
    except UcError as e:
        status = f"stopped: {e} rip={hex(uc.reg_read(UC_X86_REG_EIP))}"
    eax = uc.reg_read(UC_X86_REG_EAX)
    esp_now = uc.reg_read(UC_X86_REG_ESP)
    balanced = esp_now == esp - 4   # bare ret: the arg is still on the stack
    print(f"   {tag}: {status}; EAX=0x{eax:08x}; stack balanced(cdecl): {balanced}")
    print(f"      imports fired: {fired}")
    return eax, fired, balanced


if __name__ == "__main__":
    ok = True
    for tag in ("v26", "v25"):
        vendor, patched = build(tag)
        print(f"--- {tag} emulation: VENDOR ShowDeviceIDInvalidWindow ---")
        eax, fired, bal = emulate(vendor, 0x1F54, "vendor")
        assert "CreateDialogParamA" in fired, "vendor function must create the dialog"
        assert eax == 0 and bal, "vendor function must return 0 (success path), balanced"
        print(f"--- {tag} emulation: PATCHED ShowDeviceIDInvalidWindow ---")
        eax, fired, bal = emulate(patched, 0x1F54, "patched")
        assert eax == 0 and fired == [] and bal, "patched: no dialog, EAX=0, balanced"
        print(f"--- {tag} emulation: PATCHED ShowChipIDInvalidWindow ---")
        eax, fired, bal = emulate(patched, 0x2085, "patched-chip")
        assert eax == 0 and fired == [] and bal, "patched chip variant: same"
    print()
    print("ALL PATCH + EMULATION ASSERTIONS PASSED")
