use std::time::Duration;

use makcu_serial::{MakcuTransport, Mouse};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let t = MakcuTransport::open_auto(Duration::from_millis(800))?;
    println!("MAKCU on {} @ {}", t.port_name(), t.baud());
    let m = Mouse::new(&t);
    println!("version: {}", m.version()?);
    println!("device: {:?}", m.device()?);
    println!(
        "screen: {:?} pos: {:?} moving: {}",
        m.screen()?,
        m.getpos()?,
        m.moving()?
    );
    let p0 = m.getpos()?;
    m.move_rel(2, 0)?;
    m.move_rel(-2, 0)?;
    println!("move OK ({} -> {:?})", p0.x, m.getpos()?);
    Ok(())
}
