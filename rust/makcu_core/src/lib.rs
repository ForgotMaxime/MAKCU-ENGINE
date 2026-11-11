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
pub use mouse::{ClickProfile, Mouse, CLICK_PROFILES};
pub use transport::{ConnectionCallback, NoWaitGuard, Transport, TransportStats, RECENT_CAPACITY};
pub use types::{
    remap_point, scale_coordinates, to_local, to_virtual, virtual_bounds, Bounds, Button,
    ButtonOwnership, CoordSpace, DeviceInfo, LockTarget, Monitor, Position, Screen,
};
