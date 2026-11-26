# Firmware and vendor device files

Device-side files of the idong sensor, copied byte for byte from the vendor
software packages. The driver reads them from this folder: nothing has to be
downloaded or searched for separately.

```
software/firmware/
├── README.md            this file
├── SHA256SUMS           `sha256sum -c SHA256SUMS` checks all 24 files
├── v2.6.0.6/            the device files of vendor package xeyez_pc_v2.6.0.6
│   ├── app_m8051_2611.hex, app_m8051_2500.hex, xeyez_v21.hex, xeyez_v20.hex
│   ├── A_a_v4.4_BG0313.bit, ens_code.ldr
│   ├── licence.txt      the vendor's terms, unmodified
│   └── config/          the tables and .ini files the vendor host reads
│       ├── xeyezHal.ini, xEyeUsb.ini, DriverConfig.ini
│       ├── Led_Scale_Fpga_v26.txt, Global_RW_0316_Cmos_v26.txt,
│       │   Rolling_RW_0316_Cmos_v26.txt, data0.dat, data1.dat, data10.dat
│       └── 30fps/, 30fps_1/
└── v2.5.0.7/            vendor package xeyez_pc_v2.5.0.7: only what differs
    ├── app_m8051.hex
    └── config/xeyezHal.ini
```

## Where the files come from

| Item | Value |
|---|---|
| Packages | `xeyez_pc_v2.6.0.6` and `xeyez_pc_v2.5.0.7`, each as `xeyez_alone.dat` and `xeyez_Lobby.dat` |
| Place in the package | NSIS payload folder `$_6_`, next to the vendor program |
| Flavour check | every file is byte-identical in the `xeyez_alone` and the `xeyez_Lobby` flavour |
| Manifest check | every file matches the SHA-256 in the turn-12 extraction manifest `research/analysis/reports/all_extracted_sha256.tsv` |
| `v2.5.0.7/` | holds only the two files that differ; the other 16 device-side files of that package are byte-identical to `v2.6.0.6/` |

The `config/` files keep the names they have in the payload folder. The images
sit in the payload root there, so they sit in the version folder here.

Line endings are part of the bytes: the hex files are Intel HEX text with CRLF
line endings and the driver's SHA-256 pins cover exactly these bytes.
`.gitattributes` marks this folder `-text`, so git never rewrites line endings
here on any platform.

## What each file is

| File | Size | SHA-256 (first 16) | Role |
|---|---|---|---|
| `v2.6.0.6/app_m8051_2611.hex` | 61,756 | `1fad00c1dca7f995` | 8051 MCU program that the vendor host downloads into the bootloader (0471:2600); the image `flash_mcu` replays |
| `v2.6.0.6/app_m8051_2500.hex` | 61,031 | `ffb8b96274c3b4c4` | 8051 MCU program listed for `[Version2.5]` in `xeyezHal.ini` |
| `v2.6.0.6/xeyez_v21.hex` | 26,304 | `0298a4d8a6501bcd` | 8051 MCU program listed for `[Version2.1]` |
| `v2.6.0.6/xeyez_v20.hex` | 25,913 | `3f82b8ccd5a80aef` | 8051 MCU program listed for `[Version2.0]` |
| `v2.6.0.6/A_a_v4.4_BG0313.bit` | 235,930 | `75df0a38037e1983` | Xilinx FPGA bitstream, part `3s400aft256` (Spartan-3A XC3S400A), design `work_top.ncd`, built 2010/04/06 10:33:41 |
| `v2.6.0.6/ens_code.ldr` | 54,420 | `3a19c68a33352cda` | Loader image of the Blackfin BF527 DSP |
| `v2.6.0.6/licence.txt` | 4,081 | `eeaf96933c66571f` | The vendor's terms (GBK text), see below |
| `v2.6.0.6/config/xeyezHal.ini` | 1,568 | `aa38a93ec373f220` | The vendor host's file map: which hex and which tables per sensor version |
| `v2.6.0.6/config/xEyeUsb.ini` | 1,193 | `6e1d73d2f8520043` | Vendor host USB settings, among them `BitFile=A_a_v4.4_BG0313.bit` |
| `v2.6.0.6/config/DriverConfig.ini` | 632 | `9db544dbd427b74d` | Vendor list of hardware IDs and Windows driver files |
| `v2.6.0.6/config/*.txt`, `data*.dat`, `30fps/`, `30fps_1/` | 9 to 442 | see `SHA256SUMS` | Register tables, one `0xRR,0xVV` pair per line |
| `v2.5.0.7/app_m8051.hex` | 57,683 | `09df3d856775bbb0` | The `[Version2.5]` MCU program of package 2.5.0.7 |
| `v2.5.0.7/config/xeyezHal.ini` | 851 | `a9e64102fc686f67` | The 2.5.0.7 file map |

The full hashes are in `SHA256SUMS`.

## Which files the driver uses

| Use | File | Where it is enforced |
|---|---|---|
| 8051 flash replay (`cargo run --features usb --example flash_mcu`) | `v2.6.0.6/app_m8051_2611.hex` | `FirmwareSet::verify_hex` (file hash) and `FirmwareSet::mcu_image` (hash of the flattened image) |
| Image set of a full session (`FirmwareSet`) | `ens_code.ldr`, `A_a_v4.4_BG0313.bit`, `app_m8051_2611.hex` | `FirmwareSet::verify` |
| Locating the folder | | `src/firmware.rs` |

`src/firmware.rs` resolves the folder in this order: the crate's own
`firmware/v2.6.0.6` (compiled-in path of the crate), then that folder relative
to the program's location (covers `target/debug/examples/`), then
`firmware/v2.6.0.6` relative to the current directory. `flash_mcu` takes
`--hex <path>` to use another file; a bare file name is looked up in this
folder.

`app_m8051_2611.hex` is the only MCU image the driver flashes. The other hex
files are not pinned, so `flash_mcu` refuses them; none of them has been seen on
the wire of the test unit. The captures also show that the `.bit` and `.ldr`
files are never uploaded by the host (PROTOCOL.md, turn 32); the driver pins
them because the vendor host opens them by name.

## The vendor's file map

`v2.6.0.6/config/xeyezHal.ini`, active entries only:

| Section | Entry | File in this folder |
|---|---|---|
| `[Version2.0]` | `HexFileName` | `v2.6.0.6/xeyez_v20.hex` |
| `[Version2.1]` | `HexFileName` | `v2.6.0.6/xeyez_v21.hex` |
| `[Version2.5]` | `HexFileName` | `v2.6.0.6/app_m8051_2500.hex` |
| `[Version2.5]` | `Global_Blb_0316_Cmos`, `Global_Img_0316_Cmos` | `v2.6.0.6/config/30fps/30fps_Global_0316_Cmos.txt` |
| `[Version2.5]` | `Global_Blb_0316_Fpga` | `v2.6.0.6/config/30fps/30fps_Global_blob_1LED.dat` |
| `[Version2.5]` | `Global_Img_0316_Fpga` | `v2.6.0.6/config/30fps/30fps_Global_image_2sensor_diff_1LED.dat` |
| `[Version2.6]` | `HexFileName` | `v2.6.0.6/app_m8051_2611.hex` |
| `[Version2.6]` | `Global_Blb_0316_Cmos`, `Global_Img_0316_Cmos` | `v2.6.0.6/config/30fps_1/30fps_Global_0316_Cmos.txt` |
| `[Version2.6]` | `Global_Blb_0316_Fpga` | `v2.6.0.6/config/30fps_1/30fps_Global_blob_1LED.dat` |
| `[Version2.6]` | `Global_Img_0316_Fpga` | `v2.6.0.6/config/30fps_1/30fps_Global_image_2sensor_diff_1LED.dat` |
| both | `Rolling_RW_0316_Cmos` | `v2.6.0.6/config/Rolling_RW_0316_Cmos_v26.txt` |
| both | `Global_RW_0316_Cmos` | `v2.6.0.6/config/Global_RW_0316_Cmos_v26.txt` |
| both | `Led_Scale_Fpga` | `v2.6.0.6/config/Led_Scale_Fpga_v26.txt` |

Values that start with `#` are the vendor's comments and name no file. Two
entries without `#` name files that are not in the package at all
(`Rolling_0316_Cmos_v26.txt`, `Rolling_Img_0316_Fpga_v26.txt`), so they cannot
be shipped. The 2.5.0.7 map names `app_m8051.hex` for `[Version2.5]` and reads
the global tables from `30fps/` only.

`xEyeUsb.ini` names the bitstream (`BitFile`); `PROTOCOL.md` quotes its
quality settings (`RingSize`, the green/yellow/red thresholds, `USBComFPS`).

## Register tables

The `.txt` and `.dat` tables under `config/` are plain text: one
`0xRR,0xVV` pair (register, value) per line, CRLF line endings. They are not
bitstreams. The vendor host sends them during session init as the 8-byte chunk
writes (0x0103/0x0203) and the CMOS table writes (0x0104/0x0204) that the
captures show (PROTOCOL.md, turn 33). `30fps_1/30fps_Global_blob_1LED.dat` is
the only table that differs between the two folders.

## Licence

These files are the vendor's property and are not covered by the MIT licence
of the driver code. The vendor's terms ship with them, unmodified, as
`v2.6.0.6/licence.txt` (深圳泰山在线科技有限公司). In short: users may not
copy or modify the materials separately (clause 1) and may not reverse
engineer, decompile or disassemble the product (clause 2); the product is
freeware that users may download, install, copy and distribute
non-commercially, and commercial sale, copying or distribution needs the
vendor's prior written permission (clause 3).

## Re-checking

```
cd software/firmware && sha256sum -c SHA256SUMS
cd software && cargo test --test firmware_files
```

The test `firmware_files` checks every hash in `SHA256SUMS`, that no file is
missing from it, the three driver pins, the vendor file map above and the
folder lookup of `src/firmware.rs`.
