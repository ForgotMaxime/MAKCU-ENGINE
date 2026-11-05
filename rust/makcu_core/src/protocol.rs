use crate::{types::Button, DeviceInfo, MakcuError, Position, Result, Screen};

pub fn build_cmd(name: &str, args: &[&str]) -> Vec<u8> {
    let mut s = String::with_capacity(name.len() + args.len() * 6 + 3);
    s.push_str(name);
    s.push('(');
    s.push_str(&args.join(","));
    s.push(')');
    s.push('\r');
    s.into_bytes()
}

pub fn strip_prompt(raw: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(raw)
        .map_err(|e| MakcuError::Protocol(format!("non-ascii reply: {e}")))?;
    let Some(head) = text.split(">>>").next() else {
        return Err(MakcuError::Protocol(format!("missing prompt in {raw:?}")));
    };
    if !text.contains(">>>") {
        return Err(MakcuError::Protocol(format!("missing prompt in {raw:?}")));
    }
    Ok(head.trim().replace(['\r', '\n'], ""))
}

pub fn parse_device(payload: &str) -> DeviceInfo {
    let mut route = String::new();
    let mut mouse = String::new();
    let mut kbd = String::new();
    let mut ctrl = String::new();
    for part in payload.split(';') {
        if let Some((k, v)) = part.split_once(':') {
            match k {
                "R" => route = v.to_string(),
                "M" => mouse = v.to_string(),
                "K" => kbd = v.to_string(),
                "C" => ctrl = v.to_string(),
                _ => {}
            }
        }
    }
    DeviceInfo {
        route,
        mouse_period: mouse,
        keyboard_period: kbd,
        controller_period: ctrl,
    }
}

fn parse_pair(payload: &str) -> Result<(i32, i32)> {
    let l = payload
        .find('(')
        .ok_or_else(|| MakcuError::Protocol("missing (".into()))?;
    let r = payload
        .find(')')
        .ok_or_else(|| MakcuError::Protocol("missing )".into()))?;
    let inner = &payload[l + 1..r];
    let (a, b) = inner
        .split_once(',')
        .ok_or_else(|| MakcuError::Protocol("missing comma".into()))?;
    let x: i32 = a
        .trim()
        .parse()
        .map_err(|_| MakcuError::Protocol("bad int".into()))?;
    let y: i32 = b
        .trim()
        .parse()
        .map_err(|_| MakcuError::Protocol("bad int".into()))?;
    Ok((x, y))
}

pub fn parse_screen(payload: &str) -> Result<Screen> {
    let (w, h) = parse_pair(payload)?;
    check_screen_size(w, h)?;
    Ok(Screen {
        width: w as u16,
        height: h as u16,
    })
}

pub fn parse_getpos(payload: &str) -> Result<Position> {
    let (x, y) = parse_pair(payload)?;
    Ok(Position { x, y })
}

pub fn clamp_i16(v: i32) -> Result<i16> {
    i16::try_from(v).map_err(|_| MakcuError::InvalidArgument(format!("out of i16 range: {v}")))
}

pub fn check_xy(x: i32, y: i32) -> Result<()> {
    if !(0..=32767).contains(&x) {
        return Err(MakcuError::InvalidArgument(format!("x: {x}")));
    }
    if !(0..=32767).contains(&y) {
        return Err(MakcuError::InvalidArgument(format!("y: {y}")));
    }
    Ok(())
}

pub fn check_screen_size(w: i32, h: i32) -> Result<()> {
    if !(1..=32767).contains(&w) || !(1..=32767).contains(&h) {
        return Err(MakcuError::InvalidArgument(format!("screen: {w}x{h}")));
    }
    Ok(())
}

pub fn check_mask_bit(v: i32) -> Result<()> {
    if v != 0 && v != 1 {
        return Err(MakcuError::InvalidArgument(format!("mask 0/1: {v}")));
    }
    Ok(())
}

pub fn check_click(button: u32, count: u32, hold_ms: Option<u32>) -> Result<()> {
    if !(1..=5).contains(&button) {
        return Err(MakcuError::InvalidArgument(format!("button: {button}")));
    }
    if !(1..=255).contains(&count) {
        return Err(MakcuError::InvalidArgument(format!("count: {count}")));
    }
    if let Some(h) = hold_ms {
        if !(1..=5000).contains(&h) {
            return Err(MakcuError::InvalidArgument(format!("hold_ms: {h}")));
        }
    }
    Ok(())
}

pub fn check_stream_mode(mode: u8) -> Result<()> {
    if !(1..=3).contains(&mode) {
        return Err(MakcuError::InvalidArgument(format!("stream mode: {mode}")));
    }
    Ok(())
}

pub fn check_autorelease(timeout_ms: u32) -> Result<()> {
    if timeout_ms > 60000 {
        return Err(MakcuError::InvalidArgument(format!(
            "autorelease: {timeout_ms}"
        )));
    }
    Ok(())
}

pub fn button_method(b: Button) -> &'static str {
    match b {
        Button::Left => "left",
        Button::Right => "right",
        Button::Middle => "middle",
        Button::Side1 => "side1",
        Button::Side2 => "side2",
    }
}

pub fn mask_method(b: Button) -> &'static str {
    match b {
        Button::Left => "left_mask",
        Button::Right => "right_mask",
        Button::Middle => "middle_mask",
        Button::Side1 => "side1_mask",
        Button::Side2 => "side2_mask",
    }
}

pub fn parse_button_event(raw: &[u8]) -> Result<u8> {
    if raw.len() == 6 && raw.starts_with(b"km.") && raw.ends_with(b"\r\n") {
        return Ok(raw[3]);
    }
    Err(MakcuError::Protocol(format!("bad button frame: {raw:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build() {
        assert_eq!(build_cmd("km.move", &["10", "-3"]), b"km.move(10,-3)\r");
        assert_eq!(build_cmd("km.version", &[]), b"km.version()\r");
    }

    #[test]
    fn strip() {
        assert_eq!(strip_prompt(b"km.MAKCU\r\n>>> ").unwrap(), "km.MAKCU");
        assert_eq!(strip_prompt(b"0\r\n>>> ").unwrap(), "0");
    }

    #[test]
    fn device() {
        let d = parse_device("R:MK;M:16uf;K:8uf;C:0uf");
        assert_eq!(d.route, "MK");
        assert_eq!(d.mouse_period, "16uf");
    }

    #[test]
    fn screen_pos() {
        assert_eq!(parse_screen("km.screen(1920,1080)").unwrap().width, 1920);
        let p = parse_getpos("km.getpos(123,456)").unwrap();
        assert_eq!((p.x, p.y), (123, 456));
    }

    #[test]
    fn clamp() {
        assert!(clamp_i16(40000).is_err());
        assert_eq!(clamp_i16(-32768).unwrap(), -32768);
    }

    #[test]
    fn button_event() {
        assert_eq!(parse_button_event(b"km.\x03\r\n").unwrap(), 0x03);
        assert!(parse_button_event(b"garbage\r\n").is_err());
    }
}
