//! Serial transport for MAKCU V4 over the management COM port,
//! with an optional background demux event loop.

pub mod serial;

pub use makcu_core::{
    BatchBuilder, Button, ButtonOwnership, ConnectionCallback, DeviceInfo, LockTarget, Mouse,
    NoWaitGuard, Position, ProfileEntry, Screen, Transport, TransportStats, RECENT_CAPACITY,
};
pub use serial::MakcuTransport;
