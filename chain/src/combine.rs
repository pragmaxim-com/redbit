use futures::future::join;
use std::future::Future;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;

pub async fn futures<A, B, OA, OB>(future_a: A, future_b: B, shutdown_tx: watch::Sender<bool>)
where
    A: Future<Output = OA> + Send + 'static,
    B: Future<Output = OB> + Send + 'static,
{
    let mut sigint = signal(SignalKind::interrupt()).expect("Failed to listen for SIGINT");
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to listen for SIGTERM");

    tokio::select! {
        _ = sigint.recv() => {
            println!("Received SIGINT, shutting down...");
            let _ = shutdown_tx.send_replace(true);
        }
        _ = sigterm.recv() => {
            println!("Received SIGTERM, shutting down...");
            let _ = shutdown_tx.send_replace(true);
        }
        _ = join(future_a, future_b) => {
            // Ensure cooperative shutdown when workers finish first
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ShutdownReason {
    AlreadyTrue,
    ChangedToTrue,
    SenderDropped,
}

pub async fn await_shutdown(mut rx: watch::Receiver<bool>) -> ShutdownReason {
    if *rx.borrow() {
        return ShutdownReason::AlreadyTrue;
    }
    // mark current as seen so the next `changed()` waits for a new version
    let _ = rx.borrow_and_update();

    loop {
        match rx.changed().await {
            Ok(()) => {
                // mark this change as seen and check the value
                if *rx.borrow_and_update() {
                    return ShutdownReason::ChangedToTrue;
                }
                // value changed but stayed false; keep waiting
            }
            Err(_sender_dropped) => {
                return ShutdownReason::SenderDropped;
            }
        }
    }
}


#[tokio::test]
async fn test_shutdown_helper_variants() {
    // AlreadyTrue
    let (tx1, rx1) = watch::channel(false);
    let _ = tx1.send(true);
    let r1 = await_shutdown(rx1.clone()).await;
    assert_eq!(r1, ShutdownReason::AlreadyTrue);

    // ChangedToTrue
    let (tx2, rx2) = watch::channel(false);
    let t2 = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let _ = tx2.send(true);
    });
    let r2 = await_shutdown(rx2.clone()).await;
    let _ = t2.await;
    assert_eq!(r2, ShutdownReason::ChangedToTrue);

    // SenderDropped
    let (tx3, rx3) = watch::channel(false);
    drop(tx3);
    let r3 = await_shutdown(rx3.clone()).await;
    assert_eq!(r3, ShutdownReason::SenderDropped);
}