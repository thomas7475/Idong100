//! xEye3Ddata record parser (xEyeClass.dll, GetADataFromUSB @RVA 0x20f0).
//!
//! Layout (verified structurally, field semantics partially open):
//! 0x3BC B minimum: frame counter u32 @ +0 (overlays slot 0's first u32);
//! 10 slots x 88 B at +0: gate f64 @ slot+0x18, quality i32 @ slot+0x24
//! (>= 50 tracked); 72 B trailer; mode flag u32 @ +0x3B8 (==1 skip).
//! Pipe form: 0x208 B after the RVA 0x25e0 transform.
//!
//! TODO(P1.2): total decode + golden-vector test; open item #4: which
//! slot bytes are x/y/z vs gate - resolve via xEyeClass 0x25e0 disasm.
