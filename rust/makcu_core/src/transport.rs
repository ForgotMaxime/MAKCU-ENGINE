//! Transport abstraction: queries, silent setters, batches and streams.
//!
//! [`Transport`] is implemented by the serial driver and the in-memory mock.
//!
//! ## Observability (mirrors the Python reference)
//!
//! The Python `MakcuTransport` / `MockTransport` pair exposes a small
//! observability surface — I/O counters, a bounded ring buffer of recent
//! frames, a nestable `no_wait` flag, and connection-change callbacks.
//! This module ports that surface to Rust with identical semantics:
//!
//! * [`Transport::get_stats`] / [`Transport::reset_stats`] — counters for
//!   transmitted bytes, commands sent, reconnects, mean round-trip time and
//!   the last error message.
//! * [`Transport::get_recent_lines`] — last `n` raw `TX`/`RX` frames,
//!   oldest-first, at most [`RECENT_CAPACITY`] kept.
//! * [`Transport::enter_no_wait`] / [`Transport::exit_no_wait`] /
//!   [`Transport::is_no_wait`] plus the RAII [`NoWaitGuard`] (see
//!   [`Transport::no_wait`] and [`Transport::with_no_wait`]) — while active,
//!   `set()` degrades to fire-and-forget (no ERR-window read); queries and
//!   `send_raw()` are unaffected. May nest.
//! * [`Transport::on_connection_change`] /
//!   [`Transport::remove_connection_callback`] — callbacks run with `true`
//!   on connect, `false` on disconnect, dispatched from a snapshot taken
//!   under lock so registration during dispatch is safe.
//!
//! ## Interior mutability
//!
//! Every [`Transport`] method takes `&self` (the Python driver serializes
//! I/O behind locks while sharing the transport across threads). Any
//! implementation that records stats, buffers frames, counts `no_wait`
//! depth, or stores callbacks MUST use interior mutability (typically
//! `Mutex<...>` / atomics behind `&self`). The default method
//! implementations below are no-ops / empty snapshots so existing external
//! implementors keep compiling; override them to opt into observability.
//!
//! ## Callback panic policy
//!
//! Python logs a raising callback and continues with the rest. Rust has no
//! exceptions, so this port does NOT isolate callback panics: callbacks
//! MUST be fast, non-blocking, non-reentrant (never call
//! connect/close/reconnect from inside a callback), and MUST NOT panic.
//! If a callback does panic, it unwinds through the notify call, remaining
//! callbacks are skipped, and (for `close`/`reconnect`) the underlying
//! state change has already been applied. Transports never call
//! `catch_unwind`: swallowing panics would hide bugs and would require
//! `UnwindSafe` bounds on every callback. Keep callbacks trivial (set a
//! flag, send on a channel).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::Result;

/// How many recent `TX`/`RX` frames [`Transport::get_recent_lines`] keeps.
///
/// Oldest entries are evicted first; snapshots come back oldest-first,
/// mirroring Python's `collections.deque(maxlen=200)`.
pub const RECENT_CAPACITY: usize = 200;

/// Snapshot of I/O counters; see [`Transport::get_stats`].
///
/// Mirrors the Python `get_stats()` dict keys `tx_bytes`, `cmds_sent`,
/// `reconnects`, `avg_rtt_ms`, `last_error`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransportStats {
    /// Total transmitted payload bytes (sum of `len` over every TX frame).
    pub tx_bytes: u64,
    /// Number of transmitted frames (`query` + `set` + `send_raw` +
    /// `fire_and_forget` each count one; a failed pre-transmit `set` /
    /// `send_raw` counts zero).
    pub cmds_sent: u64,
    /// Number of `reconnect()` calls.
    pub reconnects: u64,
    /// Mean query round-trip in milliseconds (`0.0` when no query yet).
    pub avg_rtt_ms: f64,
    /// Last transport/protocol error message, if any.
    pub last_error: Option<String>,
}

/// Snapshot of per-command timings; see [`Transport::get_profile`].
///
/// Mirrors the Python `get_profile()` dict values `count`/`total_s`/
/// `avg_ms`/`min_ms`/`max_ms` for one command name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProfileEntry {
    /// Number of recorded `query` + `set` calls for this command.
    pub count: u64,
    /// Total recorded time in seconds.
    pub total_s: f64,
    /// Mean recorded time in milliseconds (`0.0` when `count == 0`).
    pub avg_ms: f64,
    /// Fastest recorded call in milliseconds.
    pub min_ms: f64,
    /// Slowest recorded call in milliseconds.
    pub max_ms: f64,
}

/// Connection-change callback: called with `true` on connect, `false` on disconnect.
///
/// Must be `Send + Sync + 'static` so transports can store and dispatch
/// callbacks from whatever thread runs `connect`/`close`/`reconnect`.
/// See the module docs for the panic policy (do not panic).
pub type ConnectionCallback = Arc<dyn Fn(bool) + Send + Sync + 'static>;

/// RAII guard for [`Transport::no_wait`]: enters on creation, exits on drop.
///
/// Mirrors Python's `with t.no_wait():` block (including nesting: each
/// guard adds one depth level, drop removes it, so nested guards stay
/// no-wait until the outermost guard drops). Dropping runs even on panic,
/// so the depth counter cannot leak.
pub struct NoWaitGuard<'a, T: Transport + ?Sized> {
    /// Transport whose `no_wait` depth this guard holds.
    transport: &'a T,
}

impl<'a, T: Transport + ?Sized> NoWaitGuard<'a, T> {
    /// Enter `no_wait` on `transport`; exits when the guard drops.
    pub fn new(transport: &'a T) -> Self {
        transport.enter_no_wait();
        Self { transport }
    }
}

impl<T: Transport + ?Sized> Drop for NoWaitGuard<'_, T> {
    fn drop(&mut self) {
        self.transport.exit_no_wait();
    }
}

/// Serial-or-mock I/O used by [`Mouse`](crate::Mouse).
pub trait Transport {
    /// Blocking query; returns the prompt-stripped payload.
    fn query(&self, name: &str, args: &[&str]) -> Result<String>;
    /// Silent setter; errors surface through the ERR window.
    ///
    /// Inside `no_wait` (see [`Transport::no_wait`]), degrades to
    /// fire-and-forget with no ERR-window read.
    fn set(&self, name: &str, args: &[&str]) -> Result<()>;
    /// Write without waiting for anything back.
    fn fire_and_forget(&self, name: &str, args: &[&str]) -> Result<()>;
    /// Write pre-framed bytes; single ERR-window read.
    fn send_raw(&self, data: &[u8]) -> Result<()>;
    /// Read one CRLF-terminated frame without touching the buffer.
    fn read_line(&self, timeout: std::time::Duration) -> Result<Vec<u8>>;
    /// Read exactly n bytes without touching the buffer.
    fn read_exact(&self, n: usize, timeout: std::time::Duration) -> Result<Vec<u8>>;
    /// Start the background demux reader (framed modes 1/2).
    fn start_event_loop(&self) -> Result<()>;
    /// Stop the background demux reader.
    fn stop_event_loop(&self) -> Result<()>;
    /// Whether the demux reader is active.
    fn event_loop_running(&self) -> bool;
    /// Next queued stream frame; only with the event loop running.
    fn poll_event(&self, timeout: std::time::Duration) -> Result<Vec<u8>>;

    /// Snapshot of I/O counters: bytes/commands/reconnects/mean RTT/last error.
    ///
    /// Default: zero snapshot (external implementors without observability
    /// keep compiling). Override with an interior-mutable counter store.
    fn get_stats(&self) -> TransportStats {
        TransportStats::default()
    }

    /// Zero all I/O counters and clear the last error (ring buffer kept).
    ///
    /// Default: no-op.
    fn reset_stats(&self) {}

    /// Opt in to per-command timing; off by default.
    ///
    /// When enabled, `query` and `set` fold each call duration into the
    /// per-command table (see [`Transport::get_profile`]). Off means a
    /// single `profiling_enabled` check per call with no clock read, so the
    /// disabled path stays cheap. Mirrors Python `enable_profiling()`.
    ///
    /// Default: no-op (profiling stays unsupported, `get_profile` is empty).
    fn enable_profiling(&self) {}

    /// Opt out of per-command timing; recorded data is discarded.
    ///
    /// Mirrors Python `disable_profiling()`. Default: no-op.
    fn disable_profiling(&self) {}

    /// Whether per-command timing is currently recorded.
    ///
    /// Default: `false`. This is the single gate checked by `query`/`set`,
    /// so callers pay one cheap check when profiling is off.
    fn profiling_enabled(&self) -> bool {
        false
    }

    /// Snapshot of per-command timings, keyed by command name.
    ///
    /// Each entry carries `count`/`total_s`/`avg_ms`/`min_ms`/`max_ms`,
    /// mirroring Python `get_profile()`. Returns a cloned snapshot, so
    /// mutating the result never affects the transport. Default: empty.
    fn get_profile(&self) -> HashMap<String, ProfileEntry> {
        HashMap::new()
    }

    /// Clear per-command timings (keeps the enabled state).
    ///
    /// Mirrors Python `reset_profile()`. Default: no-op.
    fn reset_profile(&self) {}

    /// Fold one `query`/`set` duration into the per-command table.
    ///
    /// Called by `query`/`set` only after a positive
    /// [`Transport::profiling_enabled`] check. Default: no-op. Hidden from
    /// docs: prefer `enable_profiling` / `get_profile` / `reset_profile`.
    #[doc(hidden)]
    fn record_profile(&self, _cmd: &str, _dt: Duration) {}

    /// Last `n` raw `TX`/`RX` frames, oldest-first (at most
    /// [`RECENT_CAPACITY`] kept; `n == 0` yields `[]`).
    ///
    /// Default: empty (no observability). Override with an
    /// interior-mutable bounded buffer.
    fn get_recent_lines(&self, _n: usize) -> Vec<String> {
        Vec::new()
    }

    /// Enter one `no_wait` depth level (may nest; paired with
    /// [`Transport::exit_no_wait`]).
    ///
    /// Default: no-op. Override with an interior-mutable depth counter.
    fn enter_no_wait(&self) {}

    /// Leave one `no_wait` depth level.
    ///
    /// Default: no-op. Must balance [`Transport::enter_no_wait`]; prefer
    /// [`Transport::no_wait`] / [`Transport::with_no_wait`] so exits run
    /// even on panic.
    fn exit_no_wait(&self) {}

    /// Whether inside a `no_wait` block (depth > 0).
    ///
    /// Default: `false`.
    fn is_no_wait(&self) -> bool {
        false
    }

    /// RAII `no_wait` scope: `set()` degrades to fire-and-forget until the
    /// returned guard drops. May nest.
    ///
    /// Provided method (calls [`Transport::enter_no_wait`] /
    /// [`Transport::exit_no_wait`]); excluded from dynamic dispatch
    /// (`Self: Sized`) so `dyn Transport` stays object-safe.
    fn no_wait(&self) -> NoWaitGuard<'_, Self>
    where
        Self: Sized,
    {
        NoWaitGuard::new(self)
    }

    /// Run `f` inside a `no_wait` scope; returns `f`'s value.
    ///
    /// Provided method; the guard drops (exiting one depth level) even if
    /// `f` panics. Excluded from dynamic dispatch (`Self: Sized`).
    fn with_no_wait<R>(&self, f: impl FnOnce() -> R) -> R
    where
        Self: Sized,
    {
        let _guard = self.no_wait();
        f()
    }

    /// Register a callback run with `true` on connect, `false` on disconnect.
    ///
    /// Returns the same handle back for removal. Callbacks run
    /// synchronously on the thread calling connect/close/reconnect, from a
    /// snapshot taken under lock, so registering/unregistering from inside
    /// a callback is safe. See the module docs for the panic policy:
    /// callbacks must not panic; a panic skips the remaining callbacks.
    /// Default: discards the callback (no observability) and returns it.
    fn on_connection_change(&self, cb: ConnectionCallback) -> ConnectionCallback {
        cb
    }

    /// Unregister a connection callback. Missing entries are ignored.
    ///
    /// Matches by [`Arc::ptr_eq`] identity, mirroring Python's
    /// `list.remove(cb)` identity semantics. Default: no-op.
    fn remove_connection_callback(&self, _cb: &ConnectionCallback) {}
}

impl<T: Transport> Transport for &T {
    fn query(&self, name: &str, args: &[&str]) -> Result<String> {
        (**self).query(name, args)
    }
    fn set(&self, name: &str, args: &[&str]) -> Result<()> {
        (**self).set(name, args)
    }
    fn fire_and_forget(&self, name: &str, args: &[&str]) -> Result<()> {
        (**self).fire_and_forget(name, args)
    }
    fn send_raw(&self, data: &[u8]) -> Result<()> {
        (**self).send_raw(data)
    }
    fn read_line(&self, timeout: std::time::Duration) -> Result<Vec<u8>> {
        (**self).read_line(timeout)
    }
    fn read_exact(&self, n: usize, timeout: std::time::Duration) -> Result<Vec<u8>> {
        (**self).read_exact(n, timeout)
    }
    fn start_event_loop(&self) -> Result<()> {
        (**self).start_event_loop()
    }
    fn stop_event_loop(&self) -> Result<()> {
        (**self).stop_event_loop()
    }
    fn event_loop_running(&self) -> bool {
        (**self).event_loop_running()
    }
    fn poll_event(&self, timeout: std::time::Duration) -> Result<Vec<u8>> {
        (**self).poll_event(timeout)
    }
    fn get_stats(&self) -> TransportStats {
        (**self).get_stats()
    }
    fn reset_stats(&self) {
        (**self).reset_stats();
    }
    fn enable_profiling(&self) {
        (**self).enable_profiling();
    }
    fn disable_profiling(&self) {
        (**self).disable_profiling();
    }
    fn profiling_enabled(&self) -> bool {
        (**self).profiling_enabled()
    }
    fn get_profile(&self) -> HashMap<String, ProfileEntry> {
        (**self).get_profile()
    }
    fn reset_profile(&self) {
        (**self).reset_profile();
    }
    fn record_profile(&self, cmd: &str, dt: Duration) {
        (**self).record_profile(cmd, dt);
    }
    fn get_recent_lines(&self, n: usize) -> Vec<String> {
        (**self).get_recent_lines(n)
    }
    fn enter_no_wait(&self) {
        (**self).enter_no_wait();
    }
    fn exit_no_wait(&self) {
        (**self).exit_no_wait();
    }
    fn is_no_wait(&self) -> bool {
        (**self).is_no_wait()
    }
    fn on_connection_change(&self, cb: ConnectionCallback) -> ConnectionCallback {
        (**self).on_connection_change(cb)
    }
    fn remove_connection_callback(&self, cb: &ConnectionCallback) {
        (**self).remove_connection_callback(cb);
    }
}
