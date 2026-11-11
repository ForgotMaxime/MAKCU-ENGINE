//! Serial transport over the management COM port.
//!
//! [`MakcuTransport`] mirrors the Python `MakcuTransport` observability
//! surface: I/O counters ([`Transport::get_stats`](makcu_core::Transport::get_stats)),
//! a bounded recent-frame ring, a nestable `no_wait` flag honored by
//! `set()`, a `reconnects` counter, and connection-change callbacks.

use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant};

#[allow(unused_imports)]
use std::io::{Read, Write};

use makcu_core::transport::{ConnectionCallback, TransportStats, RECENT_CAPACITY};
use makcu_core::{protocol::build_cmd, MakcuError, Result, Transport};

/// Baud probe order: 4M first, 115200 fallback.
pub const BAUDS: [u32; 2] = [4_000_000, 115_200];
/// CH343 vendor ID used to prefer the MAKCU port.
pub const USB_VID: u16 = 0x1A86;
/// CH343 product ID used to prefer the MAKCU port.
pub const USB_PID: u16 = 0x55D3;
/// Reply terminator every query ends with.
pub const PROMPT: &[u8] = b">>> ";
const ERROR_WINDOW: Duration = Duration::from_millis(80);
/// Poll granularity for the demux reader: bounds every single read so a
/// call can never exceed its deadline (not the port's open timeout).
const POLL: Duration = Duration::from_millis(10);

/// Interior-mutable observability counters; see [`Transport::get_stats`].
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

/// Push one line onto a bounded ring, evicting the oldest on overflow.
fn push_recent(recent: &mut VecDeque<String>, line: String) {
    if recent.len() >= RECENT_CAPACITY {
        recent.pop_front();
    }
    recent.push_back(line);
}

struct EventState {
    running: AtomicBool,
    stop: AtomicBool,
    /// Set when the reader thread died on a serial error (port gone).
    /// Requests then fail fast with a connection error so callers can
    /// reconnect instead of timing out against a dead reader.
    dead: AtomicBool,
    replies: Mutex<VecDeque<Vec<u8>>>,
    reply_cv: Condvar,
    events: Mutex<VecDeque<Vec<u8>>>,
    event_cv: Condvar,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl EventState {
    fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            dead: AtomicBool::new(false),
            replies: Mutex::new(VecDeque::new()),
            reply_cv: Condvar::new(),
            events: Mutex::new(VecDeque::new()),
            event_cv: Condvar::new(),
            handle: Mutex::new(None),
        }
    }
}

struct Shared {
    /// `None` while disconnected (`close()` takes the handle).
    port: Mutex<Option<Box<dyn serialport::SerialPort>>>,
    /// Serializes request/response pairs when the event loop is active.
    req: Mutex<()>,
    timeout: Duration,
    port_name: String,
    baud: u32,
    ev: EventState,
    obs: Mutex<Obs>,
    conn_cbs: Mutex<Vec<ConnectionCallback>>,
}

/// Exclusive serial transport with optional demux event loop.
///
/// Not `Clone` on purpose: `Drop` stops a running loop, which would
/// disturb other owners of a shared port. Create one transport per port.
///
/// All observability state uses interior mutability behind `&self`, so the
/// transport can be shared across threads like the Python driver.
pub struct MakcuTransport {
    shared: Arc<Shared>,
}

impl MakcuTransport {
    /// Port this transport was opened on.
    pub fn port_name(&self) -> &str {
        &self.shared.port_name
    }

    /// Baud rate this transport runs at.
    pub fn baud(&self) -> u32 {
        self.shared.baud
    }

    /// Whether the serial port is currently open.
    pub fn is_connected(&self) -> bool {
        self.shared.port.lock().unwrap().is_some()
    }

    /// Open a port exclusively (8N1, no flow control) with backoff retry.
    pub fn open(port: &str, baud: u32, timeout: Duration) -> Result<Self> {
        let mut last_err = String::new();
        for attempt in 0..10 {
            match serialport::new(port, baud)
                .data_bits(serialport::DataBits::Eight)
                .parity(serialport::Parity::None)
                .stop_bits(serialport::StopBits::One)
                .timeout(timeout)
                .open()
            {
                Ok(p) => {
                    return Ok(Self {
                        shared: Arc::new(Shared {
                            port: Mutex::new(Some(p)),
                            req: Mutex::new(()),
                            timeout,
                            port_name: port.to_string(),
                            baud,
                            ev: EventState::new(),
                            obs: Mutex::new(Obs::default()),
                            conn_cbs: Mutex::new(Vec::new()),
                        }),
                    });
                }
                Err(e) => {
                    last_err = format!("{port}@{baud}: {e}");
                    std::thread::sleep(Duration::from_millis(200 * (attempt + 1)));
                }
            }
        }
        Err(MakcuError::Connection(format!("cannot open {last_err}")))
    }

    /// Reopen a closed port with backoff retry; notifies with `true`.
    ///
    /// Idempotent: if the port is already open this is a no-op (no extra
    /// notification), unlike the constructor [`MakcuTransport::open`].
    pub fn connect(&self) -> Result<()> {
        if self.is_connected() {
            return Ok(());
        }
        let mut last_err = String::new();
        for attempt in 0..10 {
            match serialport::new(&self.shared.port_name, self.shared.baud)
                .data_bits(serialport::DataBits::Eight)
                .parity(serialport::Parity::None)
                .stop_bits(serialport::StopBits::One)
                .timeout(self.shared.timeout)
                .open()
            {
                Ok(p) => {
                    *self.shared.port.lock().unwrap() = Some(p);
                    self.notify_connection(true);
                    return Ok(());
                }
                Err(e) => {
                    last_err = format!("{}@{}: {e}", self.shared.port_name, self.shared.baud);
                    std::thread::sleep(Duration::from_millis(200 * (attempt + 1)));
                }
            }
        }
        Err(MakcuError::Connection(format!("cannot open {last_err}")))
    }

    /// Stop the event loop if running, then close the port.
    ///
    /// Emits `false` to connection callbacks only on a connected to
    /// disconnected transition; a redundant close is silent, so
    /// [`MakcuTransport::reconnect`] emits `false` then `true`.
    pub fn close(&self) {
        let _ = self.stop_event_loop();
        let was_connected = self.shared.port.lock().unwrap().take().is_some();
        if was_connected {
            self.notify_connection(false);
        }
    }

    /// Close, cool down, and reopen the port (bumps the `reconnects` counter).
    pub fn reconnect(&self) -> Result<()> {
        self.shared.obs.lock().unwrap().reconnects += 1;
        self.close();
        std::thread::sleep(Duration::from_millis(300));
        self.connect()
    }

    /// Dispatch a connection event to a snapshot of callbacks.
    ///
    /// The snapshot is taken under lock so registration/removal during
    /// dispatch is safe. Callbacks must not panic (see module docs); a
    /// panic unwinds and skips the remaining callbacks.
    fn notify_connection(&self, connected: bool) {
        let cbs = self.shared.conn_cbs.lock().unwrap().clone();
        for cb in cbs {
            cb(connected);
        }
    }

    /// Count one transmitted frame and append it to the ring buffer.
    fn note_tx(&self, payload: &[u8]) {
        let mut obs = self.shared.obs.lock().unwrap();
        obs.tx_bytes += payload.len() as u64;
        obs.cmds_sent += 1;
        push_recent(
            &mut obs.recent,
            format!("TX {}", String::from_utf8_lossy(payload)),
        );
    }

    /// Append one received frame to the ring buffer.
    fn note_rx(&self, raw: &[u8]) {
        let mut obs = self.shared.obs.lock().unwrap();
        push_recent(
            &mut obs.recent,
            format!("RX {}", String::from_utf8_lossy(raw)),
        );
    }

    /// Fold one query round-trip into the running average.
    fn note_rtt(&self, dt: Duration) {
        let mut obs = self.shared.obs.lock().unwrap();
        obs.rtt_total += dt;
        obs.rtt_count += 1;
    }

    /// Remember the last transport/protocol error message.
    fn note_error(&self, e: &MakcuError) {
        self.shared.obs.lock().unwrap().last_error = Some(e.to_string());
    }

    /// Detect a MAKCU, then open it. One-call shortcut.
    pub fn open_auto(timeout: Duration) -> Result<Self> {
        let (port, baud) = Self::detect()?;
        std::thread::sleep(Duration::from_millis(300));
        Self::open(&port, baud, timeout)
    }

    /// Serial ports ordered: CH343 match, other USB, then rest.
    pub fn list_candidates() -> Vec<String> {
        let ports = serialport::available_ports().unwrap_or_default();
        let mut scored: Vec<(u8, String)> = Vec::new();
        for info in ports {
            let score = match &info.port_type {
                serialport::SerialPortType::UsbPort(usb)
                    if usb.vid == USB_VID && usb.pid == USB_PID =>
                {
                    0
                }
                serialport::SerialPortType::UsbPort(_) => 1,
                _ => 2,
            };
            scored.push((score, info.port_name));
        }
        scored.sort();
        scored.into_iter().map(|(_, n)| n).collect()
    }

    /// Baud-scan one port with `km.version()`; returns the working baud.
    pub fn probe(port: &str) -> Result<u32> {
        for baud in BAUDS {
            let timeout = Duration::from_millis(800);
            let found = (|| {
                let mut p = serialport::new(port, baud).timeout(timeout).open().ok()?;
                let _ = p.clear(serialport::ClearBuffer::All);
                p.write_all(&build_cmd("km.version", &[])).ok()?;
                let _ = p.flush();
                let raw = read_until(&mut *p, PROMPT, timeout).ok()?;
                raw.windows(8).any(|w| w == b"km.MAKCU").then_some(baud)
            })();
            std::thread::sleep(Duration::from_millis(150));
            if let Some(baud) = found {
                return Ok(baud);
            }
        }
        Err(MakcuError::Connection(format!("no MAKCU on {port}")))
    }

    /// First candidate that answers `km.MAKCU`, with its baud.
    pub fn detect() -> Result<(String, u32)> {
        for port in Self::list_candidates() {
            if let Ok(baud) = Self::probe(&port) {
                return Ok((port, baud));
            }
        }
        Err(MakcuError::Connection(
            "no MAKCU detected on any serial port".into(),
        ))
    }

    /// Block until a MAKCU appears (hot-plug); Timeout otherwise.
    pub fn wait_for_device(timeout: Duration, poll: Duration) -> Result<(String, u32)> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(found) = Self::detect() {
                return Ok(found);
            }
            if Instant::now() >= deadline {
                return Err(MakcuError::Timeout);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            std::thread::sleep(poll.min(remaining));
        }
    }

    /// Every candidate that answers, for multi-device setups.
    pub fn detect_all() -> Vec<(String, u32)> {
        let mut out = Vec::new();
        for port in Self::list_candidates() {
            if let Ok(baud) = Self::probe(&port) {
                out.push((port, baud));
            }
        }
        out
    }

    /// Request/response through the demux reader. Empty return = silent
    /// setter (echo off), like the legacy direct path. Never clears the
    /// input buffer: the reader owns the stream and unread frames in the
    /// OS buffer belong to it.
    fn request(&self, payload: &[u8], timeout: Duration) -> Result<Vec<u8>> {
        if !self.shared.ev.running.load(Ordering::SeqCst)
            || self.shared.ev.dead.load(Ordering::SeqCst)
        {
            return Err(MakcuError::Connection("event reader not running".into()));
        }
        let _guard = self.shared.req.lock().unwrap();
        {
            let mut slot = self.shared.port.lock().unwrap();
            let Some(port) = slot.as_mut() else {
                return Err(MakcuError::Connection("transport not connected".into()));
            };
            port.write_all(payload)
                .map_err(|e| MakcuError::Connection(e.to_string()))?;
            port.flush()
                .map_err(|e| MakcuError::Connection(e.to_string()))?;
        }
        let deadline = Instant::now() + timeout;
        let mut q = self.shared.ev.replies.lock().unwrap();
        loop {
            if let Some(raw) = q.pop_front() {
                return Ok(raw);
            }
            if self.shared.ev.dead.load(Ordering::SeqCst) {
                return Err(MakcuError::Connection("event reader died".into()));
            }
            if Instant::now() >= deadline {
                return Ok(Vec::new());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            let (guard, _) = self
                .shared
                .ev
                .reply_cv
                .wait_timeout(q, remaining.min(POLL))
                .map_err(|e| MakcuError::Connection(e.to_string()))?;
            q = guard;
        }
    }

    fn exchange_query(&self, payload: &[u8]) -> Result<Vec<u8>> {
        self.note_tx(payload);
        if self.shared.ev.running.load(Ordering::SeqCst) {
            let raw = self.request(payload, self.shared.timeout)?;
            self.note_rx(&raw);
            if raw.is_empty() {
                return Err(MakcuError::Timeout);
            }
            if !contains(&raw, PROMPT) {
                return Err(MakcuError::Protocol(format!("truncated: {raw:?}")));
            }
            return Ok(raw);
        }
        let mut slot = self.shared.port.lock().unwrap();
        let Some(port) = slot.as_mut() else {
            let err = MakcuError::Connection("transport not connected".into());
            self.note_error(&err);
            return Err(err);
        };
        let _ = port.clear(serialport::ClearBuffer::Input);
        port.write_all(payload)
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        port.flush()
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        let raw = read_until(&mut **port, PROMPT, self.shared.timeout)?;
        self.note_rx(&raw);
        if raw.is_empty() {
            return Err(MakcuError::Timeout);
        }
        if !contains(&raw, PROMPT) {
            return Err(MakcuError::Protocol(format!("truncated: {raw:?}")));
        }
        Ok(raw)
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn read_until(
    port: &mut dyn serialport::SerialPort,
    delim: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>> {
    let prev = port.timeout();
    if port.set_timeout(POLL).is_err() {
        return Err(MakcuError::Connection("set_timeout failed".into()));
    }
    let out = read_until_poll(port, delim, timeout);
    let _ = port.set_timeout(prev);
    out
}

fn read_until_poll(
    port: &mut dyn serialport::SerialPort,
    delim: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>> {
    let deadline = Instant::now() + timeout;
    let mut out = Vec::new();
    let mut buf = [0u8; 256];
    loop {
        if contains(&out, delim) {
            return Ok(out);
        }
        if Instant::now() >= deadline {
            return Ok(out);
        }
        match port.read(&mut buf) {
            Ok(0) => std::thread::sleep(Duration::from_millis(2)),
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => return Err(MakcuError::Connection(e.to_string())),
        }
    }
}

fn read_exact_n(
    port: &mut dyn serialport::SerialPort,
    n: usize,
    timeout: Duration,
) -> Result<Vec<u8>> {
    let prev = port.timeout();
    if port.set_timeout(POLL).is_err() {
        return Err(MakcuError::Connection("set_timeout failed".into()));
    }
    let out = read_exact_poll(port, n, timeout);
    let _ = port.set_timeout(prev);
    out
}

fn read_exact_poll(
    port: &mut dyn serialport::SerialPort,
    n: usize,
    timeout: Duration,
) -> Result<Vec<u8>> {
    let deadline = Instant::now() + timeout;
    let mut out = Vec::with_capacity(n);
    let mut buf = [0u8; 256];
    loop {
        if out.len() >= n {
            out.truncate(n);
            return Ok(out);
        }
        if Instant::now() >= deadline {
            return Ok(out);
        }
        match port.read(&mut buf) {
            Ok(0) => std::thread::sleep(Duration::from_millis(2)),
            Ok(m) => out.extend_from_slice(&buf[..m]),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => return Err(MakcuError::Connection(e.to_string())),
        }
    }
}

fn reader_loop(shared: Arc<Shared>) {
    // Take the port lock only around each individual read: writers must
    // be able to interleave, otherwise request() deadlocks on the lock.
    let clean_exit = (|| {
        {
            let mut slot = shared.port.lock().ok()?;
            let port = slot.as_mut()?;
            port.set_timeout(POLL).ok()?;
        }
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 256];
        loop {
            if shared.ev.stop.load(Ordering::SeqCst) {
                return Some(());
            }
            let n = {
                let mut slot = shared.port.lock().ok()?;
                let port = slot.as_mut()?;
                match port.read(&mut chunk) {
                    Ok(n) => n,
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => 0,
                    Err(_) => return None,
                }
            };
            if n > 0 {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() > 65536 {
                    buf.drain(..buf.len() - 65536);
                }
                let (replies, events, rest) = makcu_core::protocol::split_frames(&buf);
                buf = rest;
                if !replies.is_empty() {
                    let mut q = shared.ev.replies.lock().unwrap();
                    q.extend(replies);
                    shared.ev.reply_cv.notify_all();
                }
                if !events.is_empty() {
                    let mut q = shared.ev.events.lock().unwrap();
                    q.extend(events);
                    shared.ev.event_cv.notify_all();
                }
            } else {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    })();
    if clean_exit.is_none() {
        // Serial error (not a requested stop): mark the loop dead so
        // requesters fail fast with a connection error instead of
        // timing out against a dead reader.
        shared.ev.dead.store(true, Ordering::SeqCst);
        shared.ev.reply_cv.notify_all();
        shared.ev.event_cv.notify_all();
    }
    if let Ok(mut slot) = shared.port.lock() {
        if let Some(port) = slot.as_mut() {
            let _ = port.set_timeout(shared.timeout);
        }
    }
}

impl Drop for MakcuTransport {
    fn drop(&mut self) {
        // Best-effort: never leak the reader thread via the Arc it holds.
        if self.shared.ev.running.load(Ordering::SeqCst) {
            let _ = self.stop_event_loop();
        }
    }
}

impl Transport for MakcuTransport {
    fn query(&self, name: &str, args: &[&str]) -> Result<String> {
        let start = Instant::now();
        let payload = build_cmd(name, args);
        let raw = match self.exchange_query(&payload) {
            Err(MakcuError::Timeout) => self.exchange_query(&payload)?,
            other => other?,
        };
        let out = match makcu_core::protocol::strip_prompt(&raw) {
            Ok(s) => s,
            Err(e) => {
                self.note_error(&e);
                return Err(e);
            }
        };
        self.note_rtt(start.elapsed());
        Ok(out)
    }

    fn set(&self, name: &str, args: &[&str]) -> Result<()> {
        if self.is_no_wait() {
            return self.fire_and_forget(name, args);
        }
        self.send_raw(&build_cmd(name, args)).map_err(|e| match e {
            MakcuError::Protocol(msg) => {
                let wrapped = MakcuError::Protocol(format!("{name} failed: {msg}"));
                self.note_error(&wrapped);
                wrapped
            }
            other => {
                self.note_error(&other);
                other
            }
        })
    }

    fn send_raw(&self, data: &[u8]) -> Result<()> {
        self.note_tx(data);
        if self.shared.ev.running.load(Ordering::SeqCst) {
            let raw = self.request(data, ERROR_WINDOW)?;
            self.note_rx(&raw);
            if contains(&raw, b"ERR") {
                let err = MakcuError::Protocol(format!("rejected: {raw:?}"));
                self.note_error(&err);
                return Err(err);
            }
            return Ok(());
        }
        // Single critical section: another op must not interleave between
        // the write and the ERR-window read, or it would consume our reply.
        let mut slot = self.shared.port.lock().unwrap();
        let Some(port) = slot.as_mut() else {
            let err = MakcuError::Connection("transport not connected".into());
            self.note_error(&err);
            return Err(err);
        };
        let _ = port.clear(serialport::ClearBuffer::Input);
        let write_res = port
            .write_all(data)
            .map_err(|e| MakcuError::Connection(e.to_string()))
            .and_then(|()| {
                port.flush()
                    .map_err(|e| MakcuError::Connection(e.to_string()))
            })
            .and_then(|()| read_until(&mut **port, PROMPT, ERROR_WINDOW));
        let raw = match write_res {
            Ok(raw) => raw,
            Err(e) => {
                self.note_error(&e);
                return Err(e);
            }
        };
        self.note_rx(&raw);
        if contains(&raw, b"ERR") {
            let err = MakcuError::Protocol(format!("rejected: {raw:?}"));
            self.note_error(&err);
            return Err(err);
        }
        Ok(())
    }

    fn read_line(&self, timeout: Duration) -> Result<Vec<u8>> {
        let mut slot = self.shared.port.lock().unwrap();
        let Some(port) = slot.as_mut() else {
            return Err(MakcuError::Connection("transport not connected".into()));
        };
        let raw = read_until(&mut **port, b"\r\n", timeout)?;
        if raw.is_empty() {
            return Err(MakcuError::Timeout);
        }
        Ok(raw)
    }

    fn read_exact(&self, n: usize, timeout: Duration) -> Result<Vec<u8>> {
        if n == 0 {
            return Err(MakcuError::InvalidArgument("read_exact n=0".into()));
        }
        let mut slot = self.shared.port.lock().unwrap();
        let Some(port) = slot.as_mut() else {
            return Err(MakcuError::Connection("transport not connected".into()));
        };
        let out = read_exact_n(&mut **port, n, timeout)?;
        if out.len() < n {
            return Err(MakcuError::Timeout);
        }
        Ok(out)
    }

    fn start_event_loop(&self) -> Result<()> {
        if self.shared.ev.running.load(Ordering::SeqCst) {
            return Ok(());
        }
        {
            let mut slot = self.shared.port.lock().unwrap();
            let Some(port) = slot.as_mut() else {
                return Err(MakcuError::Connection("transport not connected".into()));
            };
            let _ = port.clear(serialport::ClearBuffer::All);
        }
        self.shared.ev.stop.store(false, Ordering::SeqCst);
        self.shared.ev.dead.store(false, Ordering::SeqCst);
        let shared = Arc::clone(&self.shared);
        let handle = std::thread::Builder::new()
            .name("makcu-events".into())
            .spawn(move || reader_loop(shared))
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        *self.shared.ev.handle.lock().unwrap() = Some(handle);
        self.shared.ev.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn stop_event_loop(&self) -> Result<()> {
        self.shared.ev.stop.store(true, Ordering::SeqCst);
        let handle = self.shared.ev.handle.lock().unwrap().take();
        if let Some(h) = handle {
            let _ = h.join();
        }
        self.shared.ev.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn event_loop_running(&self) -> bool {
        self.shared.ev.running.load(Ordering::SeqCst)
    }

    fn poll_event(&self, timeout: Duration) -> Result<Vec<u8>> {
        let ev = &self.shared.ev;
        if !ev.running.load(Ordering::SeqCst) {
            return Err(MakcuError::Protocol("event loop not running".into()));
        }
        if ev.dead.load(Ordering::SeqCst) {
            return Err(MakcuError::Connection("event reader died".into()));
        }
        let deadline = Instant::now() + timeout;
        let mut q = self.shared.ev.events.lock().unwrap();
        loop {
            if let Some(raw) = q.pop_front() {
                return Ok(raw);
            }
            if Instant::now() >= deadline {
                return Err(MakcuError::Timeout);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            let (guard, _) = self
                .shared
                .ev
                .event_cv
                .wait_timeout(q, remaining.min(POLL))
                .map_err(|e| MakcuError::Connection(e.to_string()))?;
            q = guard;
        }
    }

    fn fire_and_forget(&self, name: &str, args: &[&str]) -> Result<()> {
        let payload = build_cmd(name, args);
        let mut slot = self.shared.port.lock().unwrap();
        let Some(port) = slot.as_mut() else {
            let err = MakcuError::Connection("transport not connected".into());
            self.note_error(&err);
            return Err(err);
        };
        let res = port
            .write_all(&payload)
            .map_err(|e| MakcuError::Connection(e.to_string()))
            .and_then(|()| {
                port.flush()
                    .map_err(|e| MakcuError::Connection(e.to_string()))
            });
        if let Err(e) = res {
            self.note_error(&e);
            return Err(e);
        }
        self.note_tx(&payload);
        Ok(())
    }

    fn get_stats(&self) -> TransportStats {
        let obs = self.shared.obs.lock().unwrap();
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
        let mut obs = self.shared.obs.lock().unwrap();
        obs.tx_bytes = 0;
        obs.cmds_sent = 0;
        obs.reconnects = 0;
        obs.rtt_total = Duration::ZERO;
        obs.rtt_count = 0;
        obs.last_error = None;
    }

    fn get_recent_lines(&self, n: usize) -> Vec<String> {
        let obs = self.shared.obs.lock().unwrap();
        let len = obs.recent.len();
        obs.recent
            .iter()
            .skip(len.saturating_sub(n))
            .cloned()
            .collect()
    }

    fn enter_no_wait(&self) {
        self.shared.obs.lock().unwrap().no_wait_depth += 1;
    }

    fn exit_no_wait(&self) {
        let mut obs = self.shared.obs.lock().unwrap();
        obs.no_wait_depth = obs.no_wait_depth.saturating_sub(1);
    }

    fn is_no_wait(&self) -> bool {
        self.shared.obs.lock().unwrap().no_wait_depth > 0
    }

    fn on_connection_change(&self, cb: ConnectionCallback) -> ConnectionCallback {
        self.shared.conn_cbs.lock().unwrap().push(cb.clone());
        cb
    }

    fn remove_connection_callback(&self, cb: &ConnectionCallback) {
        let mut cbs = self.shared.conn_cbs.lock().unwrap();
        if let Some(pos) = cbs.iter().position(|c| ConnectionCallback::ptr_eq(c, cb)) {
            cbs.remove(pos);
        }
    }
}
