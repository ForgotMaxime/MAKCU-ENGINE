//! Transport abstraction: queries, silent setters, batches and streams.
//!
//! [`Transport`] is implemented by the serial driver and the in-memory mock.

use crate::Result;

/// Serial-or-mock I/O used by [`Mouse`](crate::Mouse).
pub trait Transport {
    /// Blocking query; returns the prompt-stripped payload.
    fn query(&self, name: &str, args: &[&str]) -> Result<String>;
    /// Silent setter; errors surface through the ERR window.
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
}
