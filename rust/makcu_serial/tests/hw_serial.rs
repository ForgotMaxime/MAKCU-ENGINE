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
