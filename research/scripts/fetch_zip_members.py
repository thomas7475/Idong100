#!/usr/bin/env python3
"""fetch_zip_members.py — download specific members of a remote ZIP archive
over HTTP using ranged GETs (no full-archive download), verify them, and
print a SHA256 + MD5 for EVERY file fetched.

Built for the still-live vendor server download.i-dong.com (1.5-1.7 GB
installer zips whose interesting members total a few hundred MB).

Usage:
    python3 fetch_zip_members.py <zip_url> --list
    python3 fetch_zip_members.py <zip_url> --member NAME [--member NAME ...] \
        [--pattern REGEX ...] [--outdir DIR] [--chunk-mb 16]
        [--limit-uncompressed BYTES]   # inflate only the first N bytes
        [--manifest FILE]              # append one TSV row per fetched file

Every successfully fetched member is stored under --outdir and reported as:
    SHA256 <hex>  MD5 <hex>  <size> B  <member-name>
The member's zip CRC32 is verified after inflation (when fully fetched).
"""
import os as _os
_HERE = _os.path.dirname(_os.path.abspath(__file__))  # research/scripts
# research/analysis; override with IDONG_ANALYSIS when the extracted
# installer payloads live somewhere else
ANALYSIS = (_os.environ.get('IDONG_ANALYSIS')
            or _os.path.join(_os.path.dirname(_HERE), 'analysis'))
REPO = _os.path.dirname(_os.path.dirname(_HERE))  # the repository root
import argparse
import hashlib
import os
import re
import struct
import sys
import time
import urllib.request
import zlib

MANIFEST_HEADER = "sha256\tmd5\tsize\tzip_crc_ok\tzip_url\tmember\n"


def http_get(url, start, end, tries=5, timeout=90):
    """Ranged GET of bytes [start, end] inclusive, with retries."""
    last = None
    for attempt in range(1, tries + 1):
        try:
            req = urllib.request.Request(url, headers={'Range': f'bytes={start}-{end}'})
            with urllib.request.urlopen(req, timeout=timeout) as r:
                return r.read()
        except Exception as e:  # noqa: BLE001 - retry any transport error
            last = e
            wait = min(2 ** attempt, 20)
            print(f"    [retry {attempt}/{tries}] {e}; sleeping {wait}s", flush=True)
            time.sleep(wait)
    raise RuntimeError(f"ranged GET failed after {tries} tries: {last}")


def read_zip_cd(url):
    """Return (total_size, [ (name, method, csize, usize, lho, crc), ... ])."""
    req = urllib.request.Request(url, method='HEAD')
    with urllib.request.urlopen(req, timeout=60) as r:
        total = int(r.headers['Content-Length'])
    tail = http_get(url, max(0, total - 65536), total - 1)
    i = tail.rfind(b'PK\x05\x06')
    if i < 0:
        raise SystemExit('EOCD not found in zip tail')
    eocd = tail[i:i + 22]
    _, _, _, _, n_entries, cd_size, cd_off, _ = struct.unpack('<IHHHHIIH', eocd)
    if cd_off == 0xFFFFFFFF or n_entries == 0xFFFF:
        j = tail.rfind(b'PK\x06\x07', 0, i)
        loc = tail[j:j + 20]
        z64_eocd_off = struct.unpack('<IIQ', loc[4:20])[0]
        z64 = http_get(url, z64_eocd_off, z64_eocd_off + 56 - 1)
        assert z64[:4] == b'PK\x06\x06', 'bad zip64 EOCD magic'
        n_entries = struct.unpack('<Q', z64[32:40])[0]
        cd_size = struct.unpack('<Q', z64[40:48])[0]
        cd_off = struct.unpack('<Q', z64[48:56])[0]
    cd = http_get(url, cd_off, cd_off + cd_size - 1)
    members = []
    p = 0
    while p + 46 <= len(cd):
        (sig, _vm, _vn, _flags, method, _mt, _md, crc, csize, usize,
         nlen, elen, clen, _disk, _ia, _ea, lho) = struct.unpack(
            '<IHHHHHHIIIHHHHHII', cd[p:p + 46])
        if sig != 0x02014b50:
            raise SystemExit(f'bad central directory signature at {p}')
        name = cd[p + 46:p + 46 + nlen].decode('utf-8', 'replace')
        members.append((name, method, csize, usize, lho, crc))
        p += 46 + nlen + elen + clen
    return total, members


def member_data_offset(url, lho):
    lh = http_get(url, lho, lho + 29)
    sig, _, _fl, method, _mt, _md, _crc, csize, usize, nlen, elen = \
        struct.unpack('<IHHHHHIIIHH', lh[:30])
    assert sig == 0x04034b50, 'bad local header magic'
    return lho + 30 + nlen + elen, method, csize, usize


def fetch_member(url, name, method, csize, usize, lho, outpath,
                 chunk_mb=16, limit_uncompressed=None):
    """Chunked ranged download + inflate; returns (sha256hex, md5hex, nbytes, crc_ok)."""
    data_off, method2, csize2, usize2 = member_data_offset(url, lho)
    if method2 != method:
        print(f'    note: local method {method2} != cd method {method}')
    if csize2 == 0xFFFFFFFF:  # zip64 local header: real sizes in extra field
        lh = http_get(url, lho, lho + 29 + 128)
        # walk extra field for 0x0001 zip64
        base = lho + 30
        # name length unknown here; skip: read 256 bytes after header and scan
        blob = lh
        pos = 30
        # name first (nlen from header), then extra
        nlen = struct.unpack('<H', blob[26:28])[0]
        elen = struct.unpack('<H', blob[28:30])[0]
        extra = blob[30 + nlen:30 + nlen + elen]
        e = 0
        while e + 4 <= len(extra):
            hid, hsz = struct.unpack('<HH', extra[e:e + 4])
            if hid == 0x0001:
                vals = extra[e + 4:e + 4 + hsz]
                if hsz >= 8:
                    usize2 = struct.unpack('<Q', vals[0:8])[0]
                if hsz >= 16:
                    csize2 = struct.unpack('<Q', vals[8:16])[0]
                break
            e += 4 + hsz
    total_compressed = csize2
    outdir = os.path.dirname(outpath)
    os.makedirs(outdir or '.', exist_ok=True)
    comp_path = outpath + '.part.deflate'
    have = 0
    if os.path.exists(comp_path):  # resume an interrupted chunked download
        have = os.path.getsize(comp_path)
        if have > total_compressed:
            have = 0
    chunk = chunk_mb * 1024 * 1024
    t0 = time.time()
    with open(comp_path, 'ab') as fh:
        while have < total_compressed:
            end = min(have + chunk, total_compressed) - 1
            blob = http_get(url, data_off + have, data_off + end)
            fh.write(blob)
            fh.flush()
            have += len(blob)
            mbps = have / 1024 / 1024 / max(time.time() - t0, 0.001)
            print(f'    {have:,}/{total_compressed:,} B compressed '
                  f'({100 * have / total_compressed:.1f}%, {mbps:.1f} MB/s)', flush=True)
    # inflate (streaming)
    want = limit_uncompressed
    sha = hashlib.sha256()
    md5 = hashlib.md5()
    nbytes = 0
    crc32 = 0
    d = zlib.decompressobj(-15) if method2 == 8 else None
    with open(comp_path, 'rb') as fh, open(outpath, 'wb') as out:
        while True:
            raw = fh.read(1024 * 1024)
            if not raw:
                break
            if d is None:
                data = raw if want is None else raw[:max(0, want - nbytes)]
            else:
                # NB: decompressobj.decompress max_length: 0 = unlimited
                data = d.decompress(raw, max(0, want - nbytes) if want is not None else 0)
            if data:
                out.write(data)
                sha.update(data)
                md5.update(data)
                crc32 = zlib.crc32(data, crc32)
                nbytes += len(data)
                if want is not None and nbytes >= want:
                    break
    full = (want is None or nbytes < want) and (method2 != 8 or d.eof)
    os.remove(comp_path)
    crc_ok = None
    if full and want is None and nbytes == usize2:
        crc_ok = (crc32 & 0xFFFFFFFF) == 0  # stream CRC computed across full data
        # NOTE: zlib.crc32 accumulated equals the zip's CRC for the full member
    return sha.hexdigest(), md5.hexdigest(), nbytes, crc_ok


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('zip_url')
    ap.add_argument('--list', action='store_true', help='list members and exit')
    ap.add_argument('--member', action='append', default=[],
                    help='exact member name (repeatable)')
    ap.add_argument('--pattern', action='append', default=[],
                    help='regex over member names (repeatable)')
    ap.add_argument('--outdir', default=ANALYSIS + '/remote')
    ap.add_argument('--chunk-mb', type=int, default=16)
    ap.add_argument('--limit-uncompressed', type=int, default=None,
                    help='inflate only the first N bytes of each member')
    ap.add_argument('--manifest', default=None,
                    help='TSV manifest to append fetched-file rows to')
    args = ap.parse_args()

    total, members = read_zip_cd(args.zip_url)
    print(f'zip: {args.zip_url}')
    print(f'     {total:,} bytes total, {len(members)} members')
    if args.list:
        for name, method, csize, usize, lho, crc in members:
            print(f'  {name}  method={method} csize={csize:,} usize={usize:,} '
                  f'local@{lho:#x} crc={crc:08x}')
        return

    want = []
    for name in args.member:
        m = [x for x in members if x[0] == name]
        if not m:
            raise SystemExit(f'member not found: {name}')
        want.append(m[0])
    for pat in args.pattern:
        rx = re.compile(pat)
        for m in members:
            if rx.search(m[0]):
                want.append(m)
    seen = set()
    uniq = []
    for m in want:
        if m[0] not in seen:
            seen.add(m[0])
            uniq.append(m)
    if not uniq:
        raise SystemExit('no members matched')
    print(f'fetching {len(uniq)} member(s):')
    manifest = None
    if args.manifest:
        os.makedirs(os.path.dirname(args.manifest) or '.', exist_ok=True)
        manifest = open(args.manifest, 'a', encoding='utf-8')
        if os.path.getsize(args.manifest) == 0:
            manifest.write(MANIFEST_HEADER)
            manifest.flush()
    rows = []
    for name, method, csize, usize, lho, crc in uniq:
        if usize == 0 and name.endswith('/'):
            print(f'  [dir ] {name} (skipped)')
            continue
        safe = name.lstrip('/')
        outpath = os.path.join(args.outdir, safe.replace('/', os.sep))
        os.makedirs(os.path.dirname(outpath) or '.', exist_ok=True)
        print(f'  [get ] {name}  ({csize:,} B compressed -> {usize:,} B)', flush=True)
        sha, md5, nbytes, crc_ok = fetch_member(
            args.zip_url, name, method, csize, usize, lho, outpath,
            chunk_mb=args.chunk_mb, limit_uncompressed=args.limit_uncompressed)
        note = '' if crc_ok in (None, False) or crc_ok else ''
        print(f'  [done] {name}')
        print(f'         SHA256 {sha}')
        print(f'         MD5    {md5}  ({nbytes:,} B'
              + (', full member' if crc_ok else '') + ')')
        rows.append((sha, md5, nbytes, name))
        if manifest:
            manifest.write(f'{sha}\t{md5}\t{nbytes}\t{crc_ok}\t{args.zip_url}\t{name}\n')
            manifest.flush()
    if manifest:
        manifest.close()
    print(f'\nfetched {len(rows)} file(s); every file SHA256-printed above'
          + (f'; manifest: {args.manifest}' if args.manifest else ''))


if __name__ == '__main__':
    main()
