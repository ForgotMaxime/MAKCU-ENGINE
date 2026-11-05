use std::collections::HashMap;
use std::sync::Mutex;

use crate::{Result, Transport};

pub struct MockTransport {
    replies: HashMap<String, String>,
    sent: Mutex<Vec<String>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self {
            replies: HashMap::new(),
            sent: Mutex::new(Vec::new()),
        }
    }

    pub fn sent(&self) -> Vec<String> {
        self.sent.lock().unwrap().clone()
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
            "km.getpos" => "km.getpos(100,200)",
            "km.screen" => "km.screen(1920,1080)",
            _ => "",
        };
        Ok(d.to_string())
    }

    fn set(&self, name: &str, args: &[&str]) -> Result<()> {
        self.sent.lock().unwrap().push(fmt_cmd(name, args));
        Ok(())
    }

    fn fire_and_forget(&self, name: &str, args: &[&str]) -> Result<()> {
        self.sent.lock().unwrap().push(fmt_cmd(name, args));
        Ok(())
    }
}
