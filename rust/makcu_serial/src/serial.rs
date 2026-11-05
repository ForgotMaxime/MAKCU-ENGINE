use std::sync::Mutex;
use std::time::{Duration, Instant};

#[allow(unused_imports)]
use std::io::{Read, Write};

use makcu_core::{protocol::build_cmd, MakcuError, Result, Transport};

pub const BAUDS: [u32; 2] = [4_000_000, 115_200];
pub const USB_VID: u16 = 0x1A86;
pub const USB_PID: u16 = 0x55D3;
pub const PROMPT: &[u8] = b">>> ";
const ERROR_WINDOW: Duration = Duration::from_millis(80);

pub struct MakcuTransport {
    port_name: String,
    baud: u32,
    timeout: Duration,
    inner: Mutex<Box<dyn serialport::SerialPort>>,
}

impl MakcuTransport {
    pub fn port_name(&self) -> &str {
        &self.port_name
    }

    pub fn baud(&self) -> u32 {
        self.baud
    }

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
                        port_name: port.to_string(),
                        baud,
                        timeout,
                        inner: Mutex::new(p),
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

    pub fn open_auto(timeout: Duration) -> Result<Self> {
        let (port, baud) = Self::detect()?;
        std::thread::sleep(Duration::from_millis(300));
        Self::open(&port, baud, timeout)
    }

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

    fn exchange_query(&self, payload: &[u8]) -> Result<Vec<u8>> {
        let mut port = self.inner.lock().unwrap();
        let _ = port.clear(serialport::ClearBuffer::Input);
        port.write_all(payload)
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        port.flush()
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        let raw = read_until(&mut **port, PROMPT, self.timeout)?;
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

impl Transport for MakcuTransport {
    fn query(&self, name: &str, args: &[&str]) -> Result<String> {
        let raw = match self.exchange_query(&build_cmd(name, args)) {
            Err(MakcuError::Timeout) => self.exchange_query(&build_cmd(name, args))?,
            other => other?,
        };
        makcu_core::protocol::strip_prompt(&raw)
    }

    fn set(&self, name: &str, args: &[&str]) -> Result<()> {
        self.send_raw(&build_cmd(name, args))
    }

    fn send_raw(&self, data: &[u8]) -> Result<()> {
        let mut port = self.inner.lock().unwrap();
        let _ = port.clear(serialport::ClearBuffer::Input);
        port.write_all(data)
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        port.flush()
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        drop(port);
        let mut port = self.inner.lock().unwrap();
        let raw = read_until(&mut **port, PROMPT, ERROR_WINDOW)?;
        if contains(&raw, b"ERR") {
            return Err(MakcuError::Protocol(format!("rejected: {raw:?}")));
        }
        Ok(())
    }

    fn read_line(&self, timeout: Duration) -> Result<Vec<u8>> {
        let mut port = self.inner.lock().unwrap();
        let raw = read_until(&mut **port, b"\r\n", timeout)?;
        if raw.is_empty() {
            return Err(MakcuError::Timeout);
        }
        Ok(raw)
    }

    fn fire_and_forget(&self, name: &str, args: &[&str]) -> Result<()> {
        let mut port = self.inner.lock().unwrap();
        port.write_all(&build_cmd(name, args))
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        port.flush()
            .map_err(|e| MakcuError::Connection(e.to_string()))?;
        Ok(())
    }
}
