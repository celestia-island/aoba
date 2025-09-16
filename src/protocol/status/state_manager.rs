use anyhow::{anyhow, Result};
use flume::{Receiver, Sender};
use std::sync::{Arc, RwLock};

use crate::protocol::status::types::Status;

/// Message type for the state writer thread
/// Contains a closure that can modify the Status
pub struct StateWriteMessage {
    closure: Box<dyn FnOnce(&mut Status) -> Result<()> + Send>,
    result_sender: Option<flume::Sender<Result<()>>>,
}

impl StateWriteMessage {
    /// Create a new message with a closure that doesn't need a result
    pub fn new<F>(f: F) -> Self
    where
        F: FnOnce(&mut Status) -> Result<()> + Send + 'static,
    {
        Self {
            closure: Box::new(f),
            result_sender: None,
        }
    }

    /// Create a new message with a closure that needs to return a result
    pub fn with_result<F>(f: F) -> (Self, flume::Receiver<Result<()>>)
    where
        F: FnOnce(&mut Status) -> Result<()> + Send + 'static,
    {
        let (tx, rx) = flume::bounded(1);
        let message = Self {
            closure: Box::new(f),
            result_sender: Some(tx),
        };
        (message, rx)
    }

    /// Execute the closure on the provided status
    pub fn execute(self, status: &mut Status) {
        let result = (self.closure)(status);
        if let Some(sender) = self.result_sender {
            let _ = sender.send(result);
        }
    }
}

/// State manager that handles reads and queues writes through a message system
#[derive(Clone)]
pub struct StateManager {
    /// Shared state for reading
    state: Arc<RwLock<Status>>,
    /// Sender for write operations
    write_sender: Sender<StateWriteMessage>,
}

impl StateManager {
    /// Create a new StateManager with initial state
    pub fn new(initial_state: Status) -> (Self, Receiver<StateWriteMessage>) {
        let (tx, rx) = flume::unbounded();
        let manager = Self {
            state: Arc::new(RwLock::new(initial_state)),
            write_sender: tx,
        };
        (manager, rx)
    }

    /// Get a reference to the underlying Arc<RwLock<Status>> for compatibility
    pub fn get_state_ref(&self) -> &Arc<RwLock<Status>> {
        &self.state
    }

    /// Read from the state using a closure (non-blocking read)
    pub fn read_status<R, F>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Status) -> Result<R>,
        R: Clone,
    {
        let guard = self
            .state
            .read()
            .map_err(|err| anyhow!("status lock poisoned: {}", err))?;
        let val = f(&guard)?;
        Ok(val.clone())
    }

    /// Queue a write operation (non-blocking)
    pub fn write_status_async<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Status) -> Result<()> + Send + 'static,
    {
        let message = StateWriteMessage::new(f);
        self.write_sender
            .send(message)
            .map_err(|_| anyhow!("state writer thread disconnected"))?;
        Ok(())
    }

    /// Queue a write operation and wait for completion (blocking)
    pub fn write_status_sync<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Status) -> Result<()> + Send + 'static,
    {
        let (message, result_rx) = StateWriteMessage::with_result(f);
        self.write_sender
            .send(message)
            .map_err(|_| anyhow!("state writer thread disconnected"))?;
        
        result_rx
            .recv()
            .map_err(|_| anyhow!("failed to receive write result"))?
    }

    /// Legacy compatibility function that maintains the original write_status interface
    pub fn write_status<R, F>(&self, mut f: F) -> Result<R>
    where
        F: FnMut(&mut Status) -> Result<R> + Send + 'static,
        R: Clone + Send + 'static,
    {
        let (tx, rx) = flume::bounded(1);
        
        let message = StateWriteMessage {
            closure: Box::new(move |status| {
                let result = f(status)?;
                let _ = tx.send(result);
                Ok(())
            }),
            result_sender: None,
        };

        self.write_sender
            .send(message)
            .map_err(|_| anyhow!("state writer thread disconnected"))?;

        rx.recv()
            .map_err(|_| anyhow!("failed to receive write result"))
    }
}

/// State writer thread function that processes write messages
pub fn run_state_writer_thread(
    mut status: Status,
    message_receiver: Receiver<StateWriteMessage>,
    state_ref: Arc<RwLock<Status>>,
) -> Result<()> {
    log::info!("[STATE_WRITER] State writer thread starting");

    while let Ok(message) = message_receiver.recv() {
        // Execute the message on our local copy
        message.execute(&mut status);

        // Update the shared state (this should be fast since it's just a copy)
        {
            let mut guard = state_ref
                .write()
                .map_err(|err| anyhow!("status lock poisoned in writer thread: {}", err))?;
            *guard = status.clone();
        }
    }

    log::info!("[STATE_WRITER] State writer thread shutting down");
    Ok(())
}