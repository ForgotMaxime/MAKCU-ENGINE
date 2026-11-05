//! Background button-event pump: change callbacks plus drain queue.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{Mouse, Transport};

type Callback = Box<dyn Fn(u8) + Send + 'static>;

/// Background button-event pump: change callbacks + drain queue.
/// Works with the demux event loop running (polls never block queries)
/// or on the legacy direct-poll path.
/// Background button-event pump: change callbacks plus drain queue.
pub struct ButtonMonitor<T: Transport> {
    mouse: Arc<Mouse<T>>,
    poll: Duration,
    stop: Arc<std::sync::atomic::AtomicBool>,
    cbs: Arc<Mutex<Vec<Callback>>>,
    pending: Arc<Mutex<Vec<u8>>>,
    last: Arc<Mutex<Option<u8>>>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl<T: Transport + Send + Sync + 'static> ButtonMonitor<T> {
    /// Monitor a shared mouse; pump every `poll`.
    pub fn new(mouse: Arc<Mouse<T>>, poll: Duration) -> Self {
        Self {
            mouse,
            poll,
            stop: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            cbs: Arc::new(Mutex::new(Vec::new())),
            pending: Arc::new(Mutex::new(Vec::new())),
            last: Arc::new(Mutex::new(None)),
            handle: Mutex::new(None),
        }
    }

    /// Register a callback invoked on each mask change.
    pub fn on_change(&self, cb: impl Fn(u8) + Send + 'static) {
        self.cbs.lock().unwrap().push(Box::new(cb));
    }

    /// Enable the button stream and spawn the pump thread.
    pub fn start(&self, mode: u8) -> crate::Result<()> {
        self.mouse.start_button_stream(mode)?;
        self.stop.store(false, std::sync::atomic::Ordering::SeqCst);
        let mouse = Arc::clone(&self.mouse);
        let cbs = Arc::clone(&self.cbs);
        let pending = Arc::clone(&self.pending);
        let last = Arc::clone(&self.last);
        let stop = Arc::clone(&self.stop);
        let poll = self.poll;
        let handle = std::thread::Builder::new()
            .name("makcu-monitor".into())
            .spawn(move || {
                while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                    match mouse.poll_button_event(poll) {
                        Ok(mask) => {
                            let mut last = last.lock().unwrap();
                            if *last != Some(mask) {
                                *last = Some(mask);
                                pending.lock().unwrap().push(mask);
                                for cb in cbs.lock().unwrap().iter() {
                                    cb(mask);
                                }
                            }
                        }
                        Err(_) => std::thread::sleep(poll),
                    }
                }
            })
            .map_err(|e| crate::MakcuError::Connection(e.to_string()))?;
        *self.handle.lock().unwrap() = Some(handle);
        Ok(())
    }

    /// Stop the pump thread and disable the button stream.
    pub fn stop(&self) -> crate::Result<()> {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(h) = self.handle.lock().unwrap().take() {
            let _ = h.join();
        }
        self.mouse.stop_button_stream()
    }

    /// Poll once, dispatch on change. Testable without threads.
    pub fn pump_once(&self, timeout: Duration) -> crate::Result<Option<u8>> {
        match self.mouse.poll_button_event(timeout) {
            Err(crate::MakcuError::Timeout) => Ok(None),
            Err(e) => Err(e),
            Ok(mask) => {
                let mut last = self.last.lock().unwrap();
                if *last != Some(mask) {
                    *last = Some(mask);
                    self.pending.lock().unwrap().push(mask);
                    for cb in self.cbs.lock().unwrap().iter() {
                        cb(mask);
                    }
                }
                Ok(Some(mask))
            }
        }
    }

    /// Take all pending change events since the last drain.
    pub fn drain(&self) -> Vec<u8> {
        std::mem::take(&mut *self.pending.lock().unwrap())
    }
}
