#!/usr/bin/env python3
"""Analyze lobby.exe + LobbyConn.dll usage of xEyeMsgEngine/xEyeEngine:
what does the lobby check before declaring the sensor connected?"""
import pefile
import capstone
import re
import sys

def imports_map(path):
    pe = pefile.PE(path, fast_load=True)
    pe.parse_data_directories(directories=[
        pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_IMPORT']])
    imps = {}
    for entry in pe.DIRECTORY_ENTRY_IMPORT:
        dll = entry.dll.decode().lower()
        for imp in entry.imports:
            if imp.name:
                imps[imp.address] = (dll, imp.name.decode())
    return pe, imps

def disasm_calls(path, wanted_substrings, ctx=8):
    pe, imps = imports_map(path)
    base = pe.OPTIONAL_HEADER.ImageBase
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
    md.detail = False
    # text section
    for sec in pe.sections:
        if not (sec.Characteristics & 0x20000000):  # exec
            continue
        code = sec.get_data()
        va = base + sec.VirtualAddress
        text_hit_calls = []
        # find call sites to IAT entries of interest
        for addr, (dll, name) in imps.items():
            if any(w.lower() in name.lower() for w in wanted_substrings):
                iat_off = addr
                # search for ff 15 <iat va> (call [addr]) anywhere in image
                pat = bytes.fromhex("ff15") + iat_off.to_bytes(4, 'little')
                start = 0
                while True:
                    j = code.find(pat, start)
                    if j < 0:
                        break
                    text_hit_calls.append((va + j, addr, name))
                    start = j + 1
                pat6 = bytes.fromhex("ff25") + iat_off.to_bytes(4, 'little')  # jmp [iat]
                start = 0
                while True:
                    j = code.find(pat6, start)
                    if j < 0:
                        break
                    text_hit_calls.append((va + j, addr, name + " (jmp)"))
                    start = j + 1
        if text_hit_calls:
            print(f"== {path} section {sec.Name.decode().strip(chr(0))} ==")
            for site, iat, name in text_hit_calls:
                # disasm backwards a bit by scanning from a window start
                lo = max(0, site - va - 14 * ctx)
                lo -= lo % 16
                window = code[lo:site - va + 32]
                lines = list(md.disasm(window, va + lo))
                print(f"\n-- call {name} @ {site:#x} --")
                for ins in lines[-14:]:
                    print(f"   {ins.address:#x}: {ins.mnemonic} {ins.op_str}")
    return

for path in sys.argv[1:]:
    print(f"\n########## {path} ##########")
    disasm_calls(path, ["InitxEyeMsgEngine", "getxEyeEngineExist", "getxEyeVersion",
                        "getVersionResult", "InitxEyeEngine", "IsxEyeEngineExist",
                        "GetLobbyConn", "InitLobbyConn"])
