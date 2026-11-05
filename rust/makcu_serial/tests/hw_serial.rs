//! Hardware-gated tests. Ignored by default (CI has no MAKCU):
//! `cargo test -p makcu_serial -- --ignored`
//! Safe: queries, net-zero moves, one rejected setter for ERR timing.

use std::time::{Duration, Instant};

use makcu_serial::{MakcuTransport, Mouse, Transport};

fn open() -> MakcuTransport {
    // One physical port shared by all tests in this file: run with
    // cargo test -p makcu_serial -- --ignored --test-threads=1
    let (port, baud) = MakcuTransport::detect().expect("no MAKCU detected");
    MakcuTransport::open(&port, baud, Duration::from_millis(800)).expect("open")
}

#[test]
#[ignore]
fn hw_probe_and_version() {
    let t = open();
    let m = Mouse::new(&t);
    assert!(m.version().unwrap().contains("MAKCU"));
    assert!(!m.device().unwrap().route.is_empty());
}

#[test]
#[ignore]
fn hw_loop_queries_during_stream() {
    use makcu_serial::Mouse;
    let t = open();
    let m = Mouse::new(&t);
    t.start_event_loop().unwrap();
    m.start_button_stream(1).unwrap();
    assert!(m.version().unwrap().contains("MAKCU"));
    assert!(!m.device().unwrap().route.is_empty());
    m.move_rel(2, 0).unwrap();
    m.move_rel(-2, 0).unwrap();
    assert!(m.wait_idle(Duration::from_secs(2)).unwrap());
    m.stop_button_stream().unwrap();
    t.stop_event_loop().unwrap();
}

/// Restores echo-off on drop, even if the test panics mid-way.
struct EchoGuard<'a, T: makcu_serial::Transport> {
    mouse: &'a Mouse<T>,
}

impl<T: makcu_serial::Transport> Drop for EchoGuard<'_, T> {
    fn drop(&mut self) {
        let _ = self.mouse.set_echo(false);
    }
}

#[test]
#[ignore]
fn hw_echo_on_queries() {
    let t = open();
    let m = Mouse::new(&t);
    let v0 = m.version().unwrap();
    m.set_echo(true).unwrap();
    let _guard = EchoGuard { mouse: &m };
    assert_eq!(m.echo().unwrap(), 1);
    assert_eq!(m.version().unwrap(), v0);
    assert!(!m.device().unwrap().route.is_empty());
}

#[test]
#[ignore]
fn hw_ms1_and_stream_state() {
    use makcu_core::Button;
    let t = open();
    let m = Mouse::new(&t);
    assert_eq!(
        m.button_state(Button::MS1).unwrap(),
        makcu_core::ButtonOwnership::None
    );
    assert!(!m.button_stream_state().unwrap());
    m.start_button_stream(1).unwrap();
    assert!(m.button_stream_state().unwrap());
    m.stop_button_stream().unwrap();
    assert!(!m.button_stream_state().unwrap());
}

#[test]
#[ignore]
fn hw_smooth_netzero() {
    use std::time::Duration;
    let t = open();
    let m = Mouse::new(&t);
    m.move_smooth(8, 4, 4, Duration::ZERO).unwrap();
    m.move_rel(-8, -4).unwrap();
    assert!(m.wait_idle(Duration::from_secs(2)).unwrap());
}

#[test]
#[ignore]
fn hw_setter_error_window_fast() {
    // A rejected setter must surface within the ~80 ms ERR window,
    // not the 800 ms query timeout (audit point: read_until clamps
    // the port timeout per call).
    let t = open();
    let t0 = Instant::now();
    let r = t.set("km.bogus_cmd_xyz", &[]);
    let dt = t0.elapsed();
    assert!(r.is_err(), "bogus setter should be rejected");
    assert!(
        dt < Duration::from_millis(500),
        "ERR window exceeded: {dt:?}"
    );
}

#[test]
#[ignore]
fn hw_batch_netzero() {
    let t = open();
    let m = Mouse::new(&t);
    let mut b = m.batch();
    b.move_rel(2, 0).unwrap();
    b.move_rel(-2, 0).unwrap();
    assert_eq!(b.execute().unwrap(), 2);
    assert!(m.getpos().is_ok());
}
