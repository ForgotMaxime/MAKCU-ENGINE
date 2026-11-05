use std::fmt;

#[derive(Debug)]
pub enum MakcuError {
    Connection(String),
    Timeout,
    Protocol(String),
    InvalidArgument(String),
}

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
