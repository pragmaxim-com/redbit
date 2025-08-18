use core::future::Future;
use tokio::task::JoinHandle;

pub fn spawn_named<F>(name: &'static str, fut: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    spawn_named_impl(name, fut)
}

#[cfg(feature = "tracing")]
fn spawn_named_impl<F>(name: &'static str, fut: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::task::Builder::new()
        .name(name)
        .spawn(fut)
        .expect("spawn")
}

#[cfg(not(feature = "tracing"))]
fn spawn_named_impl<F>(_name: &'static str, fut: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(fut)
}

pub fn spawn_blocking_named<F, R>(name: &'static str, f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    spawn_blocking_named_impl(name, f)
}

#[cfg(feature = "tracing")]
fn spawn_blocking_named_impl<F, R>(name: &'static str, f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tokio::task::Builder::new()
        .name(name)
        .spawn_blocking(f)
        .expect("spawn_blocking")
}

#[cfg(not(feature = "tracing"))]
fn spawn_blocking_named_impl<F, R>(_name: &'static str, f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(f)
}
