//! Hardware-in-the-loop tests (require physical ESP32 devices).
//!
//! Run with: HIL_ENABLED=1 cargo test -p control-plane --test hil -- --nocapture

use std::env;
use std::io::Write;
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires HIL hardware and self-hosted runner"]
fn hil_trigger_to_action() {
    if env::var("HIL_ENABLED").ok().as_deref() != Some("1") {
        eprintln!("Skipping HIL test (set HIL_ENABLED=1)");
        return;
    }

    let tx_port = env::var("HIL_TX_PORT").unwrap_or_else(|_| "/dev/ttyUSB_TX".to_string());
    let db_path = env::var("HIL_DB_PATH").unwrap_or_else(|_| "./data/telemetry.db".to_string());

    let mut port = serialport::new(tx_port, 115_200)
        .timeout(Duration::from_millis(100))
        .open()
        .expect("open TX UART");

    port.write_all(b"TRIGGER\n").expect("send TRIGGER");

    let deadline = Instant::now() + Duration::from_millis(500);
    let store = sled::open(db_path).expect("open sled");
    loop {
        if Instant::now() > deadline {
            panic!("HIL timeout waiting for ACTION in sled");
        }
        if store.iter().keys().next().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
