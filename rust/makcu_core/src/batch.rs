//! Coalesced setter batch: one write, one ERR-window read.
//!
//! Same setter set and validation as [`Mouse`](crate::Mouse); queries refused.

use crate::{
    protocol::{
        build_cmd, button_method, check_autorelease, check_click, check_mask_bit, check_xy,
        clamp_i16, mask_method,
    },
    Button, LockTarget, MakcuError, Result, Transport,
};

/// Coalesced setter batch: one write, one ERR-window read.
/// Same setter set and validation as [`crate::Mouse`]; queries refused.
/// Coalesced setter batch: one write, one ERR-window read.
pub struct BatchBuilder<'a, T: Transport> {
    t: &'a T,
    buf: Vec<u8>,
    names: Vec<String>,
    count: usize,
}

impl<'a, T: Transport> BatchBuilder<'a, T> {
    /// Borrow a transport for batched writes.
    pub fn new(t: &'a T) -> Self {
        Self {
            t,
            buf: Vec::new(),
            names: Vec::new(),
            count: 0,
        }
    }

    /// Number of queued commands.
    pub fn len(&self) -> usize {
        self.count
    }

    /// True when nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn add(&mut self, name: &str, args: &[&str]) -> &mut Self {
        let raw = build_cmd(name, args);
        self.add_raw(name, &raw)
    }

    fn add_raw(&mut self, name: &str, raw: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(raw);
        self.names.push(name.to_string());
        self.count += 1;
        self
    }

    fn i16arg(v: i32) -> Result<String> {
        Ok(clamp_i16(v)?.to_string())
    }

    /// Queue a relative move.
    pub fn move_rel(&mut self, dx: i32, dy: i32) -> Result<&mut Self> {
        let (x, y) = (Self::i16arg(dx)?, Self::i16arg(dy)?);
        Ok(self.add("km.move", &[&x, &y]))
    }

    /// Queue an immediate relative move.
    pub fn move_now(&mut self, dx: i32, dy: i32) -> Result<&mut Self> {
        let (x, y) = (Self::i16arg(dx)?, Self::i16arg(dy)?);
        Ok(self.add("km.move_now", &[&x, &y]))
    }

    /// Queue an absolute move on the virtual screen.
    pub fn moveto(&mut self, x: i32, y: i32) -> Result<&mut Self> {
        check_xy(x, y)?;
        let (xs, ys) = (x.to_string(), y.to_string());
        Ok(self.add("km.moveto", &[&xs, &ys]))
    }

    /// Queue a move, silent left click, move back.
    pub fn silent(&mut self, x: i32, y: i32) -> Result<&mut Self> {
        check_xy(x, y)?;
        let (xs, ys) = (x.to_string(), y.to_string());
        Ok(self.add("km.silent", &[&xs, &ys]))
    }

    /// Queue a wheel step.
    pub fn wheel(&mut self, delta: i32) -> Result<&mut Self> {
        let d = Self::i16arg(delta)?;
        Ok(self.add("km.wheel", &[&d]))
    }

    /// Queue a button press.
    pub fn down(&mut self, b: Button) -> &mut Self {
        let cmd = format!("km.{}", button_method(b));
        let raw = build_cmd(&cmd, &["1"]);
        self.add_raw(&cmd, &raw)
    }

    /// Queue a button release.
    pub fn up(&mut self, b: Button) -> &mut Self {
        let cmd = format!("km.{}", button_method(b));
        let raw = build_cmd(&cmd, &["0"]);
        self.add_raw(&cmd, &raw)
    }

    /// Queue a scheduled click; validated like `Mouse::click`.
    pub fn click(&mut self, b: Button, count: u32, hold_ms: Option<u32>) -> Result<&mut Self> {
        let n = b as u32;
        check_click(n, count, hold_ms)?;
        let (ns, cs) = (n.to_string(), count.to_string());
        if let Some(h) = hold_ms {
            let hs = h.to_string();
            Ok(self.add("km.click", &[&ns, &cs, &hs]))
        } else {
            Ok(self.add("km.click", &[&ns, &cs]))
        }
    }

    /// Queue a physical move-direction mask (each 0/1).
    pub fn set_move_mask(
        &mut self,
        left: i32,
        right: i32,
        down: i32,
        up: i32,
    ) -> Result<&mut Self> {
        for v in [left, right, down, up] {
            check_mask_bit(v)?;
        }
        let (l, r, d, u) = (
            left.to_string(),
            right.to_string(),
            down.to_string(),
            up.to_string(),
        );
        Ok(self.add("km.move_mask", &[&l, &r, &d, &u]))
    }

    /// Queue a physical wheel-direction mask (each 0/1).
    pub fn set_wheel_mask(&mut self, down: i32, up: i32) -> Result<&mut Self> {
        check_mask_bit(down)?;
        check_mask_bit(up)?;
        let (d, u) = (down.to_string(), up.to_string());
        Ok(self.add("km.wheel_mask", &[&d, &u]))
    }

    /// Queue a physical button mask.
    pub fn set_button_mask(&mut self, b: Button, enabled: bool) -> &mut Self {
        let cmd = format!("km.{}", mask_method(b));
        let v = if enabled { "1" } else { "0" };
        let raw = build_cmd(&cmd, &[v]);
        self.add_raw(&cmd, &raw)
    }

    /// Queue an input lock on a button/axis target.
    pub fn set_lock(&mut self, t: LockTarget, enabled: bool) -> &mut Self {
        let cmd = format!("km.lock_{}", t.as_str());
        let v = if enabled { "1" } else { "0" };
        let raw = build_cmd(&cmd, &[v]);
        self.add_raw(&cmd, &raw)
    }

    /// Queue an autorelease timeout in ms (V4.103+, test builds).
    pub fn set_autorelease(&mut self, timeout_ms: u32) -> Result<&mut Self> {
        check_autorelease(timeout_ms)?;
        let v = timeout_ms.to_string();
        Ok(self.add("km.autorelease", &[&v]))
    }

    /// Queue an autorelease keep-alive (no reply).
    pub fn heartbeat(&mut self) -> &mut Self {
        let raw = build_cmd("km.heartbeat", &[]);
        self.add_raw("km.heartbeat", &raw)
    }

    /// Flush as one write + one ERR-window read; returns flushed count.
    pub fn execute(&mut self) -> Result<usize> {
        let n = self.count;
        if n > 0 {
            let names = self.names.join(",");
            if let Err(e) = self.t.send_raw(&self.buf) {
                return Err(match e {
                    MakcuError::Protocol(msg) => {
                        MakcuError::Protocol(format!("batch [{names}] failed: {msg}"))
                    }
                    other => other,
                });
            }
        }
        self.buf.clear();
        self.names.clear();
        self.count = 0;
        Ok(n)
    }
}
