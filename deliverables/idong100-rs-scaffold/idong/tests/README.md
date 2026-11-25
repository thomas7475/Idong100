Test files (draft section 8, table 7):
- frames_golden.rs   codec vs golden vectors (tests/data/)
- eeprom_decode.rs   T24C16A checksums + known-good calibration values
- mock_replay.rs     boot + stream state machines over MockTransport
- hardware.rs        bench-only, every test #[ignore] by default
- data/              golden vectors: OD frames, EEPROM dump, scripts
