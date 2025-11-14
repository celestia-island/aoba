/// Spawn a task and add it to the global task set
pub fn spawn_task<F>(future: F) -> tokio::task::JoinHandle<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    tokio::task::spawn(future)
}

/// Spawn a task that returns a Result and add it to the global task set
pub fn spawn_result_task<F, T, E>(future: F) -> tokio::task::JoinHandle<Result<T, E>>
where
    F: std::future::Future<Output = Result<T, E>> + Send + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    tokio::task::spawn(future)
}

/// Spawn a blocking task and add it to the global task set
pub fn spawn_blocking_task<F, R>(func: F) -> tokio::task::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(func)
}
