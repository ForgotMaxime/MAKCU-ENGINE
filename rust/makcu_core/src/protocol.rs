//! Pure protocol helpers: framing, parsing and shared validators.
//! No I/O. Every function is unit-tested; all failures are [`MakcuError`].

use crate::{types::Button, DeviceInfo, MakcuError, Position, Result, Screen};

/// Frame one ASCII command: `name(a,b)` + carriage return.
pub fn build_cmd(name: &str, args: &[&str]) -> Vec<u8> {
    let mut s = String::with_capacity(name.len() + args.len() * 6 + 3);
    s.push_str(name);
    s.push('(');
    s.push_str(&args.join(","));
    s.push(')');
    s.push('\r');
    s.into_bytes()
}

/// Strip the trailing `>>> ` prompt and a possible echo line.
///
/// Echo off: `<result>\r\n>>> ` yields the result. Echo on:
/// `<input>\r\n<result>\r\n>>> ` yields the result (last line wins);
/// `km.version()` is special and never echoes.
pub fn strip_prompt(raw: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(raw)
        .map_err(|e| MakcuError::Protocol(format!("non-ascii reply: {e}")))?;
    if !text.contains(">>>") {
        return Err(MakcuError::Protocol(format!("missing prompt in {raw:?}")));
    }
    let payload = text.split(">>>").next().unwrap_or("");
    let normalized = payload.replace('\r', "\n");
    let last = normalized
        .split('\n')
        .map(str::trim)
        .rfind(|l: &&str| !l.is_empty())
        .ok_or_else(|| MakcuError::Protocol(format!("empty reply: {raw:?}")))?;
    Ok(last.to_string())
}

/// Parse an `R:..;M:..;K:..;C:..` device string.
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

/// Parse and range-check a `km.screen(w,h)` reply.
pub fn parse_screen(payload: &str) -> Result<Screen> {
    let (w, h) = parse_pair(payload)?;
    check_screen_size(w, h)?;
    Ok(Screen {
        width: w as u16,
        height: h as u16,
    })
}

/// Parse a `km.getpos(x,y)` reply.
pub fn parse_getpos(payload: &str) -> Result<Position> {
    let (x, y) = parse_pair(payload)?;
    Ok(Position { x, y })
}

/// Range-check an i16 command argument.
pub fn clamp_i16(v: i32) -> Result<i16> {
    i16::try_from(v).map_err(|_| MakcuError::InvalidArgument(format!("out of i16 range: {v}")))
}

/// Range-check virtual-screen coordinates (0..=32767).
pub fn check_xy(x: i32, y: i32) -> Result<()> {
    if !(0..=32767).contains(&x) {
        return Err(MakcuError::InvalidArgument(format!("x: {x}")));
    }
    if !(0..=32767).contains(&y) {
        return Err(MakcuError::InvalidArgument(format!("y: {y}")));
    }
    Ok(())
}

/// Range-check virtual-screen dimensions (1..=32767).
pub fn check_screen_size(w: i32, h: i32) -> Result<()> {
    if !(1..=32767).contains(&w) || !(1..=32767).contains(&h) {
        return Err(MakcuError::InvalidArgument(format!("screen: {w}x{h}")));
    }
    Ok(())
}

/// Range-check a 0/1 mask argument.
pub fn check_mask_bit(v: i32) -> Result<()> {
    if v != 0 && v != 1 {
        return Err(MakcuError::InvalidArgument(format!("mask 0/1: {v}")));
    }
    Ok(())
}

/// Validate click button/count/hold arguments.
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

/// Validate a button-stream mode (1..=3).
pub fn check_stream_mode(mode: u8) -> Result<()> {
    if !(1..=3).contains(&mode) {
        return Err(MakcuError::InvalidArgument(format!("stream mode: {mode}")));
    }
    Ok(())
}

/// Validate an autorelease timeout (0..=60000).
pub fn check_autorelease(timeout_ms: u32) -> Result<()> {
    if timeout_ms > 60000 {
        return Err(MakcuError::InvalidArgument(format!(
            "autorelease: {timeout_ms}"
        )));
    }
    Ok(())
}

/// Wire name for a button (`left`, `right`, …).
pub fn button_method(b: Button) -> &'static str {
    match b {
        Button::Left => "left",
        Button::Right => "right",
        Button::Middle => "middle",
        // Side buttons use the ms1/ms2 names (like the lock targets and
        // the reference library); the firmware also accepts side1/side2.
        Button::Side1 => "ms1",
        Button::Side2 => "ms2",
    }
}

/// Wire name for a button mask (`left_mask`, …).
pub fn mask_method(b: Button) -> &'static str {
    match b {
        Button::Left => "left_mask",
        Button::Right => "right_mask",
        Button::Middle => "middle_mask",
        Button::Side1 => "side1_mask",
        Button::Side2 => "side2_mask",
    }
}

/// Parse a V4 button stream frame: bare mask byte (mode 3) or
/// `b"km."` + mask + CRLF (modes 1/2, V4.036+).
pub fn parse_button_event(raw: &[u8]) -> Result<u8> {
    if raw.len() == 1 {
        return Ok(raw[0]);
    }
    if raw.len() == 6 && raw.starts_with(b"km.") && raw.ends_with(b"\r\n") {
        return Ok(raw[3]);
    }
    Err(MakcuError::Protocol(format!("bad button frame: {raw:?}")))
}

/// Split a raw byte stream into complete frames.
/// Returns (replies, events, rest). A reply runs through the end of the
/// first `>>> ` prompt. A CRLF-terminated line *before* that prompt is an
/// event only if it matches the stream frame shape — otherwise the whole
/// reply (echo + prompt) is consumed as one unit, so interleaved
/// `frame + reply` input demultiplexes correctly. Bare bytes < 0x20 with
/// no terminator pending are mode-3 masks and go out as single-byte
/// events (`\r`/`\n` alone are kept: they may complete a split CRLF).
pub fn split_frames(buf: &[u8]) -> (Vec<Vec<u8>>, Vec<Vec<u8>>, Vec<u8>) {
    let mut replies = Vec::new();
    let mut events = Vec::new();
    let mut rest = buf.to_vec();
    loop {
        if let Some(i) = find_sub(&rest, b">>> ") {
            let head_end = i;
            if let Some(j) = find_sub(&rest[..head_end], b"\r\n") {
                if is_stream_frame(&rest[..j + 2]) {
                    events.push(rest.drain(..j + 2).collect());
                    continue;
                }
            }
            let end = i + 4;
            replies.push(rest.drain(..end).collect());
            continue;
        }
        if let Some(i) = find_sub(&rest, b"\r\n") {
            if is_stream_frame(&rest[..i + 2]) {
                events.push(rest.drain(..i + 2).collect());
                continue;
            }
            break;
        }
        if rest
            .first()
            .is_some_and(|&b| b < 0x20 && b != b'\r' && b != b'\n')
        {
            events.push(rest.drain(..1).collect());
            continue;
        }
        return (replies, events, rest);
    }
    (replies, events, rest)
}

/// Prefixed stream frame: `b"km."` + mask (< 0x20) + CRLF.
///
/// The < 0x20 threshold separates a mask byte from command echo text
/// (letters are >= 0x61).
fn is_stream_frame(line: &[u8]) -> bool {
    line.len() == 6 && line.starts_with(b"km.") && line[3] < 0x20 && line.ends_with(b"\r\n")
}

fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
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
        assert_eq!(strip_prompt(b"km.left()\r\n0\r\n>>> ").unwrap(), "0");
        assert_eq!(
            strip_prompt(b"km.version()\r\nkm.MAKCU\r\n>>> ").unwrap(),
            "km.MAKCU"
        );
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
        assert_eq!(parse_button_event(b"\x05").unwrap(), 0x05);
        assert!(parse_button_event(b"garbage\r\n").is_err());
    }

    #[test]
    fn split() {
        let (r, e, rest) = split_frames(b"km.MAKCU\r\n>>> ");
        assert_eq!(r, vec![b"km.MAKCU\r\n>>> ".to_vec()]);
        assert!(e.is_empty() && rest.is_empty());
        let (r, e, rest) = split_frames(b"km.\x03\r\n");
        assert!(r.is_empty() && e == vec![b"km.\x03\r\n".to_vec()] && rest.is_empty());
        let (r, e, rest) = split_frames(b"0\r\n>>> km.\x01\r\nkm.MA");
        assert_eq!(r, vec![b"0\r\n>>> ".to_vec()]);
        assert_eq!(e, vec![b"km.\x01\r\n".to_vec()]);
        assert_eq!(rest, b"km.MA");
        // Bare mode-3 byte with no terminator pending.
        let (r, e, rest) = split_frames(b"\x05");
        assert!(r.is_empty() && e == vec![b"\x05".to_vec()] && rest.is_empty());
        // Echo line before prompt belongs to the reply, never an event.
        let (r, e, rest) = split_frames(b"km.left(1)\r\n>>> ");
        assert_eq!(r, vec![b"km.left(1)\r\n>>> ".to_vec()]);
        assert!(e.is_empty() && rest.is_empty());
        // Interleaved event before reply.
        let (r, e, rest) = split_frames(b"km.\x01\r\n0\r\n>>> ");
        assert_eq!(r, vec![b"0\r\n>>> ".to_vec()]);
        assert_eq!(e, vec![b"km.\x01\r\n".to_vec()]);
        assert!(rest.is_empty());
    }
}
