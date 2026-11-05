use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant};

#[allow(unused_imports)]
use std::io::{Read, Write};

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

struct EventState {
    running: AtomicBool,
    stop: AtomicBool,
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
            replies: Mutex::new(VecDeque::new()),
            reply_cv: Condvar::new(),
            events: Mutex::new(VecDeque::new()),
            event_cv: Condvar::new(),
            handle: Mutex::new(None),
        }
    }
}

struct Shared {
    port: Mutex<Box<dyn serialport::SerialPort>>,
    /// Serializes request/response pairs when the event loop is active.
    req: Mutex<()>,
    timeout: Duration,
    port_name: String,
    baud: u32,
    ev: EventState,
}

/// Exclusive serial transport with optional demux event loop.
#[derive(Clone)]
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
                            port: Mutex::new(p),
                            req: Mutex::new(()),
                            timeout,
                            port_name: port.to_string(),
                            baud,
                            ev: EventState::new(),
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
    /// setter (echo off), like the legacy direct path.
    fn request(&self, payload: &[u8], timeout: Duration) -> Result<Vec<u8>> {
        let _guard = self.shared.req.lock().unwrap();
        {
            let mut port = self.shared.port.lock().unwrap();
            let _ = port.clear(serialport::ClearBuffer::All);
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
        if self.shared.ev.running.load(Ordering::SeqCst) {
            let raw = self.request(payload, self.shared.timeout)?;
            if raw.is_empty() {
                return Err(MakcuError::Timeout);
            }
            if !contains(&raw, PROMPT) {
                return Err(MakcuError::Protocol(format!("truncated: {raw:?}")));
            }
            return Ok(raw);
        }
        let mut port = self.shared.port.lock().unwrap();
        let _ = port.clear(serialport::ClearBuffer::Input);
        port.write_all(payload)
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        port.flush()
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        let raw = read_until(&mut **port, PROMPT, self.shared.timeout)?;
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
    {
        let mut port = shared.port.lock().unwrap();
        let _ = port.set_timeout(POLL);
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 256];
    loop {
        if shared.ev.stop.load(Ordering::SeqCst) {
            break;
        }
        let n = {
            let mut port = match shared.port.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            match port.read(&mut chunk) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => 0,
                Err(_) => break,
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
    let mut port = match shared.port.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let _ = port.set_timeout(shared.timeout);
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
        let raw = match self.exchange_query(&build_cmd(name, args)) {
            Err(MakcuError::Timeout) => self.exchange_query(&build_cmd(name, args))?,
            other => other?,
        };
        makcu_core::protocol::strip_prompt(&raw)
    }

    fn set(&self, name: &str, args: &[&str]) -> Result<()> {
        self.send_raw(&build_cmd(name, args)).map_err(|e| match e {
            MakcuError::Protocol(msg) => MakcuError::Protocol(format!("{name} failed: {msg}")),
            other => other,
        })
    }

    fn send_raw(&self, data: &[u8]) -> Result<()> {
        if self.shared.ev.running.load(Ordering::SeqCst) {
            let raw = self.request(data, ERROR_WINDOW)?;
            if contains(&raw, b"ERR") {
                return Err(MakcuError::Protocol(format!("rejected: {raw:?}")));
            }
            return Ok(());
        }
        // Single critical section: another op must not interleave between
        // the write and the ERR-window read, or it would consume our reply.
        let mut port = self.shared.port.lock().unwrap();
        let _ = port.clear(serialport::ClearBuffer::Input);
        port.write_all(data)
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        port.flush()
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        let raw = read_until(&mut **port, PROMPT, ERROR_WINDOW)?;
        if contains(&raw, b"ERR") {
            return Err(MakcuError::Protocol(format!("rejected: {raw:?}")));
        }
        Ok(())
    }

    fn read_line(&self, timeout: Duration) -> Result<Vec<u8>> {
        let mut port = self.shared.port.lock().unwrap();
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
        let mut port = self.shared.port.lock().unwrap();
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
            let port = self.shared.port.lock().unwrap();
            let _ = port.clear(serialport::ClearBuffer::All);
        }
        self.shared.ev.stop.store(false, Ordering::SeqCst);
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
        if !self.shared.ev.running.load(Ordering::SeqCst) {
            return Err(MakcuError::Protocol("event loop not running".into()));
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
        let mut port = self.shared.port.lock().unwrap();
        port.write_all(&build_cmd(name, args))
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        port.flush()
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        Ok(())
    }
}
