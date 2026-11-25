//! Chromium Pickle custom data (`org.chromium.web-custom-data`).
//!
//! This matches `signalPasteboardData` in `apps/macos/Pasteboard.swift` and the Lean model
//! `CopiedFormal.Pickle`. All integers are little-endian `u32`:
//!
//! ```text
//! payload_size | entry_count | (len | UTF-16LE units | zero pad to 4 bytes){2 per entry}
//! ```
//!
//! `payload_size` counts the bytes after the 4-byte header.

use alloc::string::String;
use alloc::vec::Vec;

use crate::CodecError;

/// Converts a length to the `u32` the wire format stores.
pub(crate) fn to_u32(value: usize) -> Result<u32, CodecError> {
    u32::try_from(value).map_err(|_| CodecError::Overflow)
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_string(payload: &mut Vec<u8>, value: &str) -> Result<(), CodecError> {
    push_u32(payload, to_u32(value.encode_utf16().count())?);
    for unit in value.encode_utf16() {
        payload.extend_from_slice(&unit.to_le_bytes());
    }
    while !payload.len().is_multiple_of(4) {
        payload.push(0);
    }
    Ok(())
}

/// Encodes `(mime_type, value)` pairs into Chromium's Pickle format (`org.chromium.web-custom-data`).
pub fn encode_chromium_custom_data(entries: &[(&str, &str)]) -> Result<Vec<u8>, CodecError> {
    let mut payload = Vec::new();
    push_u32(&mut payload, to_u32(entries.len())?);
    for &(mime, value) in entries {
        push_string(&mut payload, mime)?;
        push_string(&mut payload, value)?;
    }
    let mut out = Vec::with_capacity(payload.len() + 4);
    push_u32(&mut out, to_u32(payload.len())?);
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Encodes a Signal Desktop `text/signal` payload as Chromium Pickle bytes.
pub fn encode_signal_pickle(html: &str) -> Result<Vec<u8>, CodecError> {
    encode_chromium_custom_data(&[("text/signal", html)])
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, len: usize) -> Result<&[u8], CodecError> {
        let remaining = self.bytes.len() - self.pos;
        if len > remaining {
            return Err(CodecError::Truncated);
        }
        let start = self.pos;
        self.pos += len;
        Ok(&self.bytes[start..self.pos])
    }

    fn u32(&mut self) -> Result<usize, CodecError> {
        let raw = self.take(4)?;
        let value = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
        usize::try_from(value).map_err(|_| CodecError::Overflow)
    }

    fn string(&mut self) -> Result<String, CodecError> {
        let count = self.u32()?;
        let byte_len = count.checked_mul(2).ok_or(CodecError::Overflow)?;
        let raw = self.take(byte_len)?;
        let mut units = Vec::with_capacity(count);
        for pair in raw.as_chunks::<2>().0 {
            units.push(u16::from_le_bytes(*pair));
        }
        let pad = (4 - self.pos % 4) % 4;
        for &byte in self.take(pad)? {
            if byte != 0 {
                return Err(CodecError::InvalidAlignment);
            }
        }
        String::from_utf16(&units).map_err(|_| CodecError::InvalidUtf16)
    }
}

/// Decodes a Chromium Pickle custom-data buffer into `(mime_type, value)` pairs.
///
/// Decoding is strict. The declared payload size must equal the bytes present and be a
/// multiple of 4. Padding must be zero, strings must be valid UTF-16, and the entries must
/// consume the payload exactly.
pub fn decode_chromium_custom_data(bytes: &[u8]) -> Result<Vec<(String, String)>, CodecError> {
    let mut reader = Reader { bytes, pos: 0 };
    let declared = reader.u32()?;
    let actual = bytes.len() - 4;
    if declared != actual {
        return Err(CodecError::LengthMismatch { declared, actual });
    }
    if !declared.is_multiple_of(4) {
        return Err(CodecError::InvalidAlignment);
    }
    let count = reader.u32()?;
    // Untrusted count: grow as entries decode instead of reserving `count` up front.
    let mut entries = Vec::new();
    for _ in 0..count {
        let mime = reader.string()?;
        let value = reader.string()?;
        entries.push((mime, value));
    }
    if reader.pos != bytes.len() {
        return Err(CodecError::LengthMismatch {
            declared,
            actual: reader.pos - 4,
        });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::to_u32;
    use crate::CodecError;

    #[test]
    fn length_conversion_reports_overflow() {
        assert_eq!(to_u32(7), Ok(7));
        assert_eq!(to_u32(u32::MAX as usize), Ok(u32::MAX));
        #[cfg(target_pointer_width = "64")]
        assert_eq!(to_u32(u32::MAX as usize + 1), Err(CodecError::Overflow));
    }
}
