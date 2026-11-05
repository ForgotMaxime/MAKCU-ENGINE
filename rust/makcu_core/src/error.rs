//! Typed errors for every MAKCU failure mode.

use std::fmt;

/// All failures the driver can report.
#[derive(Debug)]
pub enum MakcuError {
    /// Serial port unavailable or I/O failure (holds the OS message).
    Connection(String),
    /// No reply within the deadline.
    Timeout,
    /// Framing, prompt or validation failure (holds details).
    Protocol(String),
    /// Caller-side argument out of range (holds details).
    InvalidArgument(String),
}

/// `Result` alias used across both crates.
pub type Result<T> = std::result::Result<T, MakcuError>;

impl fmt::Display for MakcuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection(s) => write!(f, "connection: {s}"),
            Self::Timeout => write!(f, "timeout: no reply"),
            Self::Protocol(s) => write!(f, "protocol: {s}"),
            Self::InvalidArgument(s) => write!(f, "invalid argument: {s}"),
        }
    }
}

impl std::error::Error for MakcuError {}
