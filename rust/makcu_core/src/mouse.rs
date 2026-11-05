use crate::{
    error::MakcuError,
    protocol::{button_method, clamp_i16, mask_method, parse_device, parse_getpos, parse_screen},
    Button, ButtonOwnership, DeviceInfo, LockTarget, Position, Result, Screen, Transport,
};

pub struct Mouse<T: Transport> {
    t: T,
}

impl<T: Transport> Mouse<T> {
    pub fn new(t: T) -> Self {
        Self { t }
    }

    pub fn version(&self) -> Result<String> {
        self.t.query("km.version", &[])
    }

    pub fn device(&self) -> Result<DeviceInfo> {
        Ok(parse_device(&self.t.query("km.device", &[])?))
    }

    pub fn echo(&self) -> Result<i32> {
        self.t
            .query("km.echo", &[])?
            .trim()
            .parse()
            .map_err(|_| MakcuError::Protocol("bad echo".into()))
    }

    pub fn set_echo(&self, enabled: bool) -> Result<()> {
        self.t.set("km.echo", &[if enabled { "1" } else { "0" }])
    }

    pub fn getpos(&self) -> Result<Position> {
        parse_getpos(&self.t.query("km.getpos", &[])?)
    }

    pub fn screen(&self) -> Result<Screen> {
        parse_screen(&self.t.query("km.screen", &[])?)
    }

    pub fn moving(&self) -> Result<bool> {
        Ok(self.t.query("km.moving", &[])? == "1")
    }

    pub fn button_state(&self, b: Button) -> Result<ButtonOwnership> {
        let v: i32 = self
            .t
            .query(&format!("km.{}", button_method(b)), &[])?
            .trim()
            .parse()
            .map_err(|_| MakcuError::Protocol("bad button state".into()))?;
        ButtonOwnership::try_from(v)
    }

    pub fn lock_state(&self, t: LockTarget) -> Result<bool> {
        Ok(self.t.query(&format!("km.lock_{}", t.as_str()), &[])? == "1")
    }

    fn i16s(dx: i32, dy: i32) -> Result<(String, String)> {
        Ok((clamp_i16(dx)?.to_string(), clamp_i16(dy)?.to_string()))
    }

    pub fn move_rel(&self, dx: i32, dy: i32) -> Result<()> {
        let (x, y) = Self::i16s(dx, dy)?;
        self.t.set("km.move", &[&x, &y])
    }

    pub fn move_now(&self, dx: i32, dy: i32) -> Result<()> {
        let (x, y) = Self::i16s(dx, dy)?;
        self.t.set("km.move_now", &[&x, &y])
    }

    pub fn moveto(&self, x: i32, y: i32) -> Result<()> {
        if !(0..=32767).contains(&x) {
            return Err(MakcuError::InvalidArgument(format!("x: {x}")));
        }
        if !(0..=32767).contains(&y) {
            return Err(MakcuError::InvalidArgument(format!("y: {y}")));
        }
        let (xs, ys) = (x.to_string(), y.to_string());
        self.t.set("km.moveto", &[&xs, &ys])
    }

    pub fn set_screen(&self, w: i32, h: i32) -> Result<()> {
        if !(1..=32767).contains(&w) || !(1..=32767).contains(&h) {
            return Err(MakcuError::InvalidArgument("screen range".into()));
        }
        let (ws, hs) = (w.to_string(), h.to_string());
        self.t.set("km.screen", &[&ws, &hs])
    }

    pub fn wheel(&self, delta: i32) -> Result<()> {
        let d = clamp_i16(delta)?.to_string();
        self.t.set("km.wheel", &[&d])
    }

    pub fn silent(&self, x: i32, y: i32) -> Result<()> {
        let (xs, ys) = (x.to_string(), y.to_string());
        self.t.set("km.silent", &[&xs, &ys])
    }

    pub fn down(&self, b: Button) -> Result<()> {
        self.t.set(&format!("km.{}", button_method(b)), &["1"])
    }

    pub fn up(&self, b: Button, silent_release: bool) -> Result<()> {
        self.t.set(
            &format!("km.{}", button_method(b)),
            &[if silent_release { "2" } else { "0" }],
        )
    }

    pub fn click(&self, b: Button, count: u32, hold_ms: Option<u32>) -> Result<()> {
        let n = b as u32;
        if !(1..=5).contains(&n) {
            return Err(MakcuError::InvalidArgument("button".into()));
        }
        if !(1..=255).contains(&count) {
            return Err(MakcuError::InvalidArgument("count".into()));
        }
        let (ns, cs) = (n.to_string(), count.to_string());
        if let Some(h) = hold_ms {
            if !(1..=5000).contains(&h) {
                return Err(MakcuError::InvalidArgument("hold_ms".into()));
            }
            let hs = h.to_string();
            self.t.set("km.click", &[&ns, &cs, &hs])
        } else {
            self.t.set("km.click", &[&ns, &cs])
        }
    }

    fn check_mask(v: i32) -> Result<String> {
        if v != 0 && v != 1 {
            return Err(MakcuError::InvalidArgument("mask 0/1".into()));
        }
        Ok(v.to_string())
    }

    pub fn set_move_mask(&self, left: i32, right: i32, down: i32, up: i32) -> Result<()> {
        let (l, r, d, u) = (
            Self::check_mask(left)?,
            Self::check_mask(right)?,
            Self::check_mask(down)?,
            Self::check_mask(up)?,
        );
        self.t.set("km.move_mask", &[&l, &r, &d, &u])
    }

    pub fn set_wheel_mask(&self, down: i32, up: i32) -> Result<()> {
        let (d, u) = (Self::check_mask(down)?, Self::check_mask(up)?);
        self.t.set("km.wheel_mask", &[&d, &u])
    }

    pub fn set_button_mask(&self, b: Button, enabled: bool) -> Result<()> {
        self.t.set(
            &format!("km.{}", mask_method(b)),
            &[if enabled { "1" } else { "0" }],
        )
    }

    pub fn set_lock(&self, t: LockTarget, enabled: bool) -> Result<()> {
        self.t.set(
            &format!("km.lock_{}", t.as_str()),
            &[if enabled { "1" } else { "0" }],
        )
    }

    pub fn move_ff(&self, dx: i32, dy: i32) -> Result<()> {
        let (x, y) = Self::i16s(dx, dy)?;
        self.t.fire_and_forget("km.move", &[&x, &y])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MockTransport;

    #[test]
    fn mock_flow() {
        let t = MockTransport::new();
        let m = Mouse::new(t);
        assert_eq!(m.version().unwrap(), "km.MAKCU");
        assert_eq!(m.device().unwrap().route, "MK");
        assert!(!m.moving().unwrap());
        m.move_rel(10, -5).unwrap();
        m.down(Button::Left).unwrap();
        m.up(Button::Left, false).unwrap();
        m.click(Button::Left, 2, Some(50)).unwrap();
    }
}
