//! Length-prefixed JSON frames: a u32 little-endian byte count, then that
//! many bytes of UTF-8 JSON. Timeouts belong to the socket, set by the caller.

use std::fmt;
use std::io::{self, Read, Write};

use wugs_model::{json, Value};

/// Op batches for a whole text can be large; anything past this is treated
/// as a corrupt or hostile length, not a big batch.
pub const MAX_FRAME: usize = 32 * 1024 * 1024;

#[derive(Debug)]
pub enum FrameError {
    /// The peer closed the connection cleanly between frames.
    Closed,
    /// The bytes on the wire are not a valid frame; the connection must close.
    Protocol(String),
    Io(io::Error),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::Closed => write!(f, "peer closed the connection"),
            FrameError::Protocol(m) => write!(f, "protocol error: {m}"),
            FrameError::Io(e) => write!(f, "i/o error: {e}"),
        }
    }
}

impl From<io::Error> for FrameError {
    fn from(e: io::Error) -> Self {
        FrameError::Io(e)
    }
}

pub fn write_frame(w: &mut impl Write, v: &Value) -> Result<(), FrameError> {
    let body = v.to_json();
    if body.len() > MAX_FRAME {
        return Err(FrameError::Protocol(format!("frame of {} bytes exceeds {MAX_FRAME}", body.len())));
    }
    w.write_all(&(body.len() as u32).to_le_bytes())?;
    w.write_all(body.as_bytes())?;
    w.flush()?;
    Ok(())
}

pub fn read_frame(r: &mut impl Read) -> Result<Value, FrameError> {
    let mut len = [0u8; 4];
    let mut got = 0;
    while got < len.len() {
        match r.read(&mut len[got..]) {
            Ok(0) if got == 0 => return Err(FrameError::Closed),
            Ok(0) => return Err(FrameError::Protocol("connection closed inside a frame header".into())),
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
    }
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(FrameError::Protocol(format!("frame of {len} bytes exceeds {MAX_FRAME}")));
    }
    // Grow with the bytes actually received: a hostile header claiming a
    // huge frame must not allocate it up front.
    let mut body = Vec::with_capacity(len.min(1 << 20));
    let mut taken = r.take(len as u64);
    let read = taken.read_to_end(&mut body).map_err(FrameError::Io)?;
    if read < len {
        return Err(FrameError::Protocol("connection closed inside a frame".into()));
    }
    let text = String::from_utf8(body).map_err(|_| FrameError::Protocol("frame is not UTF-8".into()))?;
    json::parse(&text).map_err(|e| FrameError::Protocol(format!("frame is not JSON: {e}")))
}
