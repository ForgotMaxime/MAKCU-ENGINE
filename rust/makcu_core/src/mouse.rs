//! Mouse-only high-level API over any [`Transport`].
//!
//! All validation runs through shared `protocol::check_*` helpers.

use std::sync::Mutex;
use std::time::Duration;

use crate::{
    error::MakcuError,
    protocol::{
        button_method, check_autorelease, check_bezier, check_click, check_mask_bit,
        check_screen_size, check_stream_mode, check_xy, clamp_i16, mask_method, parse_button_event,
        parse_device, parse_getpos, parse_screen,
    },
    BatchBuilder, Button, ButtonOwnership, DeviceInfo, LockTarget, Position, Result, Screen,
    Transport,
};

/// Human-like click timing profile in ms: `(down_min, down_max, wait_min, wait_max)`.
///
/// Mirrors the Python `CLICK_PROFILES` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClickProfile {
    /// Minimum button-hold time in ms.
    pub down_min_ms: u32,
    /// Maximum button-hold time in ms.
    pub down_max_ms: u32,
    /// Minimum inter-click wait in ms.
    pub wait_min_ms: u32,
    /// Maximum inter-click wait in ms.
    pub wait_max_ms: u32,
}

/// Named human-like click timing profiles in ms.
///
/// Mirrors the Python `CLICK_PROFILES` table exactly.
pub const CLICK_PROFILES: &[(&str, ClickProfile)] = &[
    (
        "normal",
        ClickProfile {
            down_min_ms: 60,
            down_max_ms: 120,
            wait_min_ms: 100,
            wait_max_ms: 180,
        },
    ),
    (
        "fast",
        ClickProfile {
            down_min_ms: 30,
            down_max_ms: 60,
            wait_min_ms: 50,
            wait_max_ms: 100,
        },
    ),
    (
        "slow",
        ClickProfile {
            down_min_ms: 100,
            down_max_ms: 180,
            wait_min_ms: 150,
            wait_max_ms: 300,
        },
    ),
    (
        "variable",
        ClickProfile {
            down_min_ms: 40,
            down_max_ms: 200,
            wait_min_ms: 80,
            wait_max_ms: 250,
        },
    ),
    (
        "gaming",
        ClickProfile {
            down_min_ms: 20,
            down_max_ms: 40,
            wait_min_ms: 30,
            wait_max_ms: 60,
        },
    ),
];

/// Look up a [`CLICK_PROFILES`] entry by name.
fn lookup_profile(name: &str) -> Result<ClickProfile> {
    CLICK_PROFILES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, p)| *p)
        .ok_or_else(|| MakcuError::InvalidArgument(format!("unknown click profile: {name:?}")))
}

/// Python-compatible `round()` for one interpolated cumulative step.
///
/// CPython `round(float)` rounds ties to even; Rust `f64::round` rounds ties
/// away from zero. `f64::round_ties_even` matches CPython on the same `f64`
/// quotient (both are IEEE-754 doubles), e.g. `-2.5` goes to `-2`.
fn py_round(v: f64) -> i32 {
    v.round_ties_even() as i32
}

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
    ///
    /// Convenience wrapper with the Python default poll (20 ms); see
    /// [`wait_idle_with_poll`](Self::wait_idle_with_poll) for a tunable
    /// poll. Negative polls are unrepresentable (`Duration` is unsigned),
    /// so the Python `poll_s < 0` validation has no Rust counterpart.
    pub fn wait_idle(&self, timeout: std::time::Duration) -> Result<bool> {
        self.wait_idle_with_poll(timeout, std::time::Duration::from_millis(20))
    }

    /// Block until no injected movement remains, polling every `poll`.
    ///
    /// Returns `true` as soon as `moving()` reports idle, `false` on
    /// timeout. Mirrors Python `wait_idle(timeout_s, poll_s)`: each
    /// iteration sleeps `min(poll, remaining)` so the call never exceeds
    /// `timeout` by more than one final `moving()` round-trip.
    pub fn wait_idle_with_poll(
        &self,
        timeout: std::time::Duration,
        poll: std::time::Duration,
    ) -> Result<bool> {
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

    /// Whether a lock target is currently locked (alias of [`lock_state`](Self::lock_state)).
    ///
    /// Stateless single query.
    pub fn is_locked(&self, t: LockTarget) -> Result<bool> {
        self.lock_state(t)
    }

    /// Every lock target keyed by wire name, in [`LockTarget::ALL`] order.
    ///
    /// Stateless, one query per target; mirrors Python `get_all_lock_states`.
    pub fn get_all_lock_states(&self) -> Result<Vec<(&'static str, bool)>> {
        let mut out = Vec::with_capacity(LockTarget::ALL.len());
        for t in LockTarget::ALL {
            out.push((t.as_str(), self.lock_state(t)?));
        }
        Ok(out)
    }

    /// Ownership of every canonical button, in [`Button::ALL`] order.
    ///
    /// Stateless, one query per button; mirrors Python `get_button_states`.
    pub fn get_button_states(&self) -> Result<[(Button, ButtonOwnership); 5]> {
        let mut out = [(Button::Left, ButtonOwnership::None); 5];
        for (i, b) in Button::ALL.iter().enumerate() {
            out[i] = (*b, self.button_state(*b)?);
        }
        Ok(out)
    }

    /// Injected-hold bitmask over canonical buttons (bit `1 << (value-1)`).
    ///
    /// A bit is set when that button's ownership is INJECTED or BOTH;
    /// physical-only holds do not set bits. Stateless, one query per button.
    pub fn get_button_mask(&self) -> Result<u8> {
        let mut mask: u8 = 0;
        for (b, ownership) in self.get_button_states()? {
            if matches!(ownership, ButtonOwnership::Injected | ButtonOwnership::Both) {
                mask |= 1 << ((b as u8) - 1);
            }
        }
        Ok(mask)
    }

    /// Whether injected code currently holds a button. Stateless single query.
    pub fn is_pressed(&self, b: Button) -> Result<bool> {
        Ok(matches!(
            self.button_state(b)?,
            ButtonOwnership::Injected | ButtonOwnership::Both
        ))
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
    ///
    /// Cumulative targets use Python-compatible `round()` (ties go to even,
    /// via `f64::round_ties_even`): each step sends the delta between
    /// successive rounded cumulative positions, so the deltas always sum to
    /// exactly `(dx, dy)`. This differs from plain integer truncation
    /// (`dx * i / steps`), which skews negative moves toward zero, and from
    /// Rust `f64::round` (half away from zero): exact `.5` ties round to the
    /// even neighbor here, e.g. `-2.5` goes to `-2`, matching CPython.
    /// Residual differences vs CPython can only come from platform float
    /// formatting edge cases: the quotient is computed as an `f64` division
    /// (same IEEE-754 double as CPython) and rounded half-to-even, so
    /// representable values match bit-for-bit.
    pub fn move_smooth(&self, dx: i32, dy: i32, steps: u32, step: Duration) -> Result<()> {
        if steps < 1 {
            return Err(MakcuError::InvalidArgument(format!("steps: {steps}")));
        }
        let (mut px, mut py) = (0, 0);
        for i in 1..=steps {
            let x = py_round(dx as f64 * f64::from(i) / f64::from(steps));
            let y = py_round(dy as f64 * f64::from(i) / f64::from(steps));
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

    /// Press a mouse button (alias of [`down`](Self::down)).
    pub fn press(&self, b: Button) -> Result<()> {
        self.down(b)
    }

    /// Release a mouse button; silent variant skips the frame.
    ///
    /// Verdict on `button_up_force` (no Python counterpart, no hardware to
    /// probe): no separate force API exists. `up(b, true)` sends the
    /// firmware silent-release (`... 2`) frame — the only force path — and
    /// matches Python `up(silent_release=True)` exactly.
    pub fn up(&self, b: Button, silent_release: bool) -> Result<()> {
        self.t.set(
            &format!("km.{}", button_method(b)),
            &[if silent_release { "2" } else { "0" }],
        )
    }

    /// Release a mouse button (alias of [`up`](Self::up)).
    pub fn release(&self, b: Button, silent_release: bool) -> Result<()> {
        self.up(b, silent_release)
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

    /// Two clicks with one sleep between them.
    ///
    /// Mirrors Python `double_click(button, gap_s)`; the float-seconds gap
    /// becomes a [`Duration`] here, so negative gaps are unrepresentable
    /// (Python rejects `gap_s < 0`).
    pub fn double_click(&self, b: Button, gap: Duration) -> Result<()> {
        self.click(b, 1, None)?;
        if gap > Duration::ZERO {
            std::thread::sleep(gap);
        }
        self.click(b, 1, None)
    }

    /// Human-like clicks: randomized hold/inter-click sleeps per profile.
    ///
    /// The caller supplies `rng`, a uniform `[0, 1)` sampler (e.g. a small
    /// LCG in tests or `rand::random` behind a feature in your app). The
    /// core stays zero-dependency, so no RNG lives here — this also keeps
    /// timing tests deterministic. Jitter draws uniform integers from
    /// `-jitter..=jitter` per axis, matching Python
    /// `random.randint(-jitter, jitter)`; hold/wait draws are uniform over
    /// the profile's millisecond window.
    ///
    /// Mirrors Python validation: unknown profile, `count` outside 1..=255,
    /// or negative `jitter` all fail.
    pub fn click_human_like<F>(
        &self,
        b: Button,
        count: u32,
        profile: &str,
        jitter: i32,
        rng: &mut F,
    ) -> Result<()>
    where
        F: FnMut() -> f64,
    {
        let p = lookup_profile(profile)?;
        if !(1..=255).contains(&count) {
            return Err(MakcuError::InvalidArgument(format!("count: {count}")));
        }
        if jitter < 0 {
            return Err(MakcuError::InvalidArgument(format!("jitter: {jitter}")));
        }
        for i in 0..count {
            if jitter > 0 {
                let span = f64::from(jitter) * 2.0 + 1.0;
                let jx = (rng() * span).floor() as i32 - jitter;
                let jy = (rng() * span).floor() as i32 - jitter;
                if jx != 0 || jy != 0 {
                    self.move_rel(jx, jy)?;
                }
            }
            self.down(b)?;
            let hold_ms =
                f64::from(p.down_min_ms) + rng() * f64::from(p.down_max_ms - p.down_min_ms);
            std::thread::sleep(Duration::from_secs_f64(hold_ms / 1000.0));
            self.up(b, false)?;
            if i + 1 < count {
                let wait_ms =
                    f64::from(p.wait_min_ms) + rng() * f64::from(p.wait_max_ms - p.wait_min_ms);
                std::thread::sleep(Duration::from_secs_f64(wait_ms / 1000.0));
            }
        }
        Ok(())
    }

    /// Firmware-side quadratic curve: `km.move(dx,dy,segments,cx,cy)`.
    ///
    /// `ctrl` defaults to `(dx.div_euclid(2), dy.div_euclid(2))`, matching
    /// Python `dx // 2` floor division (Rust `/` truncates toward zero, so
    /// `div_euclid` is used deliberately). Ranges mirror Python
    /// `check_bezier` (segments 1..=255, control i16).
    pub fn move_bezier(
        &self,
        dx: i32,
        dy: i32,
        segments: u32,
        ctrl: Option<(i32, i32)>,
    ) -> Result<()> {
        let (cx, cy) = ctrl.unwrap_or((dx.div_euclid(2), dy.div_euclid(2)));
        check_bezier(segments, cx, cy)?;
        let (x, y) = Self::i16s(dx, dy)?;
        let (ss, cxs, cys) = (segments.to_string(), cx.to_string(), cy.to_string());
        self.t.set("km.move", &[&x, &y, &ss, &cxs, &cys])
    }

    /// Absolute drag: `moveto(start)`, press, smooth-move, release.
    ///
    /// Mirrors Python `drag_abs`: `duration` must be positive (Python
    /// rejects `duration <= 0`), both endpoints are `check_xy`-validated,
    /// `segments = max(10, duration * 30)`, with 20 ms settles around the
    /// press/release. The button is always released, even if the smooth
    /// move fails (the final settle sleep is then skipped, as in Python).
    pub fn drag_abs(
        &self,
        start: (i32, i32),
        end: (i32, i32),
        button: Button,
        duration: Duration,
    ) -> Result<()> {
        if duration == Duration::ZERO {
            return Err(MakcuError::InvalidArgument(
                "duration must be positive".into(),
            ));
        }
        check_xy(start.0, start.1)?;
        check_xy(end.0, end.1)?;
        let segments = std::cmp::max(10, (duration.as_secs_f64() * 30.0) as u32);
        self.moveto(start.0, start.1)?;
        std::thread::sleep(Duration::from_millis(20));
        self.down(button)?;
        let r = self.move_smooth(
            end.0 - start.0,
            end.1 - start.1,
            segments,
            duration / segments,
        );
        let up_r = self.up(button, false);
        r.and(up_r)?;
        std::thread::sleep(Duration::from_millis(20));
        Ok(())
    }

    /// Best-effort release of all buttons; stops the button stream if running.
    ///
    /// Never fails: per-button and stream errors are swallowed, mirroring
    /// Python's `try/except Exception: pass` release loop.
    pub fn init(&self) {
        for b in Button::ALL {
            let _ = self.up(b, false);
        }
        if self
            .stream_mode
            .lock()
            .map(|m| m.is_some())
            .unwrap_or(false)
        {
            let _ = self.stop_button_stream();
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

    /// Require at least firmware API version `min`.
    ///
    /// No Python counterpart exists yet (the sibling branch has not added
    /// `requires_api`): this queries [`api_version`](Self::api_version) and
    /// compares, failing with the same [`MakcuError::InvalidArgument`]
    /// shape as every other argument gate (`"requires api >= {min},
    /// device reports {v}"`). Call before using version-gated commands
    /// (`autorelease` / `heartbeat` need 1, i.e. V4.103+).
    pub fn requires_api(&self, min: u32) -> Result<()> {
        let v = self.api_version()?;
        if v >= min {
            Ok(())
        } else {
            Err(MakcuError::InvalidArgument(format!(
                "requires api >= {min}, device reports {v}"
            )))
        }
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
        assert_eq!(
            m.move_smooth(1, 1, 0, Duration::ZERO),
            Err(MakcuError::InvalidArgument("steps: 0".into()))
        );
        let t2 = MockTransport::new();
        let m2 = Mouse::new(&t2);
        m2.drag(Button::Side1, 6, 0, 3, Duration::ZERO).unwrap();
        let sent = t2.sent();
        assert_eq!(sent[0], "km.ms1(1)");
        assert_eq!(sent[sent.len() - 1], "km.ms1(0)");
    }

    #[test]
    fn smooth_negative_deltas_match_python_round() {
        use std::time::Duration;
        // Python round() ties-to-even: round(-1.5) == -2, round(-2.5) == -2.
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        m.move_smooth(-3, -5, 2, Duration::ZERO).unwrap();
        assert_eq!(t.sent(), vec!["km.move(-2,-2)", "km.move(-1,-3)"]);
        // Deltas always sum to exactly the requested total.
        let mut total = (0, 0);
        for c in t.sent() {
            let inner = &c["km.move(".len()..c.len() - 1];
            let mut it = inner.split(',');
            total.0 += it.next().unwrap().parse::<i32>().unwrap();
            total.1 += it.next().unwrap().parse::<i32>().unwrap();
        }
        assert_eq!(total, (-3, -5));
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
    fn wait_idle_with_poll_consumes_scripted_moving() {
        use std::time::Duration;
        let t = MockTransport::new();
        t.push_moving(true);
        t.push_moving(true);
        t.push_moving(false);
        let m = Mouse::new(&t);
        assert!(m
            .wait_idle_with_poll(Duration::from_secs(2), Duration::from_millis(1))
            .unwrap());
        let moving_queries = t
            .sent()
            .iter()
            .filter(|c| c.as_str() == "km.moving()")
            .count();
        assert_eq!(moving_queries, 3);
    }

    #[test]
    fn wait_idle_with_poll_times_out_when_busy() {
        use std::time::Duration;
        let t = MockTransport::new().with_reply("km.moving", "1");
        let m = Mouse::new(&t);
        assert!(!m
            .wait_idle_with_poll(Duration::from_millis(50), Duration::from_millis(5))
            .unwrap());
    }

    #[test]
    fn requires_api_gates_on_version() {
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        assert!(m.requires_api(1).is_ok());
        assert_eq!(
            m.requires_api(2),
            Err(MakcuError::InvalidArgument(
                "requires api >= 2, device reports 1".into()
            ))
        );
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

    #[test]
    fn mock_callback_reentrant_register() {
        // A callback that registers another callback must not deadlock:
        // callbacks run after all monitor locks are released.
        use crate::events::ButtonMonitor;
        use std::sync::{Arc, Mutex};
        use std::time::Duration;
        let t = MockTransport::new();
        t.push_stream_frame(b"km.\x01\r\n");
        let m = Arc::new(Mouse::new(t));
        let mon = Arc::new(ButtonMonitor::new(
            Arc::clone(&m),
            Duration::from_millis(10),
        ));
        let slot: Arc<Mutex<Option<Arc<ButtonMonitor<MockTransport>>>>> =
            Arc::new(Mutex::new(None));
        let slot2 = Arc::clone(&slot);
        let seen = Arc::new(Mutex::new(0u32));
        let seen2 = Arc::clone(&seen);
        mon.on_change(move |_mask| {
            *seen2.lock().unwrap() += 1;
            slot2.lock().unwrap().as_ref().unwrap().on_change(|_| {});
        });
        *slot.lock().unwrap() = Some(Arc::clone(&mon));
        m.start_button_stream(1).unwrap();
        assert_eq!(
            mon.pump_once(Duration::from_millis(100)).unwrap(),
            Some(0x01)
        );
        assert_eq!(*seen.lock().unwrap(), 1);
        m.stop_button_stream().unwrap();
    }

    #[test]
    fn button_states_mask_and_pressed() {
        let t = MockTransport::new()
            .with_reply("km.left", "2")
            .with_reply("km.right", "1")
            .with_reply("km.middle", "0")
            .with_reply("km.ms1", "3")
            .with_reply("km.ms2", "0");
        let m = Mouse::new(&t);
        let states = m.get_button_states().unwrap();
        assert_eq!(states[0], (Button::Left, ButtonOwnership::Injected));
        assert_eq!(states[3], (Button::Side1, ButtonOwnership::Both));
        assert_eq!(m.get_button_mask().unwrap(), 0b01001);
        assert!(m.is_pressed(Button::Left).unwrap());
        assert!(!m.is_pressed(Button::Right).unwrap());
        assert!(m.is_pressed(Button::Side1).unwrap());
    }

    #[test]
    fn locks_and_aliases() {
        let t = MockTransport::new().with_reply("km.lock_mx", "1");
        let m = Mouse::new(&t);
        assert!(m.lock_state(LockTarget::Mx).unwrap());
        assert!(m.is_locked(LockTarget::Mx).unwrap());
        assert!(!m.is_locked(LockTarget::My).unwrap());
        let all = m.get_all_lock_states().unwrap();
        assert_eq!(all.len(), 14);
        assert_eq!(all[5], ("mx", true));
        assert_eq!(all[6], ("my", false));
        m.press(Button::Right).unwrap();
        m.release(Button::Right, true).unwrap();
        let sent = t.sent();
        assert!(sent.contains(&"km.right(1)".to_string()));
        assert!(sent.contains(&"km.right(2)".to_string()));
    }

    #[test]
    fn double_click_sends_two_clicks() {
        use std::time::Duration;
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        m.double_click(Button::Left, Duration::ZERO).unwrap();
        assert_eq!(t.sent(), vec!["km.click(1,1)", "km.click(1,1)"]);
    }

    #[test]
    fn bezier_wire_format_and_validation() {
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        m.move_bezier(100, 50, 20, None).unwrap();
        assert_eq!(t.sent(), vec!["km.move(100,50,20,50,25)"]);
        assert!(m.move_bezier(0, 0, 0, None).is_err());
        assert!(m.move_bezier(0, 0, 256, None).is_err());
        assert!(m.move_bezier(0, 0, 1, Some((40000, 0))).is_err());
        let t2 = MockTransport::new();
        let m2 = Mouse::new(&t2);
        m2.move_bezier(-3, -5, 1, None).unwrap();
        // Python // floors: -3//2 == -2, -5//2 == -3.
        assert_eq!(t2.sent(), vec!["km.move(-3,-5,1,-2,-3)"]);
    }

    #[test]
    fn human_like_validates_and_clicks() {
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        let mut zero = || 0.0;
        assert_eq!(
            m.click_human_like(Button::Left, 1, "nope", 0, &mut zero),
            Err(MakcuError::InvalidArgument(
                "unknown click profile: \"nope\"".into()
            ))
        );
        assert_eq!(
            m.click_human_like(Button::Left, 0, "normal", 0, &mut zero),
            Err(MakcuError::InvalidArgument("count: 0".into()))
        );
        assert_eq!(
            m.click_human_like(Button::Left, 1, "normal", -1, &mut zero),
            Err(MakcuError::InvalidArgument("jitter: -1".into()))
        );
        m.click_human_like(Button::Left, 1, "fast", 0, &mut zero)
            .unwrap();
        assert_eq!(t.sent(), vec!["km.left(1)", "km.left(0)"]);
    }

    #[test]
    fn human_like_jitter_zero_sends_only_down_up() {
        // With jitter == 0 no move_rel frames may appear: the exact wire
        // sequence is one down + one up per click.
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        let mut zero = || 0.0;
        m.click_human_like(Button::Left, 2, "fast", 0, &mut zero)
            .unwrap();
        assert_eq!(
            t.sent(),
            vec!["km.left(1)", "km.left(0)", "km.left(1)", "km.left(0)"]
        );
    }

    #[test]
    fn drag_abs_happy_path_and_validation() {
        use std::time::Duration;
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        m.drag_abs((0, 0), (10, 0), Button::Left, Duration::from_millis(10))
            .unwrap();
        let sent = t.sent();
        assert_eq!(sent[0], "km.moveto(0,0)");
        assert_eq!(sent[1], "km.left(1)");
        assert_eq!(sent[sent.len() - 1], "km.left(0)");
        assert!(m
            .drag_abs((0, 0), (1, 1), Button::Left, Duration::ZERO)
            .is_err());
        assert!(m
            .drag_abs((40000, 0), (1, 1), Button::Left, Duration::from_millis(10))
            .is_err());
    }

    #[test]
    fn batch_up_silent_release() {
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        let mut b = m.batch();
        b.down(Button::Middle);
        b.up(Button::Middle, true);
        b.up(Button::Middle, false);
        b.execute().unwrap();
        assert_eq!(t.sent()[0], "km.middle(1)\rkm.middle(2)\rkm.middle(0)\r");
    }

    #[test]
    fn init_releases_and_stops_stream() {
        use std::time::Duration;
        let t = MockTransport::new();
        let m = Mouse::new(&t);
        m.start_button_stream(1).unwrap();
        m.init();
        let sent = t.sent();
        for cmd in [
            "km.left(0)",
            "km.right(0)",
            "km.middle(0)",
            "km.ms1(0)",
            "km.ms2(0)",
        ] {
            assert!(sent.contains(&cmd.to_string()), "{sent:?}");
        }
        assert!(sent.contains(&"km.buttons(0)".to_string()));
        // Stream mode cleared: polling now errors.
        assert!(m.poll_button_event(Duration::from_millis(10)).is_err());
    }
}
