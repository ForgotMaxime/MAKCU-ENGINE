//! Serial transport for MAKCU V4 over the management COM port,
//! with an optional background demux event loop.

pub mod serial;

pub use makcu_core::{
    BatchBuilder, Button, DeviceInfo, LockTarget, Mouse, Position, Screen, Transport,
};
pub use serial::MakcuTransport;
