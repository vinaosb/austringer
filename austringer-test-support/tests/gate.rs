//! Self-test suite for [`ResponseGate`] (plan todo 2d): deterministic
//! orchestration of concurrent producer/consumer tasks — block-until-release
//! ordering, cancellation, and bounded waits. Runs under `start_paused` with
//! spawned tasks; zero real sleeps.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use austringer_test_support::gate::{GateWaitError, ResponseGate};

#[tokio::test(start_paused = true)]
async fn gate_wait_blocks_until_producer_releases() {
    let gate = Arc::new(ResponseGate::new());
    let order: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));

    let consumer = {
        let gate = Arc::clone(&gate);
        let order = Arc::clone(&order);
        tokio::spawn(async move {
            gate.wait().await.expect("release() must open the gate");
            order.lock().unwrap().push("consumed");
        })
    };
    let producer = {
        let gate = Arc::clone(&gate);
        let order = Arc::clone(&order);
        tokio::spawn(async move {
            order.lock().unwrap().push("prepared");
            gate.release();
        })
    };

    producer.await.unwrap();
    consumer.await.unwrap();
    assert_eq!(*order.lock().unwrap(), ["prepared", "consumed"]);
    assert!(gate.is_released());
}

#[tokio::test(start_paused = true)]
async fn gate_cancel_fails_current_and_future_waiters() {
    let gate = Arc::new(ResponseGate::new());

    let waiter = {
        let gate = Arc::clone(&gate);
        tokio::spawn(async move { gate.wait().await })
    };
    // Let the waiter park on the gate before cancelling it.
    tokio::task::yield_now().await;
    gate.cancel();

    match waiter.await.unwrap() {
        Err(e) => assert_eq!(e.to_string(), "gate closed before release"),
        Ok(()) => panic!("cancelled gate must not admit parked waiters"),
    }
    // Future waits fail immediately — cancel is sticky.
    assert!(gate.wait().await.is_err());
}

#[tokio::test(start_paused = true)]
async fn gate_wait_for_times_out_deterministically_on_virtual_clock() {
    let gate = ResponseGate::new();
    let start = tokio::time::Instant::now();
    let result = gate.wait_for(Duration::from_secs(5)).await;
    assert!(matches!(result, Err(GateWaitError::Timeout)));
    assert_eq!(start.elapsed(), Duration::from_secs(5));

    // A release still wins when it happens inside the window.
    let gate = ResponseGate::new();
    gate.release();
    assert_eq!(gate.wait_for(Duration::from_secs(5)).await, Ok(()));
}
