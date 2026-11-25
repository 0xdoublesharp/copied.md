//! Native copied.md command line and stdio MCP entry points.

pub mod cli;
pub mod mcp;

mod pasteboard_compat;
mod picker;

#[cfg(target_os = "linux")]
mod linux_owner;
