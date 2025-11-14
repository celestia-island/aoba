/// Spawn a task that returns anyhow::Result and add it to the global task set
pub fn spawn_task<F, T>(future: F) -> tokio::task::JoinHandle<anyhow::Result<T>>
where
    F: std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn(future)
}

/// Spawn a blocking task that returns anyhow::Result and add it to the global task set
pub fn spawn_blocking_task<F, T>(func: F) -> tokio::task::JoinHandle<anyhow::Result<T>>
where
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(func)
}
