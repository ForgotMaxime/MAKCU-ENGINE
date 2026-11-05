//! Mouse-only high-level API over any [`Transport`].
//!
//! All validation runs through shared `protocol::check_*` helpers.

use std::sync::Mutex;
use std::time::Duration;

use crate::{
    error::MakcuError,
    protocol::{
        button_method, check_autorelease, check_click, check_mask_bit, check_screen_size,
        check_stream_mode, check_xy, clamp_i16, mask_method, parse_button_event, parse_device,
        parse_getpos, parse_screen,
    },
    BatchBuilder, Button, ButtonOwnership, DeviceInfo, LockTarget, Position, Result, Screen,
    Transport,
};

/// Mouse-only high-level API over any [`Transport`].
pub struct Mouse<T: Transport> {
    t: T,
    stream_mode: Mutex<Option<u8>>,
}

impl<T: Transport> Mouse<T> {
    /// Wrap a connected transport (serial or mock).
    pub fn new(t: T) -> Self {
        Self {
            t,
            stream_mode: Mutex::new(None),
        }
    }

    /// Firmware identity string (`km.MAKCU` on V4).
    pub fn version(&self) -> Result<String> {
        self.t.query("km.version", &[])
    }

    /// Route and report periods (`R:MK;M:..;K:..;C:..`).
    pub fn device(&self) -> Result<DeviceInfo> {
        Ok(parse_device(&self.t.query("km.device", &[])?))
    }

    /// Command echo flag (0 = silent setters).
    pub fn echo(&self) -> Result<i32> {
        self.t
            .query("km.echo", &[])?
            .trim()
            .parse()
            .map_err(|_| MakcuError::Protocol("bad echo".into()))
    }

    /// Enable or disable command echo.
    pub fn set_echo(&self, enabled: bool) -> Result<()> {
        self.t.set("km.echo", &[if enabled { "1" } else { "0" }])
    }

    /// Tracked pointer position estimate.
    pub fn getpos(&self) -> Result<Position> {
        parse_getpos(&self.t.query("km.getpos", &[])?)
    }

    /// Virtual screen size.
    pub fn screen(&self) -> Result<Screen> {
        parse_screen(&self.t.query("km.screen", &[])?)
    }

    /// True while injected movement is still being sent.
    pub fn moving(&self) -> Result<bool> {
        Ok(self.t.query("km.moving", &[])? == "1")
    }

    /// Block until no injected movement remains; false on timeout.
    pub fn wait_idle(&self, timeout: std::time::Duration) -> Result<bool> {
        let poll = std::time::Duration::from_millis(20);
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if !self.moving()? {
                return Ok(true);
            }
            match deadline.checked_duration_since(std::time::Instant::now()) {
                None => return Ok(false),
                Some(remaining) => std::thread::sleep(poll.min(remaining)),
            }
        }
    }

    /// Physical/injected ownership of a button.
    pub fn button_state(&self, b: Button) -> Result<ButtonOwnership> {
        let v: i32 = self
            .t
            .query(&format!("km.{}", button_method(b)), &[])?
            .trim()
            .parse()
            .map_err(|_| MakcuError::Protocol("bad button state".into()))?;
        ButtonOwnership::try_from(v)
    }

    /// Whether a lock target is currently locked.
    pub fn lock_state(&self, t: LockTarget) -> Result<bool> {
        Ok(self.t.query(&format!("km.lock_{}", t.as_str()), &[])? == "1")
    }

    fn i16s(dx: i32, dy: i32) -> Result<(String, String)> {
        Ok((clamp_i16(dx)?.to_string(), clamp_i16(dy)?.to_string()))
    }

    /// Injected relative move (i16 each axis).
    pub fn move_rel(&self, dx: i32, dy: i32) -> Result<()> {
        let (x, y) = Self::i16s(dx, dy)?;
        self.t.set("km.move", &[&x, &y])
    }

    /// Injected move requesting the next free report.
    pub fn move_now(&self, dx: i32, dy: i32) -> Result<()> {
        let (x, y) = Self::i16s(dx, dy)?;
        self.t.set("km.move_now", &[&x, &y])
    }

    /// Absolute move on the virtual screen (0..=32767).
    pub fn moveto(&self, x: i32, y: i32) -> Result<()> {
        check_xy(x, y)?;
        let (xs, ys) = (x.to_string(), y.to_string());
        self.t.set("km.moveto", &[&xs, &ys])
    }

    /// Set the virtual screen; requires `confirm` (moves tracking origin).
    pub fn set_screen(&self, w: i32, h: i32, confirm: bool) -> Result<()> {
        if !confirm {
            return Err(MakcuError::InvalidArgument(
                "set_screen needs confirm=true (changes live tracking)".into(),
            ));
        }
        check_screen_size(w, h)?;
        let (ws, hs) = (w.to_string(), h.to_string());
        self.t.set("km.screen", &[&ws, &hs])
    }

    /// Injected wheel delta (i16).
    pub fn wheel(&self, delta: i32) -> Result<()> {
        let d = clamp_i16(delta)?.to_string();
        self.t.set("km.wheel", &[&d])
    }

    /// Split a relative move into steps so the path looks drawn, not jumped.
    pub fn move_smooth(&self, dx: i32, dy: i32, steps: u32, step: Duration) -> Result<()> {
        if steps < 1 {
            return Err(MakcuError::InvalidArgument(format!("steps: {steps}")));
        }
        let (mut px, mut py) = (0, 0);
        for i in 1..=steps {
            let x = (dx as i64 * i as i64 / steps as i64) as i32;
            let y = (dy as i64 * i as i64 / steps as i64) as i32;
            self.move_rel(x - px, y - py)?;
            px = x;
            py = y;
            if step > Duration::ZERO && i < steps {
                std::thread::sleep(step);
            }
        }
        Ok(())
    }

    /// Press, smooth-move, release. The button always goes back up.
    pub fn drag(&self, b: Button, dx: i32, dy: i32, steps: u32, step: Duration) -> Result<()> {
        self.down(b)?;
        let r = self.move_smooth(dx, dy, steps, step);
        let up = self.up(b, false);
        r.and(up)
    }

    /// Move, silent left click, move back.
    pub fn silent(&self, x: i32, y: i32) -> Result<()> {
        check_xy(x, y)?;
        let (xs, ys) = (x.to_string(), y.to_string());
        self.t.set("km.silent", &[&xs, &ys])
    }

    /// Press a mouse button.
    pub fn down(&self, b: Button) -> Result<()> {
        self.t.set(&format!("km.{}", button_method(b)), &["1"])
    }

    /// Release a mouse button; silent variant skips the frame.
    pub fn up(&self, b: Button, silent_release: bool) -> Result<()> {
        self.t.set(
            &format!("km.{}", button_method(b)),
            &[if silent_release { "2" } else { "0" }],
        )
    }

    /// Scheduled clicks with optional hold time.
    pub fn click(&self, b: Button, count: u32, hold_ms: Option<u32>) -> Result<()> {
        let n = b as u32;
        check_click(n, count, hold_ms)?;
        let (ns, cs) = (n.to_string(), count.to_string());
        if let Some(h) = hold_ms {
            let hs = h.to_string();
            self.t.set("km.click", &[&ns, &cs, &hs])
        } else {
            self.t.set("km.click", &[&ns, &cs])
        }
    }

    /// Block physical movement per direction (each 0/1).
    pub fn set_move_mask(&self, left: i32, right: i32, down: i32, up: i32) -> Result<()> {
        for v in [left, right, down, up] {
            check_mask_bit(v)?;
        }
        let (l, r, d, u) = (
            left.to_string(),
            right.to_string(),
            down.to_string(),
            up.to_string(),
        );
        self.t.set("km.move_mask", &[&l, &r, &d, &u])
    }

    /// Block physical wheel per direction (each 0/1).
    pub fn set_wheel_mask(&self, down: i32, up: i32) -> Result<()> {
        check_mask_bit(down)?;
        check_mask_bit(up)?;
        let (d, u) = (down.to_string(), up.to_string());
        self.t.set("km.wheel_mask", &[&d, &u])
    }

    /// Block a physical button; injection still works.
    pub fn set_button_mask(&self, b: Button, enabled: bool) -> Result<()> {
        self.t.set(
            &format!("km.{}", mask_method(b)),
            &[if enabled { "1" } else { "0" }],
        )
    }

    /// Lock or unlock a button/axis target.
    pub fn set_lock(&self, t: LockTarget, enabled: bool) -> Result<()> {
        self.t.set(
            &format!("km.lock_{}", t.as_str()),
            &[if enabled { "1" } else { "0" }],
        )
    }

    /// Fire-and-forget relative move (no reply wait).
    pub fn move_ff(&self, dx: i32, dy: i32) -> Result<()> {
        let (x, y) = Self::i16s(dx, dy)?;
        self.t.fire_and_forget("km.move", &[&x, &y])
    }

    /// Start a coalesced [`BatchBuilder`] on this transport.
    pub fn batch(&self) -> BatchBuilder<'_, T> {
        BatchBuilder::new(&self.t)
    }

    /// Injected-button autorelease timeout in ms (V4.103+).
    pub fn autorelease(&self) -> Result<u32> {
        self.t
            .query("km.autorelease", &[])?
            .trim()
            .parse()
            .map_err(|_| MakcuError::Protocol("bad autorelease".into()))
    }

    /// Set the autorelease timeout, 0 disables (V4.103+).
    pub fn set_autorelease(&self, timeout_ms: u32) -> Result<()> {
        check_autorelease(timeout_ms)?;
        let v = timeout_ms.to_string();
        self.t.set("km.autorelease", &[&v])
    }

    /// Restart the autorelease timer (V4.103+).
    pub fn heartbeat(&self) -> Result<()> {
        self.t.set("km.heartbeat", &[])
    }

    /// Firmware API version integer (V4.105 reports 1).
    pub fn api_version(&self) -> Result<u32> {
        self.t
            .query("km.api_version", &[])?
            .trim()
            .parse()
            .map_err(|_| MakcuError::Protocol("bad api_version".into()))
    }

    /// Enable physical button-change streaming (modes 1-3).
    pub fn start_button_stream(&self, mode: u8) -> Result<()> {
        check_stream_mode(mode)?;
        self.t.set("km.buttons", &[&mode.to_string()])?;
        *self.stream_mode.lock().unwrap() = Some(mode);
        Ok(())
    }

    /// True while the button stream is enabled on the device.
    pub fn button_stream_state(&self) -> Result<bool> {
        Ok(self.t.query("km.buttons", &[])?.trim() == "1")
    }

    /// Disable button-change streaming.
    pub fn stop_button_stream(&self) -> Result<()> {
        self.t.set("km.buttons", &["0"])?;
        *self.stream_mode.lock().unwrap() = None;
        Ok(())
    }

    /// Next button mask; mode 3 reads one bare byte.
    pub fn poll_button_event(&self, timeout: std::time::Duration) -> Result<u8> {
        match *self.stream_mode.lock().unwrap() {
            None => Err(MakcuError::Protocol(
                "stream not started — call start_button_stream first".into(),
            )),
            Some(3) if self.t.event_loop_running() => Err(MakcuError::Protocol(
                "mode 3 needs the legacy poll path (stop the event loop)".into(),
            )),
            Some(3) => parse_button_event(&self.t.read_exact(1, timeout)?),
            Some(_) if self.t.event_loop_running() => {
                parse_button_event(&self.t.poll_event(timeout)?)
            }
            Some(_) => parse_button_event(&self.t.read_line(timeout)?),
        }
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

    #[test]
    fn mock_batch() {
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        let mut b = m.batch();
        b.move_rel(10, 0).unwrap();
        b.move_rel(-10, 0).unwrap();
        b.wheel(1).unwrap();
        assert_eq!(b.execute().unwrap(), 3);
        assert_eq!(t.sent().len(), 1);
        assert_eq!(t.sent()[0], "km.move(10,0)\rkm.move(-10,0)\rkm.wheel(1)\r");
    }

    #[test]
    fn batch_validates_like_mouse() {
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        let mut b = m.batch();
        assert!(b.click(Button::Left, 0, None).is_err());
        assert!(b.click(Button::Left, 1, Some(6000)).is_err());
        assert!(b.moveto(-1, 0).is_err());
        assert!(b.silent(0, 40000).is_err());
        assert!(b.set_move_mask(0, 0, 0, 2).is_err());
        assert!(b.set_autorelease(60001).is_err());
        assert!(b.is_empty());
        b.move_now(1, 1).unwrap();
        b.moveto(100, 100).unwrap();
        b.silent(100, 100).unwrap();
        b.click(Button::Right, 2, Some(50)).unwrap();
        b.set_move_mask(1, 0, 0, 0).unwrap();
        b.set_wheel_mask(0, 1).unwrap();
        b.set_button_mask(Button::Left, true);
        b.set_autorelease(1000).unwrap();
        b.heartbeat();
        assert_eq!(b.execute().unwrap(), 9);
        assert_eq!(t.sent().len(), 1);
    }

    #[test]
    fn mock_testbuild() {
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        assert_eq!(m.api_version().unwrap(), 1);
        assert_eq!(m.autorelease().unwrap(), 0);
        m.set_autorelease(5000).unwrap();
        m.heartbeat().unwrap();
    }

    #[test]
    fn mock_stream() {
        use std::time::Duration;
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        assert!(m.poll_button_event(Duration::from_millis(10)).is_err());
        m.start_button_stream(1).unwrap();
        t.push_stream_frame(b"km.\x01\r\n");
        assert_eq!(
            m.poll_button_event(Duration::from_millis(100)).unwrap(),
            0x01
        );
        m.stop_button_stream().unwrap();
    }

    #[test]
    fn mock_stream_mode3() {
        use std::time::Duration;
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        m.start_button_stream(3).unwrap();
        t.push_stream_frame(b"\x05");
        assert_eq!(
            m.poll_button_event(Duration::from_millis(100)).unwrap(),
            0x05
        );
        m.stop_button_stream().unwrap();
    }

    #[test]
    fn mock_smooth_and_drag() {
        use std::time::Duration;
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        m.move_smooth(10, 4, 5, Duration::ZERO).unwrap();
        let moves: Vec<_> = t
            .sent()
            .iter()
            .filter(|c| c.starts_with("km.move("))
            .cloned()
            .collect();
        assert_eq!(moves.len(), 5);
        let mut total = (0, 0);
        for c in &moves {
            let inner = &c["km.move(".len()..c.len() - 1];
            let mut it = inner.split(',');
            total.0 += it.next().unwrap().parse::<i32>().unwrap();
            total.1 += it.next().unwrap().parse::<i32>().unwrap();
        }
        assert_eq!(total, (10, 4));
        assert!(m.move_smooth(1, 1, 0, Duration::ZERO).is_err());
        let t2 = MockTransport::new();
        let m2 = Mouse::new(&t2);
        m2.drag(Button::Side1, 6, 0, 3, Duration::ZERO).unwrap();
        let sent = t2.sent();
        assert_eq!(sent[0], "km.ms1(1)");
        assert_eq!(sent[sent.len() - 1], "km.ms1(0)");
    }

    #[test]
    fn mock_stream_loop_path() {
        use std::time::Duration;
        let t = MockTransport::new().with_reply("km.buttons", "1");
        let m = Mouse::new(&t);
        t.start_event_loop().unwrap();
        m.start_button_stream(1).unwrap();
        assert!(m.button_stream_state().unwrap());
        t.push_stream_frame(b"km.\x02\r\n");
        assert_eq!(
            m.poll_button_event(Duration::from_millis(100)).unwrap(),
            0x02
        );
        m.stop_button_stream().unwrap();
        t.stop_event_loop().unwrap();
    }

    #[test]
    fn mock_wait_idle() {
        use std::time::Duration;
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        assert!(m.wait_idle(Duration::from_millis(200)).unwrap());
    }

    #[test]
    fn mock_monitor_pump() {
        use crate::events::ButtonMonitor;
        use std::sync::{Arc, Mutex};
        use std::time::Duration;
        let t = MockTransport::new();
        t.push_stream_frame(b"km.\x03\r\n");
        t.push_stream_frame(b"km.\x03\r\n");
        let m = Arc::new(Mouse::new(t));
        let mon = ButtonMonitor::new(Arc::clone(&m), Duration::from_millis(10));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = Arc::clone(&seen);
        mon.on_change(move |mask| seen2.lock().unwrap().push(mask));
        m.start_button_stream(1).unwrap();
        assert_eq!(
            mon.pump_once(Duration::from_millis(100)).unwrap(),
            Some(0x03)
        );
        assert_eq!(
            mon.pump_once(Duration::from_millis(100)).unwrap(),
            Some(0x03)
        );
        assert_eq!(mon.drain(), vec![0x03]);
        m.stop_button_stream().unwrap();
    }

    #[test]
    fn mock_batch_error_names() {
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        t.fail_next_send();
        let mut b = m.batch();
        b.move_rel(1, 0).unwrap();
        b.wheel(1).unwrap();
        let e = b.execute().unwrap_err();
        let msg = format!("{e}");
        assert!(msg.contains("km.move") && msg.contains("km.wheel"), "{msg}");
    }
}
