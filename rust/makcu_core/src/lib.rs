#![warn(missing_docs)]
//! MAKCU V4 mouse-only core: zero-dependency protocol, types and
//! transport-agnostic mouse API. Pair with `makcu_serial` for hardware I/O.

pub mod batch;
pub mod error;
pub mod events;
pub mod mock;
pub mod mouse;
pub mod protocol;
pub mod transport;
pub mod types;

pub use batch::BatchBuilder;
pub use error::{MakcuError, Result};
pub use mock::MockTransport;
pub use mouse::Mouse;
pub use transport::Transport;
pub use types::{Button, ButtonOwnership, DeviceInfo, LockTarget, Position, Screen};
