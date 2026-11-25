//! Runtime command engine - the "7F 55" framing (xeyezHal.dll v2.6.0.6).
//!
//! H2D: [0]=0x7F [1]=0x55 [2]=cmd [3..5]=b2-b4 [6..7]=u16 arg (BYTE
//! -SWAPPED on the wire) [8..]=payload (<= 56 B).  D2H: [4]=status.
//! Commands: 3 register write (8 B chunks), 5 blob read, 6 GET_IMG_START
//! (status 1), 15 classifier write.  Timeouts 1 s TX / 5 s RX; data
//! pipe 3 for GetImageData.
//!
//! Safety: only vendor-decoded commands are representable (error.rs
//! rejects everything else - rule "no speculative writes").

pub enum Cmd { RegWrite = 3, BlobRead = 5, ImgStart = 6, Classifier = 15 }

// TODO(P1.4): CmdEngine { execute(cmd, arg, payload) -> Result<Response> }
