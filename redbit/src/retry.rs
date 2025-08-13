use core::future::Future;

pub async fn retry_with_delay<F, Fut, T, E>(
    attempts: usize,
    delay: std::time::Duration,
    mut op: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    assert!(attempts >= 1);
    let mut left = attempts;
    loop {
        match op().await {
            Ok(v) => return Ok(v),
            Err(_e) if left > 1 => {
                left -= 1;
                tokio::time::sleep(delay).await;
            }
            Err(e) => return Err(e),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::task::JoinHandle;
    use tokio::time::{advance};

    // 1) Immediate success: should not retry, no sleeps needed.
    #[tokio::test(start_paused = true)]
    async fn retry_immediate_success() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        CALLS.store(0, Ordering::SeqCst);

        let out: Result<i32, &'static str> = retry_with_delay(5, Duration::from_secs(1), || async {
            CALLS.fetch_add(1, Ordering::SeqCst);
            Ok(42)
        }).await;

        assert_eq!(out.unwrap(), 42);
        assert_eq!(CALLS.load(Ordering::SeqCst), 1, "must not retry on success");
    }

    // 2) Succeeds after 2 failures: exactly 3 calls; requires advancing 2 delays.
    #[tokio::test(start_paused = true)]
    async fn retry_succeeds_after_two_failures() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        CALLS.store(0, Ordering::SeqCst);
        let delay = Duration::from_millis(500);

        let task = tokio::spawn(async move {
            retry_with_delay(3, delay, || async {
                let n = CALLS.fetch_add(1, Ordering::SeqCst) + 1;
                if n < 3 { Err("not yet") } else { Ok(7) }
            }).await
        });

        // Two failures → two sleeps
        advance(delay).await;
        advance(delay).await;

        let res = task.await.unwrap();
        assert_eq!(res.unwrap(), 7);
        assert_eq!(CALLS.load(Ordering::SeqCst), 3, "must stop right after success");
    }

    // 3) All attempts fail: returns the *last* error; sleeps happen attempts-1 times.
    #[tokio::test(start_paused = true)]
    async fn retry_all_fail_propagates_last_error() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        CALLS.store(0, Ordering::SeqCst);
        let delay = Duration::from_secs(1);

        let task: JoinHandle<Result<(), &'static str>> = tokio::spawn(async move {
            retry_with_delay(3, delay, || async {
                let n = CALLS.fetch_add(1, Ordering::SeqCst) + 1;
                Err(match n {
                    1 => "e1",
                    2 => "e2",
                    _ => "e3", // last error should bubble out
                })
            }).await
        });

        // We expect exactly 2 sleeps (between 3 attempts)
        advance(delay).await;
        advance(delay).await;

        let err = task.await.unwrap().unwrap_err();
        assert_eq!(err, "e3");
        assert_eq!(CALLS.load(Ordering::SeqCst), 3, "exactly N attempts on failure");
    }

    // 4) attempts == 0 should panic due to the assert! guard.
    #[tokio::test(start_paused = true)]
    #[should_panic]
    async fn retry_zero_attempts_panics() {
        let _ = retry_with_delay::<_, _, (), ()>(0, Duration::from_secs(1), || async { Ok(()) }).await;
    }

    // 5) Zero delay: still retries immediately; no time advance required.
    #[tokio::test(start_paused = true)]
    async fn retry_zero_delay_fast() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        CALLS.store(0, Ordering::SeqCst);

        let res: Result<i32, &str> = retry_with_delay(4, Duration::ZERO, || async {
            let n = CALLS.fetch_add(1, Ordering::SeqCst) + 1;
            if n < 4 { Err("nope") } else { Ok(99) }
        }).await;

        assert_eq!(res.unwrap(), 99);
        assert_eq!(CALLS.load(Ordering::SeqCst), 4);
        // No advance() calls necessary; zero-delay sleeps complete immediately in a paused clock.
    }

    // 6) Stops retrying the moment it succeeds (no extra calls after success).
    #[tokio::test(start_paused = true)]
    async fn retry_stops_after_success() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        CALLS.store(0, Ordering::SeqCst);
        let delay = Duration::from_millis(300);

        let task = tokio::spawn(async move {
            retry_with_delay(10, delay, || async {
                let n = CALLS.fetch_add(1, Ordering::SeqCst) + 1;
                if n == 5 { Ok(1) } else { Err("x") }
            }).await
        });

        // 4 failures → 4 sleeps before success on 5th attempt
        for _ in 0..4 { advance(delay).await; }

        let _ = task.await.unwrap().unwrap();
        assert_eq!(CALLS.load(Ordering::SeqCst), 5, "must not overshoot past the first success");
    }
}
