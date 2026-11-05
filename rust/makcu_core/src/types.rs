#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Button {
    Left = 1,
    Right = 2,
    Middle = 3,
    Side1 = 4,
    Side2 = 5,
}

impl Button {
    /// V4 alias: `km.ms1` addresses the same side button as `km.side1`.
    pub const MS1: Button = Button::Side1;
    /// V4 alias: `km.ms2` addresses the same side button as `km.side2`.
    pub const MS2: Button = Button::Side2;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ButtonOwnership {
    None = 0,
    Physical = 1,
    Injected = 2,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub route: String,
    pub mouse_period: String,
    pub keyboard_period: String,
    pub controller_period: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockTarget {
    Ml,
    Mr,
    Mm,
    Ms1,
    Ms2,
    Mx,
    My,
    Mw,
    MxPos,
    MxNeg,
    MyPos,
    MyNeg,
    MwPos,
    MwNeg,
}

impl LockTarget {
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
