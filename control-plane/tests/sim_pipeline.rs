use control_plane::{rules::RuleOutcome, store::TelemetryStore, subscriber::process_frame};
use protocol::frame::{Payload, TelemetryFrame};
use protocol::{encode_frame, ReplayGuard};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

fn sample_frame(seq: u32, value: bool) -> TelemetryFrame {
    TelemetryFrame {
        seq,
        timestamp_ms: 1,
        node_id: 1,
        payload: Payload::BoolCmd(value),
    }
}

#[test]
fn replay_guard_rejects_duplicate_seq() {
    let mut guard = ReplayGuard::new();
    assert!(guard.accept(1, 1));
    assert!(guard.accept(1, 2));
    assert!(!guard.accept(1, 2));
}

#[test]
fn sim_pipeline_replay_protection_logic() {
    let tmp = TempDir::new().unwrap();
    let store = TelemetryStore::open(tmp.path().join("telemetry.db")).unwrap();
    let mut replay = ReplayGuard::new();
    let mut actions = 0u32;

    for frame in [
        sample_frame(1, true),
        sample_frame(2, true),
        sample_frame(2, true),
    ] {
        if !replay.accept(frame.node_id, frame.seq) {
            continue;
        }
        if control_plane::rules::evaluate(&frame) == RuleOutcome::ActionTriggered {
            store.insert(&frame).unwrap();
            actions += 1;
        }
    }

    assert_eq!(actions, 2);
    assert!(store.contains_action(1, 1));
    assert!(store.contains_action(1, 2));
}

#[test]
fn sim_pipeline_zmq_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(TelemetryStore::open(tmp.path().join("telemetry.db")).unwrap());
    let replay = Arc::new(Mutex::new(ReplayGuard::new()));

    let endpoint = "inproc://sim-pipeline";
    let replay_clone = replay.clone();
    let store_clone = store.clone();
    let handle = std::thread::spawn(move || {
        control_plane::subscriber::run_subscriber_blocking(
            endpoint.to_string(),
            replay_clone,
            store_clone,
            Some(3),
        )
        .expect("subscriber");
    });

    std::thread::sleep(Duration::from_millis(100));

    let ctx = zmq::Context::new();
    let sock = ctx.socket(zmq::PUB).unwrap();
    sock.bind(endpoint).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    let f1 = encode_frame(&sample_frame(1, true)).unwrap();
    let f2 = encode_frame(&sample_frame(2, true)).unwrap();
    sock.send(&f1, 0).unwrap();
    sock.send(&f2, 0).unwrap();
    sock.send(&f2, 0).unwrap();

    std::thread::sleep(Duration::from_millis(300));
    assert!(store.contains_action(1, 1));
    assert!(store.contains_action(1, 2));
    handle.join().expect("subscriber thread");
}

#[test]
fn process_frame_unit() {
    let tmp = TempDir::new().unwrap();
    let store = TelemetryStore::open(tmp.path().join("telemetry.db")).unwrap();
    let mut replay = ReplayGuard::new();
    let frame = sample_frame(9, true);
    process_frame(frame, &mut replay, &store).unwrap();
    assert!(store.contains_action(1, 9));
}
