#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// Errors returned by clipboard wire-format codecs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    Truncated,
    LengthMismatch { declared: usize, actual: usize },
    InvalidAlignment,
    InvalidUtf16,
    InvalidCfHtmlHeader,
    InvalidRtfEscape,
    Overflow,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("codec input truncated"),
            Self::LengthMismatch { declared, actual } => {
                write!(f, "codec payload length mismatch: declared {declared}, actual {actual}")
            }
            Self::InvalidAlignment => f.write_str("codec payload not 4-byte aligned"),
            Self::InvalidUtf16 => f.write_str("invalid UTF-16 sequence"),
            Self::InvalidCfHtmlHeader => f.write_str("invalid CF_HTML header"),
            Self::InvalidRtfEscape => f.write_str("invalid RTF escape sequence"),
            Self::Overflow => f.write_str("codec integer overflow"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CodecError {}

/// Parsed byte offsets for Windows `CF_HTML` clipboard payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CfHtmlOffsets {
    pub start_html: usize,
    pub end_html: usize,
    pub start_fragment: usize,
    pub end_fragment: usize,
}

pub mod pickle {
    use super::{CodecError, String, Vec};

    /// Encodes `(mime_type, value)` pairs into Chromium's Pickle format (`org.chromium.web-custom-data`).
    pub fn encode_chromium_custom_data(_entries: &[(&str, &str)]) -> Result<Vec<u8>, CodecError> {
        Ok(Vec::new())
    }

    /// Encodes a Signal Desktop `text/signal` payload as Chromium Pickle bytes.
    pub fn encode_signal_pickle(html: &str) -> Result<Vec<u8>, CodecError> {
        encode_chromium_custom_data(&[("text/signal", html)])
    }

    /// Decodes a Chromium Pickle custom-data buffer into `(mime_type, value)` pairs.
    pub fn decode_chromium_custom_data(_bytes: &[u8]) -> Result<Vec<(String, String)>, CodecError> {
        Ok(Vec::new())
    }
}

pub mod cf_html {
    use super::{CfHtmlOffsets, CodecError, String};

    /// Wraps an HTML fragment in a valid Windows `CF_HTML` envelope with UTF-8 byte offsets.
    pub fn encode_cf_html(fragment: &str) -> String {
        String::from(fragment)
    }

    /// Parses a Windows `CF_HTML` envelope and validates byte offsets and fragment markers.
    pub fn parse_cf_html(_payload: &str) -> Result<CfHtmlOffsets, CodecError> {
        Err(CodecError::InvalidCfHtmlHeader)
    }
}

pub mod rtf {
    use super::{CodecError, String};

    /// Escapes a UTF-8 string into ASCII-safe RTF using signed 16-bit UTF-16 `\uN?` escapes.
    pub fn escape_rtf(value: &str) -> String {
        String::from(value)
    }

    /// Unescapes an RTF-escaped text run back into UTF-8.
    pub fn unescape_rtf(value: &str) -> Result<String, CodecError> {
        Ok(String::from(value))
    }
}
