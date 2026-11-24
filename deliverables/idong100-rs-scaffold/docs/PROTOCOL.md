# PROTOCOL.md - wire facts (P0.1 deliverable; to be filled from FINDINGS)

Every row must cite its source (DLL + RVA). Constants here are encoded
verbatim into `idong/src/protocol/mod.rs`.

## USB identity

| Item | Value | Source |
|---|---|---|
| bootloader | 0471:2600, iface 0, EP 0x02 OUT / 0x82 IN, maxpacket 64 | xeyez.inf |
| runtime | 0471:2500 composite, vendor MI_04 | xeyez.inf, DriverConfig.ini |

## Boot framing (PU/OD magic)

| Item | Value | Source |
|---|---|---|
| H2D magic | 0x7f7f5550 "PU" | xEyeUSB.dll |
| D2H magic | 0x7f7f444f "OD" | xEyeUSB.dll |
| packet size | 64 B type-3 | _InitBlobUpload |
| timeouts | 3 s ready / 6 s packet / 300 ms x 10 dev-ID / 1 s reset | xEyeUSB.dll |

## Runtime framing (7F 55)

| Item | Value | Source |
|---|---|---|
| layout | 7F 55; cmd@[2]; b2-b4@[3..5]; u16 arg@[6..7] byte-swapped; payload <= 56 B | xeyezHal builder @0x10003010 |
| cmd 3 | register write, 8 B chunks | SetCmosTable loop |
| cmd 5 | blob data read | GetBlobData |
| cmd 6 | GET_IMG_START (status 1) | GetImageBegin |
| cmd 15 | classifier/reg write | chunk loop @0x10003a50 |

## Marker record (xEye3Ddata)

0x3BC B: frame counter @+0 (overlays slot 0); 10 x 88 B slots;
quality i32 @ slot+0x24 (>= 50 tracked); gate f64 @ slot+0x18;
mode flag u32 @ +0x3B8; pipe form 0x208 B. Source: xEyeClass.dll
GetADataFromUSB @RVA 0x20f0.

## Open items (P2.3 USBPcap gate)

- multi-firmware staging order (ldr/bit/hex)
- CRC variant announced in count+CRC step
- exact count/CRC payload framing

## Firmware sha256 pins

From analysis/reports/all_extracted_sha256.tsv (turn 12). Fill in at
P0.3 from the manifest; verify with `idong/examples/verify_firmware`.
