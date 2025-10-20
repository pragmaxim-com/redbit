use crossbeam::channel::{after, select, unbounded, Receiver, Sender};
use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, Instant};

enum Cmd<M> {
    ScheduleAt { at: Instant, msg: M },
}

#[derive(Clone)]
pub struct Delayed<M: Send + 'static> {
    tx: Sender<Cmd<M>>,
}

impl<M: Send + 'static> Delayed<M> {
    pub fn start(outbox: Sender<M>) -> Self {
        let (tx, rx): (Sender<Cmd<M>>, Receiver<Cmd<M>>) = unbounded();
        thread::spawn(move || {
            let mut by_time: BTreeMap<Instant, Vec<M>> = BTreeMap::new();

            loop {
                // drain immediate commands
                while let Ok(Cmd::ScheduleAt { at, msg }) = rx.try_recv() {
                    by_time.entry(at).or_default().push(msg);
                }

                // if empty, block for one more cmd or exit
                if by_time.is_empty() {
                    match rx.recv() {
                        Ok(Cmd::ScheduleAt { at, msg }) => {
                            by_time.entry(at).or_default().push(msg);
                            continue;
                        }
                        Err(_) => break, // no more senders
                    }
                }

                // deliver all overdue
                let now = Instant::now();
                while let Some((&at, _)) = by_time.first_key_value() {
                    if at > now { break; }
                    if let Some(mut v) = by_time.remove(&at) {
                        for m in v.drain(..) {
                            if outbox.send(m).is_err() { return; }
                        }
                    }
                }

                // wait until next deadline or new command
                if let Some((&next_at, _)) = by_time.first_key_value() {
                    let timer = after(next_at.saturating_duration_since(Instant::now()));
                    select! {
                        recv(rx) -> cmd => {
                            if let Ok(Cmd::ScheduleAt { at, msg }) = cmd {
                                by_time.entry(at).or_default().push(msg);
                            }
                            // if Err, channel closed; keep ticking timers until queue empties
                        }
                        recv(timer) -> _ => { /* deliver on next loop */ }
                    }
                }
            }
        });
        Self { tx }
    }

    #[inline]
    pub fn schedule_in(&self, delay: Duration, msg: M) {
        let _ = self.tx.send(Cmd::ScheduleAt { at: Instant::now() + delay, msg });
    }

    #[inline]
    pub fn schedule_at(&self, at: Instant, msg: M) {
        let _ = self.tx.send(Cmd::ScheduleAt { at, msg });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam::channel::unbounded;

    #[test]
    fn fires_once_after_delay() {
        let (tx, rx) = unbounded::<u32>();
        let d = Delayed::start(tx);
        d.schedule_in(Duration::from_millis(2), 7);

        let got = rx.recv_timeout(Duration::from_millis(50)).unwrap();
        assert_eq!(got, 7);
        assert!(rx.recv_timeout(Duration::from_millis(5)).is_err());
    }

    #[test]
    fn multiple_items_respect_order() {
        let (tx, rx) = unbounded::<&'static str>();
        let d = Delayed::start(tx);

        d.schedule_in(Duration::from_millis(5), "third");
        d.schedule_in(Duration::from_millis(1), "first");
        d.schedule_in(Duration::from_millis(3), "second");

        let a = rx.recv_timeout(Duration::from_millis(80)).unwrap();
        let b = rx.recv_timeout(Duration::from_millis(80)).unwrap();
        let c = rx.recv_timeout(Duration::from_millis(80)).unwrap();
        assert_eq!((a, b, c), ("first", "second", "third"));
        assert!(rx.recv_timeout(Duration::from_millis(5)).is_err());
    }

    #[test]
    fn batches_same_instant() {
        let (tx, rx) = unbounded::<u8>();
        let d = Delayed::start(tx);

        d.schedule_in(Duration::from_millis(2), 1);
        d.schedule_in(Duration::from_millis(2), 2);
        d.schedule_in(Duration::from_millis(2), 3);

        let mut v = vec![];
        v.push(rx.recv_timeout(Duration::from_millis(60)).unwrap());
        v.push(rx.recv_timeout(Duration::from_millis(60)).unwrap());
        v.push(rx.recv_timeout(Duration::from_millis(60)).unwrap());
        assert_eq!(v, vec![1,2,3]);
    }

    #[test]
    fn drains_overdue_buckets() {
        // Intentionally use a near-zero delay and then sleep the test thread a bit
        // to ensure multiple buckets are overdue; the worker should deliver them all.
        let (tx, rx) = unbounded::<u8>();
        let d = Delayed::start(tx);

        d.schedule_in(Duration::from_millis(1), 1);
        d.schedule_in(Duration::from_millis(2), 2);
        d.schedule_in(Duration::from_millis(3), 3);

        // Allow all to become overdue before we start receiving
        std::thread::sleep(Duration::from_millis(10));

        let a = rx.recv_timeout(Duration::from_millis(50)).unwrap();
        let b = rx.recv_timeout(Duration::from_millis(50)).unwrap();
        let c = rx.recv_timeout(Duration::from_millis(50)).unwrap();
        assert_eq!((a, b, c), (1, 2, 3));
    }

    #[test]
    fn exits_when_outbox_closed() {
        let (tx, rx) = unbounded::<u8>();
        let d = Delayed::start(tx);
        d.schedule_in(Duration::from_millis(2), 9);
        drop(rx); // outbox closed => worker exits on first failed send
        // sanity: no panic/deadlock expected
    }

    #[test]
    fn drains_then_exits_when_cmd_closed() {
        let (tx, rx) = unbounded::<u8>();
        let d = Delayed::start(tx);
        d.schedule_in(Duration::from_millis(2), 5);
        drop(d); // close command channel

        let got = rx.recv_timeout(Duration::from_millis(80)).unwrap();
        assert_eq!(got, 5);
    }
}
