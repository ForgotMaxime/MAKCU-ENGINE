//! In-memory [`Transport`] for tests and offline development.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use crate::{MakcuError, Result, Transport};

/// In-memory `Transport` for tests and offline development.
pub struct MockTransport {
    replies: HashMap<String, String>,
    sent: Mutex<Vec<String>>,
    stream: Mutex<Vec<u8>>,
    fail_next_send: Mutex<bool>,
    loop_active: Mutex<bool>,
}

impl MockTransport {
    /// Empty mock with canned query replies.
    pub fn new() -> Self {
        Self {
            replies: HashMap::new(),
            sent: Mutex::new(Vec::new()),
            stream: Mutex::new(Vec::new()),
            fail_next_send: Mutex::new(false),
            loop_active: Mutex::new(false),
        }
    }

    /// Commands recorded so far, in order.
    pub fn sent(&self) -> Vec<String> {
        self.sent.lock().unwrap().clone()
    }

    /// Enqueue a raw stream frame for `read_line` / `poll_event`.
    pub fn push_stream_frame(&self, frame: &[u8]) {
        self.stream.lock().unwrap().extend_from_slice(frame);
    }

    /// Canned reply override, matched on the full command or base name.
    /// Canned reply override, matched on the full command or base name.
    pub fn with_reply(mut self, name: &str, value: &str) -> Self {
        self.replies.insert(name.to_string(), value.to_string());
        self
    }

    /// Make the next `send_raw`/`set` fail with a device ERR.
    pub fn fail_next_send(&self) {
        *self.fail_next_send.lock().unwrap() = true;
    }

    fn take_fail(&self) -> bool {
        let mut f = self.fail_next_send.lock().unwrap();
        let v = *f;
        *f = false;
        v
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

fn fmt_cmd(name: &str, args: &[&str]) -> String {
    format!("{name}({})", args.join(","))
}

impl Transport for MockTransport {
    fn query(&self, name: &str, args: &[&str]) -> Result<String> {
        let cmd = fmt_cmd(name, args);
        self.sent.lock().unwrap().push(cmd.clone());
        let base = name.to_string();
        if let Some(r) = self.replies.get(&cmd).or_else(|| self.replies.get(&base)) {
            return Ok(r.clone());
        }
        let d: &str = match base.as_str() {
            "km.version" => "km.MAKCU",
            "km.device" => "R:MK;M:16uf;K:8uf;C:0uf",
            "km.echo" => "0",
            "km.moving" => "0",
            "km.left" => "0",
            "km.buttons" => "0",
            "km.autorelease" => "0",
            "km.api_version" => "1",
            "km.getpos" => "km.getpos(100,200)",
            "km.screen" => "km.screen(1920,1080)",
            _ => "",
        };
        Ok(d.to_string())
    }

    fn set(&self, name: &str, args: &[&str]) -> Result<()> {
        if self.take_fail() {
            return Err(MakcuError::Protocol(format!("{name} failed: device ERR")));
        }
        self.sent.lock().unwrap().push(fmt_cmd(name, args));
        Ok(())
    }

    fn fire_and_forget(&self, name: &str, args: &[&str]) -> Result<()> {
        self.sent.lock().unwrap().push(fmt_cmd(name, args));
        Ok(())
    }

    fn send_raw(&self, data: &[u8]) -> Result<()> {
        if self.take_fail() {
            return Err(MakcuError::Protocol(format!(
                "device ERR: {:?}",
                &data[..data.len().min(32)]
            )));
        }
        self.sent
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(data).into_owned());
        Ok(())
    }

    fn read_line(&self, _timeout: Duration) -> Result<Vec<u8>> {
        let mut s = self.stream.lock().unwrap();
        let pos = s
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or(MakcuError::Timeout)?;
        Ok(s.drain(..pos + 2).collect())
    }

    fn read_exact(&self, n: usize, _timeout: Duration) -> Result<Vec<u8>> {
        if n == 0 {
            return Err(MakcuError::InvalidArgument("read_exact n=0".into()));
        }
        let mut s = self.stream.lock().unwrap();
        if s.len() < n {
            return Err(MakcuError::Timeout);
        }
        Ok(s.drain(..n).collect())
    }

    /// No-op flag used to exercise the loop code path in tests.
    fn start_event_loop(&self) -> Result<()> {
        *self.loop_active.lock().unwrap() = true;
        Ok(())
    }

    /// Clears the loop flag.
    fn stop_event_loop(&self) -> Result<()> {
        *self.loop_active.lock().unwrap() = false;
        Ok(())
    }

    /// Whether the loop flag is set.
    fn event_loop_running(&self) -> bool {
        *self.loop_active.lock().unwrap()
    }

    /// Pops the next queued stream frame.
    fn poll_event(&self, timeout: Duration) -> Result<Vec<u8>> {
        self.read_line(timeout)
    }
}
