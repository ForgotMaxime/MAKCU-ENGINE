use crate::{protocol::build_cmd, Button, LockTarget, Result, Transport};

pub struct BatchBuilder<'a, T: Transport> {
    t: &'a T,
    buf: Vec<u8>,
    count: usize,
}

impl<'a, T: Transport> BatchBuilder<'a, T> {
    pub fn new(t: &'a T) -> Self {
        Self {
            t,
            buf: Vec::new(),
            count: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn add(&mut self, name: &str, args: &[&str]) -> &mut Self {
        self.buf.extend_from_slice(&build_cmd(name, args));
        self.count += 1;
        self
    }

    pub fn move_rel(&mut self, dx: i32, dy: i32) -> Result<&mut Self> {
        let (x, y) = (
            crate::protocol::clamp_i16(dx)?.to_string(),
            crate::protocol::clamp_i16(dy)?.to_string(),
        );
        Ok(self.add("km.move", &[&x, &y]))
    }

    pub fn wheel(&mut self, delta: i32) -> Result<&mut Self> {
        let d = crate::protocol::clamp_i16(delta)?.to_string();
        Ok(self.add("km.wheel", &[&d]))
    }

    pub fn down(&mut self, b: Button) -> &mut Self {
        let m = crate::protocol::button_method(b);
        let cmd = format!("km.{m}");
        let raw = build_cmd(&cmd, &["1"]);
        self.buf.extend_from_slice(&raw);
        self.count += 1;
        self
    }

    pub fn up(&mut self, b: Button) -> &mut Self {
        let m = crate::protocol::button_method(b);
        let cmd = format!("km.{m}");
        let raw = build_cmd(&cmd, &["0"]);
        self.buf.extend_from_slice(&raw);
        self.count += 1;
        self
    }

    pub fn set_lock(&mut self, t: LockTarget, enabled: bool) -> &mut Self {
        let cmd = format!("km.lock_{}", t.as_str());
        let v = if enabled { "1" } else { "0" };
        let raw = build_cmd(&cmd, &[v]);
        self.buf.extend_from_slice(&raw);
        self.count += 1;
        self
    }

    pub fn execute(&mut self) -> Result<usize> {
        let n = self.count;
        if n > 0 {
            self.t.send_raw(&self.buf)?;
        }
        self.buf.clear();
        self.count = 0;
        Ok(n)
    }
}
