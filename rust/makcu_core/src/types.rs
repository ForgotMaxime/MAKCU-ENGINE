//! Mouse-only domain types. No I/O here.

//! Mouse-only domain types. No I/O here.

/// Physical mouse buttons (1-5, as used by `km.click`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Button {
    /// Left button (1).
    Left = 1,
    /// Right button (2).
    Right = 2,
    /// Middle button (3).
    Middle = 3,
    /// Side button 1 (4).
    Side1 = 4,
    /// Side button 2 (5).
    Side2 = 5,
}

impl Button {
    /// V4 alias: `km.ms1` addresses the same side button as `km.side1`.
    pub const MS1: Button = Button::Side1;
    /// V4 alias: `km.ms2` addresses the same side button as `km.side2`.
    pub const MS2: Button = Button::Side2;
}

/// Physical vs injected button ownership, as reported by `km.left()` & co.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ButtonOwnership {
    /// Nobody holds the button.
    None = 0,
    /// Held by the physical device only.
    Physical = 1,
    /// Held by injected API state only.
    Injected = 2,
    /// Held by both physical and injected state.
    Both = 3,
}

impl TryFrom<i32> for ButtonOwnership {
    type Error = crate::MakcuError;
    fn try_from(v: i32) -> crate::Result<Self> {
        match v {
            0 => Ok(Self::None),
            1 => Ok(Self::Physical),
            2 => Ok(Self::Injected),
            3 => Ok(Self::Both),
            _ => Err(crate::MakcuError::Protocol(format!("bad ownership: {v}"))),
        }
    }
}

/// Route and report periods from `km.device()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Active route (e.g. `MK`).
    pub route: String,
    /// Mouse report period string (e.g. `8uf`).
    pub mouse_period: String,
    /// Keyboard report period string.
    pub keyboard_period: String,
    /// Controller report period string (`0uf` = none attached).
    pub controller_period: String,
}

/// Virtual screen size from `km.screen()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    /// Width in virtual pixels (1..=32767).
    pub width: u16,
    /// Height in virtual pixels (1..=32767).
    pub height: u16,
}

/// Tracked pointer position from `km.getpos()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// X coordinate.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
}

/// Lockable buttons and axes (`km.lock_*` targets).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockTarget {
    /// Left button.
    Ml,
    /// Right button.
    Mr,
    /// Middle button.
    Mm,
    /// Side button 1.
    Ms1,
    /// Side button 2.
    Ms2,
    /// Whole X axis.
    Mx,
    /// Whole Y axis.
    My,
    /// Whole wheel axis.
    Mw,
    /// Positive X only.
    MxPos,
    /// Negative X only.
    MxNeg,
    /// Positive Y only.
    MyPos,
    /// Negative Y only.
    MyNeg,
    /// Positive wheel only.
    MwPos,
    /// Negative wheel only.
    MwNeg,
}

impl LockTarget {
    /// Wire name used in `km.lock_<name>`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ml => "ml",
            Self::Mr => "mr",
            Self::Mm => "mm",
            Self::Ms1 => "ms1",
            Self::Ms2 => "ms2",
            Self::Mx => "mx",
            Self::My => "my",
            Self::Mw => "mw",
            Self::MxPos => "mx+",
            Self::MxNeg => "mx-",
            Self::MyPos => "my+",
            Self::MyNeg => "my-",
            Self::MwPos => "mw+",
            Self::MwNeg => "mw-",
        }
    }
}
