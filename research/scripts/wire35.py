#!/usr/bin/env python3
"""Turn-35 wire phase decoder (Python port + video analysis).

Decodes USBPcap .pcap captures of the idong100 sensor:
  phase 0  enum only
  phase 1  boot flash    (0101 flash upload, 0201/0301/0401)
  phase 2  session/polls (0008 -> "2619", EEPROM reads 0107)
  phase 3  device-ID read (0107 @0x0300 + serial reply)
  phase 4  UVC negotiation (probe/commit, SET_INTERFACE)
  phase 5  VIDEO STREAMING (ep 0x85 isochronous / bulk IN)

Usage: wire35.py <file.pcap> [more.pcap ...]
"""
import sys
import struct
from collections import defaultdict

RT = b"\x7f\x55"                                    # MCU framing
PU = b"\x50\x55\x7f\x7f"                            # DSP boot host->sensor
OD = b"\x4f\x44\x7f\x7f"                            # DSP boot sensor->host
LDR = bytes.fromhex("4000 80ff 0400 0000 1200 86d4".replace(" ", ""))
BIT = bytes.fromhex("0009 0ff0 0ff0 0ff0 0ff0 0000 0161 0030".replace(" ", ""))
SERIAL = b"AB460DC8BA12C6840B49CD5786C329DD"


def u16(b, o):
    return b[o] | (b[o + 1] << 8)


def u32(b, o):
    return (b[o] | (b[o + 1] << 8) | (b[o + 2] << 16) | (b[o + 3] << 24)) & 0xFFFFFFFF


def fmt_size(n):
    if n >= 1048576:
        return f"{n / 1048576:.1f} MB"
    if n >= 1024:
        return f"{n / 1024:.1f} kB"
    return f"{n} B"


class Result:
    def __init__(self, path):
        self.path = path
        self.size = 0
        self.packets = 0
        self.data = 0
        self.err = ""
        self.first_ts = 0.0
        self.last_ts = 0.0
        self.flash_out = 0
        self.flash_in = 0
        self.ep02_out = 0
        self.ep82_in = 0
        self.pu = 0
        self.od = 0
        self.v2600 = 0
        self.v2619 = 0
        self.ee_reads = []
        self.dev_id_read = 0
        self.dev_id_reply = 0
        self.ldr_out = 0
        self.ldr_bytes = 0
        self.bit_out = 0
        self.bit_bytes = 0
        self.uvc_probe = 0
        self.uvc_commit = 0
        self.set_iface = []
        self.in_by_ep = defaultdict(int)
        self.out_by_ep = defaultdict(int)
        self.iso_by_ep = defaultdict(int)       # isochronous IN payloads
        self.cam2600 = -1
        self.cam2500 = -1
        self.ep85_pkts = 0
        self.ep85_bytes = 0
        self.ep85_payload_sizes = defaultdict(int)
        # cmd histogram of 7f55 runtime frames (OUT)
        self.cmd_hist = defaultdict(int)
        self.cmd_hist_in = defaultdict(int)
        self.uvc_control = []                    # (bRequest, wValue, wIndex, wLength, dir)
        self.string_descs = []
        self.set_cur_payloads = defaultdict(list)  # CS -> payloads
        self.probe_commit_payloads = []
        self.other_bulk_eps = defaultdict(int)

    def process(self, pkt, off, incl, ts):
        self.packets += 1
        if not self.first_ts:
            self.first_ts = ts
        self.last_ts = ts
        if incl < 27:
            return
        hlen = u16(pkt, off)
        if hlen < 27 or hlen > incl:
            return
        dev = u16(pkt, off + 19)
        ep = pkt[off + 21]
        tr = pkt[off + 22]
        dlen = u32(pkt, off + 23)
        d = off + hlen
        avail = incl - hlen
        if avail > dlen:
            avail = dlen
        if avail <= 0:
            return

        if tr == 2:  # control
            b0 = pkt[d] if avail > 0 else -1
            b1 = pkt[d + 1] if avail > 1 else -1
            if b0 == 0x12 and b1 == 0x01 and avail >= 18 and u16(pkt, d + 8) == 0x0471:
                pid = u16(pkt, d + 10)
                if pid == 0x2600 and self.cam2600 < 0:
                    self.cam2600 = dev
                if pid == 0x2500 and self.cam2500 < 0:
                    self.cam2500 = dev
            if b0 == 0x01 and b1 == 0x0b:  # SET_INTERFACE
                alt = u16(pkt, d + 2)
                iface = u16(pkt, d + 4)
                if alt > 0:
                    self.set_iface.append(f"if{iface} alt{alt}")
                self.uvc_control.append(("SET_IF", f"if{iface}", alt, avail, "out"))
            if b0 == 0x21 and b1 == 0x01:  # UVC SET_CUR
                val = u16(pkt, d + 2)
                idx = u16(pkt, d + 4)
                cs = (val >> 8) & 0xFF
                ent = val & 0xFF
                if cs == 0x01:
                    self.uvc_probe += 1
                if cs == 0x02:
                    self.uvc_commit += 1
                self.set_cur_payloads[cs].append(bytes(pkt[d + 8:d + min(avail, 8 + 48)]))
                self.uvc_control.append(("SET_CUR", f"cs{cs:02x}", idx, avail, "out"))
            if b0 == 0x21 and b1 == 0x81:  # GET_CUR
                val = u16(pkt, d + 2)
                idx = u16(pkt, d + 4)
                cs = (val >> 8) & 0xFF
                self.uvc_control.append(("GET_CUR", f"cs{cs:02x}", idx, u16(pkt, d + 6), "in"))
            if b0 == 0x06 and b1 == 0x00:  # GET_DESCRIPTOR
                self.uvc_control.append(("GET_DESC", f"{u16(pkt, d + 2):04x}", u16(pkt, d + 4), u16(pkt, d + 6), "?"))
            if b0 == 0x80 and b1 == 0x06:  # GET_DESCRIPTOR (device)
                dtype = u16(pkt, d + 2) & 0xFF
                if dtype == 0x03:
                    self.uvc_control.append(("GET_DESC_STR", f"{u16(pkt, d + 2):04x}", u16(pkt, d + 4), u16(pkt, d + 6), "in"))
            return

        if tr == 0:  # isochronous
            epn = ep & 0x7F
            if ep & 0x80:
                self.iso_by_ep[epn] += avail
                if epn == 0x85:
                    self.ep85_pkts += 1
                    self.ep85_bytes += avail
                    self.ep85_payload_sizes[avail] += 1
                else:
                    self.other_bulk_eps[f"iso ep{epn:02x}"] += 1
            else:
                self.out_by_ep[epn] += avail
            return

        if tr not in (3, 1):
            return
        if avail < 2:
            return
        self.data += 1
        head = bytes(pkt[d:d + 4])
        if not (ep & 0x80):  # OUT
            self.out_by_ep[ep & 0x7F] += avail
            if (ep & 0x7F) == 0x02:
                self.ep02_out += avail
                if head == RT:
                    cmd = u16(pkt, d + 2)
                    self.cmd_hist[cmd] += 1
                    alen = pkt[d + 7] if avail > 7 else 0
                    if cmd == 0x0107 and alen >= 2 and avail >= 10:
                        eo = (pkt[d + 8] << 8) | pkt[d + 9]
                        el = (pkt[d + 10] << 8) | pkt[d + 11] if avail > 11 else 0
                        self.ee_reads.append((eo, el))
                        if eo == 0x0300 and el == 0x21:
                            self.dev_id_read += 1
                    if cmd == 0x0101:
                        self.flash_out += 1
                if bytes(pkt[d:d + 12]) == LDR:
                    self.ldr_out += 1
                if bytes(pkt[d:d + 16]) == BIT:
                    self.bit_out += 1
                if head == PU:
                    self.pu += 1
        else:  # IN
            epn = ep & 0x7F
            self.in_by_ep[epn] += avail
            if epn == 0x02:
                self.ep82_in += avail
                if head == PU:
                    self.od += 1
                if head == RT:
                    if avail >= 12 and pkt[d + 2] == 0x08 and pkt[d + 3] == 0x00:
                        ver = bytes(pkt[d + 8:d + 12])
                        if ver == b"2600":
                            self.v2600 += 1
                        elif ver == b"2619":
                            self.v2619 += 1
                    if avail >= 4:
                        self.cmd_hist_in[u16(pkt, d + 2)] += 1
                if avail >= 33 and bytes(pkt[d:d + 32]) == SERIAL:
                    self.dev_id_reply += 1
            else:
                self.other_bulk_eps[f"bulk ep{epn:02x}"] += avail

    def phase(self):
        if self.ep82_in > 1000 and (self.set_iface or self.uvc_commit):
            return 5
        if self.in_by_ep.get(0x85, 0) > 10000 or self.iso_by_ep.get(0x85, 0) > 10000:
            return 5
        if self.set_iface or self.uvc_commit or self.uvc_probe:
            return 4
        if self.dev_id_read or self.dev_id_reply:
            return 3
        if self.v2619 or self.ee_reads:
            return 2
        if self.ep02_out > 10000:
            return 1
        if self.packets:
            return 0
        return -1


PHASES = ["enum only", "boot flash", "session/polls", "device-ID read",
          "UVC negotiation", "VIDEO STREAMING"]


def scan(path):
    r = Result(path)
    with open(path, "rb") as f:
        data = f.read()
    r.size = len(data)
    if r.size < 24:
        r.err = "file is too small"
        return r
    if data[:4] != b"\xd4\xc3\xb2\xa1":
        r.err = "not a USBPcap pcap file"
        return r
    pos = 24
    n = 0
    while pos + 16 <= len(data):
        incl = u32(data, pos + 8)
        if incl < 1 or incl > 8 * 1024 * 1024:
            r.err = f"corrupt record at offset {pos}"
            break
        if pos + 16 + incl > len(data):
            break
        ts = u32(data, pos) + u32(data, pos + 4) / 1e6
        r.process(data, pos + 16, incl, ts)
        pos += 16 + incl
        n += 1
    return r


def report(r):
    out = []
    out.append(f"  file      : {r.path}")
    dur = r.last_ts - r.first_ts if r.packets else 0
    out.append(f"  size/time : {fmt_size(r.size)}, {dur:.1f} s, {r.packets} packets")
    if r.err:
        out.append(f"  ERROR     : {r.err}")
        return out
    cams = []
    if r.cam2600 >= 0:
        cams.append(f"2600 dev{r.cam2600}")
    if r.cam2500 >= 0:
        cams.append(f"2500 dev{r.cam2500}")
    out.append(f"  sensor    : {' '.join(cams) if cams else 'not seen'}")
    out.append(f"  ep02 out  : {fmt_size(r.ep02_out)}   ep82 in : {fmt_size(r.ep82_in)}")
    out.append(f"  versions  : 2600 x{r.v2600}, 2619 x{r.v2619}, DSP PU x{r.pu}, OD x{r.od}")
    ee = r.ee_reads[:8]
    ees = "; ".join(f"0x{o:04x} len {l}" for o, l in ee)
    out.append(f"  EEPROM    : {ees if ees else 'none'}" +
               (f" ... {len(r.ee_reads)} total" if len(r.ee_reads) > 8 else ""))
    out.append(f"  device-ID : read x{r.dev_id_read}, serial reply x{r.dev_id_reply}")
    out.append(f"  DSP/FPGA  : ldr-start x{r.ldr_out}, bit-start x{r.bit_out}")
    out.append(f"  UVC       : probe x{r.uvc_probe}, commit x{r.uvc_commit}" +
               (f", {'; '.join(r.set_iface)}" if r.set_iface else ", no alt set"))
    bulk = [f"ep 0x{k:02x} in {fmt_size(v)}" for k, v in sorted(r.in_by_ep.items()) if v > 1000]
    out.append(f"  bulk IN   : {', '.join(bulk) if bulk else 'none'}")
    iso = [f"ep 0x{k:02x} iso {fmt_size(v)}" for k, v in sorted(r.iso_by_ep.items()) if v > 0]
    out.append(f"  iso IN    : {', '.join(iso) if iso else 'none'}")
    if r.ep85_pkts:
        out.append(f"  VIDEO ep85: {r.ep85_pkts} iso packets, {fmt_size(r.ep85_bytes)}, "
                   f"avg {r.ep85_bytes // max(r.ep85_pkts, 1)} B/pkt")
        sizes = sorted(r.ep85_payload_sizes.items())
        out.append(f"  ep85 sizes: {', '.join(f'{s}B x{c}' for s, c in sizes[:12])}")
    cmds = sorted(r.cmd_hist.items())
    if cmds:
        out.append("  cmd OUT   : " + ", ".join(f"{c:04x} x{n}" for c, n in cmds))
    cmdi = sorted(r.cmd_hist_in.items())
    if cmdi:
        out.append("  cmd IN    : " + ", ".join(f"{c:04x} x{n}" for c, n in cmdi))
    ph = r.phase()
    out.append(f"  PHASE     : {PHASES[max(ph, 0)]}")
    return out


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(3)
    print("=== idong100 wire phase report, turn 35 ===")
    best = -1
    for p in sys.argv[1:]:
        r = scan(p)
        print()
        print("\n".join(report(r)))
        if not r.err:
            best = max(best, r.phase())
    print()
    print(f"Verdict: engine reached \"{PHASES[max(best, 0)]}\".")
    sys.exit(0 if best >= 5 else 1)


if __name__ == "__main__":
    main()
