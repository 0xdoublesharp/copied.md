//! Clipboard wire-format codecs shared by every copied.md surface.
//!
//! * [`pickle`]: Chromium Pickle custom data (`org.chromium.web-custom-data`), used for Signal.
//! * [`cf_html`]: the Windows `CF_HTML` envelope with UTF-8 byte offsets.
//! * [`rtf`]: ASCII-safe RTF text escaping with signed UTF-16 `\uN?` escapes.
//!
//! The crate is `no_std + alloc`. The Lean models in `formal/lean` prove the matching
//! properties, and `tests/lean_vectors.rs` checks this crate against their output.
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

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
                write!(
                    f,
                    "codec payload length mismatch: declared {declared}, actual {actual}"
                )
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
///
/// Both HTML offsets are `None` when the payload contains only a fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CfHtmlOffsets {
    pub start_html: Option<usize>,
    pub end_html: Option<usize>,
    pub start_fragment: usize,
    pub end_fragment: usize,
}

pub mod cf_html;
pub mod pickle;
pub mod rtf;
