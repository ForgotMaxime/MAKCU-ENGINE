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
    /// Every canonical button in wire order (LEFT..SIDE2).
    pub const ALL: [Button; 5] = [
        Button::Left,
        Button::Right,
        Button::Middle,
        Button::Side1,
        Button::Side2,
    ];
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
    /// Every lock target in canonical order (matches Python `LockTarget` iteration).
    pub const ALL: [LockTarget; 14] = [
        Self::Ml,
        Self::Mr,
        Self::Mm,
        Self::Ms1,
        Self::Ms2,
        Self::Mx,
        Self::My,
        Self::Mw,
        Self::MxPos,
        Self::MxNeg,
        Self::MyPos,
        Self::MyNeg,
        Self::MwPos,
        Self::MwNeg,
    ];

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

/// One display in virtual-desktop coordinates.
///
/// `x`/`y` is the monitor origin (negative when placed left/above the
/// primary), `w`/`h` its size in physical pixels, `scale` the DPI factor
/// (physical pixels per logical unit). Mirrors the Python `Monitor` dataclass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Monitor {
    /// Monitor origin X in virtual-desktop logical units.
    pub x: i32,
    /// Monitor origin Y in virtual-desktop logical units.
    pub y: i32,
    /// Monitor width in physical pixels.
    pub w: i32,
    /// Monitor height in physical pixels.
    pub h: i32,
    /// DPI factor: physical pixels per logical unit (must be > 0).
    pub scale: f64,
}

/// Bounding box over monitors: origin + size in logical units.
///
/// Mirrors the Python `Bounds` dataclass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// Left edge in virtual-desktop logical units.
    pub x: i32,
    /// Top edge in virtual-desktop logical units.
    pub y: i32,
    /// Width in logical units.
    pub w: i32,
    /// Height in logical units.
    pub h: i32,
}

/// Bounding box spanning all monitors; negative origins supported.
///
/// Mirrors Python `virtual_bounds` (`ValueError` on empty becomes
/// [`MakcuError::InvalidArgument`]).
/// Rounding follows Rust `f64::round` (half away from zero), while Python
/// uses banker's rounding — exact ties can differ by 1.
/// A non-positive monitor `scale` is rejected (Python would divide by zero).
pub fn virtual_bounds(monitors: &[Monitor]) -> crate::Result<Bounds> {
    if monitors.is_empty() {
        return Err(crate::MakcuError::InvalidArgument(
            "virtual_bounds needs at least one monitor".into(),
        ));
    }
    for m in monitors {
        if m.scale <= 0.0 || !m.scale.is_finite() {
            return Err(crate::MakcuError::InvalidArgument(format!(
                "scale out of range: {}",
                m.scale
            )));
        }
    }
    let left = monitors.iter().map(|m| m.x).min().unwrap_or(0);
    let top = monitors.iter().map(|m| m.y).min().unwrap_or(0);
    let right = monitors
        .iter()
        .map(|m| f64::from(m.x) + f64::from(m.w) / m.scale)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = monitors
        .iter()
        .map(|m| f64::from(m.y) + f64::from(m.h) / m.scale)
        .fold(f64::NEG_INFINITY, f64::max);
    Ok(Bounds {
        x: left,
        y: top,
        w: (right - f64::from(left)).round() as i32,
        h: (bottom - f64::from(top)).round() as i32,
    })
}

/// Monitor-local physical pixel to virtual-desktop logical point.
///
/// Mirrors Python `to_virtual` (`round` semantics differ on exact ties;
/// see [`virtual_bounds`]).
pub fn to_virtual(x: i32, y: i32, monitor: &Monitor) -> Position {
    Position {
        x: monitor.x + (f64::from(x) / monitor.scale).round() as i32,
        y: monitor.y + (f64::from(y) / monitor.scale).round() as i32,
    }
}

/// Virtual-desktop logical point to monitor-local physical pixel.
///
/// Mirrors Python `to_local` (`round` semantics differ on exact ties;
/// see [`virtual_bounds`]).
pub fn to_local(x: i32, y: i32, monitor: &Monitor) -> Position {
    Position {
        x: ((f64::from(x - monitor.x)) * monitor.scale).round() as i32,
        y: ((f64::from(y - monitor.y)) * monitor.scale).round() as i32,
    }
}

/// Re-express a `src`-local pixel as a `dst`-local pixel via virtual space.
///
/// Pure composition of [`to_virtual`] and [`to_local`]; mirrors Python
/// `remap_point`.
pub fn remap_point(x: i32, y: i32, src: &Monitor, dst: &Monitor) -> Position {
    let v = to_virtual(x, y, src);
    to_local(v.x, v.y, dst)
}

/// Coordinate space tag (no global state).
///
/// Mirrors the Python `CoordSpace` enum (`desktop` / `game`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordSpace {
    /// Desktop pixel space.
    Desktop,
    /// Game count space.
    Game,
}

impl CoordSpace {
    /// Wire string for this space (`desktop` / `game`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Game => "game",
        }
    }
}

/// Scale desktop pixels to game counts: `round(v * game_dpi / desktop_dpi * sens)`.
///
/// Pure helper; all factors must be > 0 (mirrors Python `ValueError` checks
/// with [`MakcuError::InvalidArgument`]). Rounding follows Rust
/// `f64::round` (half away from zero); Python uses banker's rounding.
pub fn scale_coordinates(
    x: i32,
    y: i32,
    game_dpi: f64,
    desktop_dpi: f64,
    sens: f64,
) -> crate::Result<(i32, i32)> {
    if game_dpi <= 0.0 || !game_dpi.is_finite() {
        return Err(crate::MakcuError::InvalidArgument(format!(
            "game_dpi out of range: {game_dpi}"
        )));
    }
    if desktop_dpi <= 0.0 || !desktop_dpi.is_finite() {
        return Err(crate::MakcuError::InvalidArgument(format!(
            "desktop_dpi out of range: {desktop_dpi}"
        )));
    }
    if sens <= 0.0 || !sens.is_finite() {
        return Err(crate::MakcuError::InvalidArgument(format!(
            "sens out of range: {sens}"
        )));
    }
    let factor = game_dpi / desktop_dpi * sens;
    Ok((
        (f64::from(x) * factor).round() as i32,
        (f64::from(y) * factor).round() as i32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coord_space_names() {
        assert_eq!(CoordSpace::Desktop.as_str(), "desktop");
        assert_eq!(CoordSpace::Game.as_str(), "game");
    }

    #[test]
    fn scale_identity_and_validation() {
        assert_eq!(
            scale_coordinates(10, -4, 1000.0, 1000.0, 1.0).unwrap(),
            (10, -4)
        );
        assert_eq!(
            scale_coordinates(10, 10, 2000.0, 1000.0, 0.5).unwrap(),
            (10, 10)
        );
        assert!(scale_coordinates(1, 1, 0.0, 1.0, 1.0).is_err());
        assert!(scale_coordinates(1, 1, 1.0, -1.0, 1.0).is_err());
        assert!(scale_coordinates(1, 1, 1.0, 1.0, 0.0).is_err());
    }

    #[test]
    fn monitor_math_roundtrip() {
        let m = Monitor {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
            scale: 1.0,
        };
        assert_eq!(to_virtual(960, 540, &m), Position { x: 960, y: 540 });
        assert_eq!(to_local(960, 540, &m), Position { x: 960, y: 540 });
        let hi = Monitor {
            x: 1920,
            y: 0,
            w: 3840,
            h: 2160,
            scale: 2.0,
        };
        // 3840 physical / 2 = 1920 logical wide.
        assert_eq!(to_virtual(3840, 2160, &hi), Position { x: 3840, y: 1080 });
        assert_eq!(remap_point(0, 0, &m, &hi), to_local(0, 0, &hi));
    }

    #[test]
    fn bounds_span_negative_origins() {
        let ms = [
            Monitor {
                x: -1920,
                y: 0,
                w: 1920,
                h: 1080,
                scale: 1.0,
            },
            Monitor {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
                scale: 1.0,
            },
        ];
        assert_eq!(
            virtual_bounds(&ms).unwrap(),
            Bounds {
                x: -1920,
                y: 0,
                w: 3840,
                h: 1080
            }
        );
        assert!(virtual_bounds(&[]).is_err());
    }
}
