use crate::base::connections::Connection;
use crate::base::errors::{VastError, VastErrorType, VastResult};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

fn sleep(duration: Duration) {
    #[cfg(not(test))]
    std::thread::sleep(duration);

    #[cfg(test)]
    let _ = duration;
}

fn connection_worker_error(name: &str, message: impl Into<String>) -> VastError {
    VastError::new(
        VastErrorType::ConnectionError,
        format!("{name} {}", message.into()),
    )
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ReceiveOptions {
    pub delay: Duration,
    pub trim_suffix: Option<char>,
}

enum WorkerRequest {
    Send {
        command: String,
        response: Sender<VastResult<()>>,
    },
    SendReceive {
        command: String,
        options: ReceiveOptions,
        response: Sender<VastResult<String>>,
    },
    Shutdown,
}

fn worker_loop(mut connection: Box<dyn Connection>, requests: Receiver<WorkerRequest>) {
    while let Ok(request) = requests.recv() {
        match request {
            WorkerRequest::Send { command, response } => {
                let _ = response.send(connection.send(&command));
            }
            WorkerRequest::SendReceive {
                command,
                options,
                response,
            } => {
                let result = connection.send(&command).and_then(|_| {
                    sleep(options.delay);
                    let mut received = connection.receive()?;
                    if let Some(trim_suffix) = options.trim_suffix {
                        if received.ends_with(trim_suffix) {
                            received.pop();
                        }
                    }
                    Ok(received)
                });
                let _ = response.send(result);
            }
            WorkerRequest::Shutdown => {
                connection.disconnect();
                break;
            }
        }
    }
}

/// Serialized connection worker for native drivers.
///
/// One worker thread owns underlying [`Connection`]. Callers interact through blocking request /
/// response methods, so transport I/O stays ordered and safe under multithreaded access.
pub struct ConnectionWorker {
    name: &'static str,
    worker_tx: Sender<WorkerRequest>,
    worker_handle: Option<JoinHandle<()>>,
}

impl ConnectionWorker {
    pub fn new(name: &'static str, connection: Box<dyn Connection>) -> Self {
        let (worker_tx, worker_rx) = mpsc::channel();
        let worker_handle = std::thread::spawn(move || worker_loop(connection, worker_rx));

        Self {
            name,
            worker_tx,
            worker_handle: Some(worker_handle),
        }
    }

    pub fn send(&self, command: &str) -> VastResult<()> {
        let (response_tx, response_rx) = mpsc::channel();
        self.worker_tx
            .send(WorkerRequest::Send {
                command: command.to_string(),
                response: response_tx,
            })
            .map_err(|_| connection_worker_error(self.name, "worker thread is not available"))?;

        response_rx
            .recv()
            .map_err(|_| connection_worker_error(self.name, "failed to receive send result"))?
    }

    pub fn send_receive(&self, command: &str) -> VastResult<String> {
        self.send_receive_with_options(command, ReceiveOptions::default())
    }

    pub fn send_receive_with_options(
        &self,
        command: &str,
        options: ReceiveOptions,
    ) -> VastResult<String> {
        let (response_tx, response_rx) = mpsc::channel();
        self.worker_tx
            .send(WorkerRequest::SendReceive {
                command: command.to_string(),
                options,
                response: response_tx,
            })
            .map_err(|_| connection_worker_error(self.name, "worker thread is not available"))?;

        response_rx
            .recv()
            .map_err(|_| connection_worker_error(self.name, "failed to receive response"))?
    }

    pub fn shutdown(&mut self) {
        let _ = self.worker_tx.send(WorkerRequest::Shutdown);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ConnectionWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}
