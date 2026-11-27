#!/usr/bin/env python3
"""crc_model.py - literal model of crc.dll!GenerateCRC32 (v2.6.0.6).

Table: init_crc32_table @0x1010 - reverse8(i) << 24, 8 rounds MSB-first
poly 0x04C11DB7, then the result bit-reversed (32-bit).
Update: GenerateCRC32 @0x1110 - crc=0; crc = ((crc << 8) | b) ^ T[crc >> 24].
"""

M32 = 0xFFFFFFFF
POLY = 0x04C11DB7


def reverse8(i):
    r = 0
    for b in range(8):
        r |= ((i >> b) & 1) << (7 - b)
    return r


def reverse32(x):
    r = 0
    for b in range(32):
        r |= ((x >> b) & 1) << (31 - b)
    return r


def build_table():
    tbl = []
    for i in range(256):
        c = reverse8(i) << 24
        for _ in range(8):
            t = (c << 1) & M32
            if c & 0x80000000:
                t ^= POLY
            c = t
        tbl.append(reverse32(c))
    return tbl


T = build_table()


def generate_crc32(buf, _unused_len=None):
    crc = 0
    for b in buf:
        crc = (((crc << 8) & M32) | b) ^ T[(crc >> 24) & 0xFF]
    return crc


if __name__ == '__main__':
    # sanity: does the table equal the reflected 0xEDB88320 table?
    ref = []
    for i in range(256):
        c = i
        for _ in range(8):
            if c & 1:
                c = (c >> 1) ^ 0xEDB88320
            else:
                c >>= 1
        ref.append(c)
    print('table == reflected 0xEDB88320 table:', T == ref)

    for s in [b'123456789', b'', b'\x00', b'a', b'hello world']:
        print(f'{s!r:16} -> {generate_crc32(s):08x}')
    print('CRC-32/MPEG-2 check would be 0376e6e7; plain zlib crc32(123456789) =',
          f'{__import__("zlib").crc32(b"123456789"):08x}')

    # the actual ens_code.ldr
    import hashlib
    d = open('/home/z/my-project/Idong100/software/firmware/v2.6.0.6/ens_code.ldr', 'rb').read()
    print('ens_code.ldr size', len(d), 'sha256', hashlib.sha256(d).hexdigest())
    print('ens_code.ldr CRC (vendor) =', f'{generate_crc32(d):08x}')
    print('chunks of 64:', (len(d) + 63) // 64, 'last-chunk valid bytes:', len(d) % 64)
