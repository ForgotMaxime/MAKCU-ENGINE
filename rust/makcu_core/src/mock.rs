//! In-memory [`Transport`](crate::Transport) for tests and offline development.
//!
//! Full observability mirror of the Python `MockTransport`: I/O counters,
//! a bounded [`RECENT_CAPACITY`](crate::transport::RECENT_CAPACITY) ring
//! buffer, a nestable `no_wait` flag, and connection-change callbacks with
//! snapshot-then-dispatch semantics (`connect` always notifies `true`;
//! `close` notifies `false` only on a connected-to-disconnected transition,
//! so `reconnect` emits `false` then `true`).

use std::collections::{HashMap, VecDeque};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::{Duration, Instant};

use crate::{
    transport::{ConnectionCallback, TransportStats, RECENT_CAPACITY},
    MakcuError, Result, Transport,
};

/// Interior-mutable observability counters for [`MockTransport`].
#[derive(Debug, Default)]
struct Obs {
    /// Total transmitted payload bytes.
    tx_bytes: u64,
    /// Number of transmitted frames.
    cmds_sent: u64,
    /// Number of `reconnect()` calls.
    reconnects: u64,
    /// Accumulated query round-trip time.
    rtt_total: Duration,
    /// Number of timed queries.
    rtt_count: u64,
    /// Last transport/protocol error message.
    last_error: Option<String>,
    /// Bounded recent-frame ring (`TX ...` / `RX ...`, oldest-first).
    recent: VecDeque<String>,
    /// Nesting depth of `no_wait` scopes.
    no_wait_depth: usize,
}

/// In-memory `Transport` for tests and offline development.
pub struct MockTransport {
    replies: HashMap<String, String>,
    sent: Mutex<Vec<String>>,
    stream: Mutex<Vec<u8>>,
    fail_next_send: Mutex<bool>,
    loop_active: Mutex<bool>,
    connected: AtomicBool,
    conn_cbs: Mutex<Vec<ConnectionCallback>>,
    obs: Mutex<Obs>,
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
            connected: AtomicBool::new(false),
            conn_cbs: Mutex::new(Vec::new()),
            obs: Mutex::new(Obs::default()),
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

    /// Whether the mock port is currently open.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Mark the mock port open and notify callbacks with `true`.
    ///
    /// Always notifies (no transition check), mirroring the Python mock.
    pub fn connect(&self) {
        self.connected.store(true, Ordering::SeqCst);
        self.notify_connection(true);
    }

    /// Mark the mock port closed, notifying with `false` on transition.
    ///
    /// A redundant close while already disconnected is silent, mirroring
    /// the Python mock (and serial transport).
    pub fn close(&self) {
        let was = self.connected.swap(false, Ordering::SeqCst);
        if was {
            self.notify_connection(false);
        }
    }

    /// Close then reopen the mock port (emits `false` then `true`).
    ///
    /// Increments the `reconnects` counter first, mirroring Python.
    pub fn reconnect(&self) {
        self.obs.lock().unwrap().reconnects += 1;
        self.close();
        self.connect();
    }

    /// Dispatch a connection event to a snapshot of callbacks.
    ///
    /// The snapshot is taken under lock so registration/removal during
    /// dispatch is safe. Callbacks must not panic (see module docs); a
    /// panic unwinds and skips the remaining callbacks.
    fn notify_connection(&self, connected: bool) {
        let cbs = self.conn_cbs.lock().unwrap().clone();
        for cb in cbs {
            cb(connected);
        }
    }

    /// Count one transmitted frame and append it to the ring buffer.
    fn note_tx(&self, payload: &[u8]) {
        let mut obs = self.obs.lock().unwrap();
        obs.tx_bytes += payload.len() as u64;
        obs.cmds_sent += 1;
        push_recent(
            &mut obs.recent,
            format!("TX {}", String::from_utf8_lossy(payload)),
        );
    }

    /// Append one canned reply to the ring buffer.
    fn note_rx(&self, reply: &str) {
        let mut obs = self.obs.lock().unwrap();
        push_recent(&mut obs.recent, format!("RX {reply}"));
    }

    /// Fold one query round-trip into the running average.
    fn note_rtt(&self, dt: Duration) {
        let mut obs = self.obs.lock().unwrap();
        obs.rtt_total += dt;
        obs.rtt_count += 1;
    }

    /// Remember the last transport/protocol error message.
    fn note_error(&self, e: &MakcuError) {
        self.obs.lock().unwrap().last_error = Some(e.to_string());
    }

    fn take_fail(&self) -> bool {
        let mut f = self.fail_next_send.lock().unwrap();
        let v = *f;
        *f = false;
        v
    }
}

/// Push one line onto a bounded ring, evicting the oldest on overflow.
fn push_recent(recent: &mut VecDeque<String>, line: String) {
    if recent.len() >= RECENT_CAPACITY {
        recent.pop_front();
    }
    recent.push_back(line);
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
        let start = Instant::now();
        let payload = crate::protocol::build_cmd(name, args);
        self.note_tx(&payload);
        let cmd = fmt_cmd(name, args);
        self.sent.lock().unwrap().push(cmd.clone());
        let base = name.to_string();
        let reply = if let Some(r) = self.replies.get(&cmd).or_else(|| self.replies.get(&base)) {
            r.clone()
        } else {
            let d: &str = match base.as_str() {
                "km.version" => "km.MAKCU",
                "km.device" => "R:MK;M:16uf;K:8uf;C:0uf",
                "km.echo" => "0",
                "km.moving" => "0",
                "km.left" => "0",
                "km.right" => "0",
                "km.middle" => "0",
                "km.ms1" => "0",
                "km.ms2" => "0",
                "km.buttons" => "0",
                "km.autorelease" => "0",
                "km.api_version" => "1",
                "km.getpos" => "km.getpos(100,200)",
                "km.screen" => "km.screen(1920,1080)",
                _ if base.starts_with("km.lock_") => "0",
                _ => "",
            };
            d.to_string()
        };
        self.note_rtt(start.elapsed());
        self.note_rx(&reply);
        Ok(reply)
    }

    fn set(&self, name: &str, args: &[&str]) -> Result<()> {
        if self.is_no_wait() {
            return self.fire_and_forget(name, args);
        }
        if self.take_fail() {
            let err = MakcuError::Protocol(format!("{name} failed: device ERR"));
            self.note_error(&err);
            return Err(err);
        }
        let payload = crate::protocol::build_cmd(name, args);
        self.note_tx(&payload);
        self.sent.lock().unwrap().push(fmt_cmd(name, args));
        Ok(())
    }

    fn fire_and_forget(&self, name: &str, args: &[&str]) -> Result<()> {
        let payload = crate::protocol::build_cmd(name, args);
        self.note_tx(&payload);
        self.sent.lock().unwrap().push(fmt_cmd(name, args));
        Ok(())
    }

    fn send_raw(&self, data: &[u8]) -> Result<()> {
        if self.take_fail() {
            // Mirror Python's truncation to the first 32 bytes in the message.
            let truncated =
                MakcuError::Protocol(format!("device ERR: {:?}", &data[..data.len().min(32)]));
            self.note_error(&truncated);
            return Err(truncated);
        }
        self.note_tx(data);
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

    fn get_stats(&self) -> TransportStats {
        let obs = self.obs.lock().unwrap();
        TransportStats {
            tx_bytes: obs.tx_bytes,
            cmds_sent: obs.cmds_sent,
            reconnects: obs.reconnects,
            avg_rtt_ms: if obs.rtt_count > 0 {
                obs.rtt_total.as_secs_f64() / obs.rtt_count as f64 * 1000.0
            } else {
                0.0
            },
            last_error: obs.last_error.clone(),
        }
    }

    fn reset_stats(&self) {
        let mut obs = self.obs.lock().unwrap();
        obs.tx_bytes = 0;
        obs.cmds_sent = 0;
        obs.reconnects = 0;
        obs.rtt_total = Duration::ZERO;
        obs.rtt_count = 0;
        obs.last_error = None;
    }

    fn get_recent_lines(&self, n: usize) -> Vec<String> {
        let obs = self.obs.lock().unwrap();
        let len = obs.recent.len();
        obs.recent
            .iter()
            .skip(len.saturating_sub(n))
            .cloned()
            .collect()
    }

    fn enter_no_wait(&self) {
        self.obs.lock().unwrap().no_wait_depth += 1;
    }

    fn exit_no_wait(&self) {
        let mut obs = self.obs.lock().unwrap();
        obs.no_wait_depth = obs.no_wait_depth.saturating_sub(1);
    }

    fn is_no_wait(&self) -> bool {
        self.obs.lock().unwrap().no_wait_depth > 0
    }

    fn on_connection_change(&self, cb: ConnectionCallback) -> ConnectionCallback {
        self.conn_cbs.lock().unwrap().push(cb.clone());
        cb
    }

    fn remove_connection_callback(&self, cb: &ConnectionCallback) {
        let mut cbs = self.conn_cbs.lock().unwrap();
        if let Some(pos) = cbs.iter().position(|c| ConnectionCallback::ptr_eq(c, cb)) {
            cbs.remove(pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn stats_increment_all_paths() {
        let t = MockTransport::new();
        t.query("km.version", &[]).unwrap();
        t.set("km.move", &["10", "0"]).unwrap();
        t.send_raw(b"km.wheel(1)\r").unwrap();
        t.fire_and_forget("km.move", &["1", "1"]).unwrap();
        let s = t.get_stats();
        assert_eq!(s.cmds_sent, 4);
        assert_eq!(
            s.tx_bytes,
            b"km.version()\rkm.move(10,0)\rkm.wheel(1)\rkm.move(1,1)\r".len() as u64
        );
        assert!(s.avg_rtt_ms >= 0.0);
        assert_eq!(s.reconnects, 0);
        assert_eq!(s.last_error, None);
    }

    #[test]
    fn stats_reset_and_error() {
        let t = MockTransport::new();
        t.query("km.version", &[]).unwrap();
        t.fail_next_send();
        let r = t.set("km.move", &["1", "0"]);
        assert!(r.is_err());
        let s = t.get_stats();
        // The failed set never transmitted.
        assert_eq!(s.cmds_sent, 1);
        assert!(s.last_error.as_deref().unwrap().contains("km.move"));
        t.reconnect();
        assert_eq!(t.get_stats().reconnects, 1);
        t.reset_stats();
        assert_eq!(
            t.get_stats(),
            TransportStats {
                tx_bytes: 0,
                cmds_sent: 0,
                reconnects: 0,
                avg_rtt_ms: 0.0,
                last_error: None,
            }
        );
    }

    #[test]
    fn ring_buffer_bounded() {
        let t = MockTransport::new();
        for i in 0..250 {
            let a = i.to_string();
            t.fire_and_forget("km.move", &[&a, "0"]).unwrap();
        }
        let lines = t.get_recent_lines(500);
        assert_eq!(lines.len(), RECENT_CAPACITY);
        assert_eq!(lines[0], "TX km.move(50,0)\r");
        assert_eq!(lines[RECENT_CAPACITY - 1], "TX km.move(249,0)\r");
        assert_eq!(t.get_recent_lines(20), lines[lines.len() - 20..]);
        assert_eq!(t.get_recent_lines(3), lines[lines.len() - 3..]);
        assert!(t.get_recent_lines(0).is_empty());
    }

    #[test]
    fn ring_buffer_query_set_contents() {
        let t = MockTransport::new();
        t.query("km.version", &[]).unwrap();
        t.set("km.move", &["1", "2"]).unwrap();
        let lines = t.get_recent_lines(20);
        assert_eq!(lines[0], "TX km.version()\r");
        assert_eq!(lines[1], "RX km.MAKCU");
        assert_eq!(lines[2], "TX km.move(1,2)\r");
    }

    #[test]
    fn no_wait_set_skips_err_window() {
        let t = MockTransport::new();
        t.fail_next_send();
        {
            let _guard = t.no_wait();
            t.set("km.move", &["1", "0"]).unwrap();
        }
        assert!(t.sent().contains(&"km.move(1,0)".to_string()));
        // Outside the block the ERR window still applies.
        t.fail_next_send();
        assert!(t.set("km.move", &["1", "0"]).is_err());
        // Nesting: still no-wait until the outer guard drops.
        t.fail_next_send();
        {
            let _outer = t.no_wait();
            let _inner = t.no_wait();
            t.set("km.move", &["2", "0"]).unwrap();
        }
        assert!(t.sent().contains(&"km.move(2,0)".to_string()));
        // with_no_wait closure form.
        t.fail_next_send();
        t.with_no_wait(|| {
            t.set("km.move", &["3", "0"]).unwrap();
        });
        assert!(t.sent().contains(&"km.move(3,0)".to_string()));
    }

    #[test]
    fn no_wait_queries_still_wait() {
        let t = MockTransport::new();
        {
            let _guard = t.no_wait();
            assert_eq!(t.query("km.version", &[]).unwrap(), "km.MAKCU");
            assert_eq!(t.query("km.getpos", &[]).unwrap(), "km.getpos(100,200)");
        }
        assert_eq!(t.get_stats().cmds_sent, 2);
        // TX+RX per query.
        assert_eq!(t.get_recent_lines(20).len(), 4);
    }

    #[test]
    fn connection_callbacks_transition() {
        let t = MockTransport::new();
        let calls: Arc<Mutex<Vec<bool>>> = Arc::new(Mutex::new(Vec::new()));
        let calls2 = Arc::clone(&calls);
        let cb = Arc::new(move |c: bool| calls2.lock().unwrap().push(c));
        let handle = t.on_connection_change(cb);
        assert!(!t.is_connected());
        // Redundant close is silent.
        t.close();
        assert!(calls.lock().unwrap().is_empty());
        t.connect();
        assert_eq!(*calls.lock().unwrap(), vec![true]);
        // Reconnect emits false then true and bumps the counter.
        t.reconnect();
        assert_eq!(*calls.lock().unwrap(), vec![true, false, true]);
        assert_eq!(t.get_stats().reconnects, 1);
        // Removal stops notifications.
        t.remove_connection_callback(&handle);
        t.close();
        assert_eq!(*calls.lock().unwrap(), vec![true, false, true]);
        // Removing a missing entry is ignored.
        t.remove_connection_callback(&handle);
    }

    #[test]
    fn callback_snapshot_registration_during_dispatch() {
        use std::sync::Arc as StdArc;
        let t = StdArc::new(MockTransport::new());
        let t2 = StdArc::clone(&t);
        // Registering from inside a callback must not deadlock: dispatch
        // runs from a snapshot, so the newcomer misses the in-flight event.
        let reg: ConnectionCallback = Arc::new(move |_: bool| {
            t2.on_connection_change(Arc::new(|_: bool| {}));
        });
        t.on_connection_change(reg);
        t.connect();
        assert!(t.is_connected());
        // Second connect notifies both the original and the newcomer.
        t.close();
        t.connect();
        assert!(t.is_connected());
    }
}
