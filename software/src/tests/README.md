Integration tests (src/tests/ - turn 39: everything lives under src/;
run them from software/):
- mock_replay.rs    the historical PU/OD boot framing + CRC/timeout
                    injection over MockTransport (the DLL-side model)
- runtime_replay.rs the OBSERVED wire protocol: the cap3 flash
                    sequence byte-for-byte (335 blocks + readback +
                    corrupted-readback rejection), the turn-33 engine
                    gate reads, record classification
- video_golden.rs   the UVC plane: 12-B payload headers, probe/commit
                    bodies, PTS frame closure, two-channel raster split,
                    the 0x0205 stream poll
- firmware_files.rs the vendor files shipped in software/firmware/
                    (turn 40): every image against the turn-12 manifest,
                    the driver pins, SHA256SUMS completeness, the vendor
                    file map (xeyezHal.ini / xEyeUsb.ini), the register
                    table format, the folder lookup of src/firmware.rs
                    (reads the source tree, no device, no MockTransport)
The first three are hardware-free (MockTransport); the bench tests are
the examples under src/examples/ (docs/LINUX_TESTS.md).
