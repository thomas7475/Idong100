#!/usr/bin/env python3
"""Fetch the first N bytes of a given .bin slice member from the i-dong installer
zip on download.i-dong.com via ranged GET, then extract selected files from it
using inno_bin_extract.py machinery.

Usage: python3 fetch_slice_extract.py <slice-stem e.g. idong_setup-2.bin> <prefix_bytes> [file names...]
"""
import hashlib
import os
import struct
import sys
import urllib.request
import zlib

sys.path.insert(0, '/home/z/my-project/scripts')
from inno_bin_extract import SetupInfo, parse_loader_offsets, parse_setup_header_block, \
    SliceSource, read_chunk

ZIP_URL = 'http://download.i-dong.com/setup/yundongwang-ver/idong_1.9.04_windows_yundongwang_A_setup.zip'
SETUP = '/home/z/my-project/analysis/live_test/idong_setup.exe'
OUTDIR = '/home/z/my-project/analysis/live_test'
EXTRACT_DIR = '/home/z/my-project/analysis/capstill'


def ranged_get(url, start, end):
    req = urllib.request.Request(url, headers={'Range': f'bytes={start}-{end}'})
    with urllib.request.urlopen(req, timeout=90) as r:
        return r.read()


def list_members():
    req = urllib.request.Request(ZIP_URL, method='HEAD')
    with urllib.request.urlopen(req, timeout=60) as r:
        total = int(r.headers['Content-Length'])
    tail = ranged_get(ZIP_URL, total - 65536, total - 1)
    i = tail.rfind(b'PK\x05\x06')
    eocd = tail[i:i + 22]
    _, _, _, _, n_entries, cd_size, cd_off, _ = struct.unpack('<IHHHHIIH', eocd)
    if cd_off == 0xFFFFFFFF or n_entries == 0xFFFF:
        j = tail.rfind(b'PK\x06\x07', 0, i)
        loc = tail[j:j + 20]
        z64_eocd_off = struct.unpack('<IIQ', loc[4:24 - 4])[0]
        z64 = ranged_get(ZIP_URL, z64_eocd_off, z64_eocd_off + 56 - 1)
        n_entries = struct.unpack('<Q', z64[32:40])[0]
        cd_size = struct.unpack('<Q', z64[40:48])[0]
        cd_off = struct.unpack('<Q', z64[48:56])[0]
    cd = ranged_get(ZIP_URL, cd_off, cd_off + cd_size - 1)
    members = []
    p = 0
    while p + 46 <= len(cd):
        sig, ver_m, ver_n, flags, method, mtime, mdate, crc, csize, usize, nlen, elen, clen, disk, iattr, eattr, lho = \
            struct.unpack('<IHHHHHHIIIHHHHHII', cd[p:p + 46])
        name = cd[p + 46:p + 46 + nlen].decode('utf-8', 'replace')
        members.append((name, method, csize, usize, lho, crc))
        p += 46 + nlen + elen + clen
    return members


def fetch_slice_prefix(name, prefix_bytes):
    members = list_members()
    m = [x for x in members if x[0] == name]
    if not m:
        avail = [x[0] for x in members if x[0].lower().endswith('.bin')]
        raise SystemExit(f'{name} not in zip. Available bins: {avail[:10]} ... ({len(avail)})')
    _, method, csize, usize, lho, _ = m[0]
    lh = ranged_get(ZIP_URL, lho, lho + 29)
    sig, _, flags2, method2, _, _, crc2, csize2, usize2, nlen2, elen2 = \
        struct.unpack('<IHHHHHIIIHH', lh[:30])
    data_off = lho + 30 + nlen2 + elen2
    print(f'{name}: method={method2} csize={csize2:,} usize={usize:,} data@{data_off:#x}')
    if method2 == 0:
        # stored: direct mapping, but slice itself may be compressed inside the archive
        prefix = ranged_get(ZIP_URL, data_off, data_off + prefix_bytes - 1)
    else:
        comp = ranged_get(ZIP_URL, data_off, data_off + int(csize2) - 1)  # whole member if feasible
        if len(comp) > 4_000_000:
            comp = ranged_get(ZIP_URL, data_off, data_off + 4_000_000 - 1)
        d = zlib.decompressobj(-15)
        prefix = d.decompress(comp, prefix_bytes)
        print(f'  inflated {len(prefix):,} bytes from {len(comp):,} compressed')
    outp = os.path.join(OUTDIR, os.path.basename(name))
    open(outp, 'wb').write(prefix)
    print(f'saved {len(prefix):,} bytes -> {outp}')
    return outp


def extract_from_slices(want_bases):
    exe_data = open(SETUP, 'rb').read()
    off = parse_loader_offsets(exe_data)
    vstr, ver, header_data, b1 = parse_setup_header_block(exe_data, off['header_offset'])
    info = SetupInfo(ver, header_data, quirk_byte=True)
    _, _, second_data, _ = parse_setup_header_block(exe_data, off['header_offset'] + 64 + 9 + b1, second=True)
    info.parse_data_entries(second_data)
    src = SliceSource(SETUP, off['data_offset'], '', info.h.get('slices_per_disk', 1), 'idong_setup')
    os.makedirs(EXTRACT_DIR, exist_ok=True)
    got = []
    for i, f in enumerate(info.files):
        base = f['destination'].replace(b'\\', b'/').split(b'/')[-1]
        if base not in want_bases:
            continue
        loc = f['location']
        if loc >= len(info.data_entries):
            continue
        d = info.data_entries[loc]
        try:
            chunk = read_chunk(src, d, info.h.get('compression', 1))
            filedata = chunk[d['file_offset']:d['file_offset'] + d['file_size']]
            md5 = hashlib.md5(filedata).hexdigest()
            exp = d.get('md5', b'').hex() if d.get('md5') else '?'
            status = 'MD5 OK' if md5 == exp else f'MISMATCH exp={exp}'
            outp = os.path.join(EXTRACT_DIR, base.decode())
            open(outp, 'wb').write(filedata)
            print(f'  {base.decode()} {d["file_size"]:,} B slice={d["slice"]} @{d["offset"]:#x} -> {status}')
            if md5 == exp:
                got.append(outp)
        except Exception as e:
            print(f'  {base.decode()}: {e}')
    return got


if __name__ == '__main__':
    slice_name = sys.argv[1] if len(sys.argv) > 1 else 'idong_setup-2.bin'
    prefix = int(sys.argv[2]) if len(sys.argv) > 2 else 2_000_000
    wants = [w.encode() for w in sys.argv[3:]] or [b'CapStill.dll', b'CapStill.exe']
    fetch_slice_prefix(slice_name, prefix)
    extract_from_slices(wants)
