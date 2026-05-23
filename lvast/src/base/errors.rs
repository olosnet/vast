use std::{fmt::Display, result};

#[derive(Debug, Clone, PartialEq)]
/// High-level category for a [`VastError`].
pub enum VastErrorType {
    /// Error returned by a native camera SDK or driver implementation.
    CameraDriverError,
    /// Error caused by invalid camera state or camera-level operation failure.
    CameraError,
    /// Error while reading or writing files.
    FileError,
    /// Generic connection or worker-channel error.
    ConnectionError,
    /// Invalid input or configuration.
    InvalidInput,
    /// Connection was refused serial device.
    SerialConnectionRefused,
    /// Generic serial error.
    SerialGenericError,
    /// Serial write error.
    SerialWriteError,
    /// Serial read error.
    SerialReadError,
    /// Connection was refused by a TCP endpoint.
    TcpConnectionRefused,
    /// Generic TCP connection error.
    TcpGenericError,
    /// TCP write error.
    TcpWriteError,
    /// TCP read error.
    TcpReadError,
}

#[derive(Debug, Clone, PartialEq)]
/// Library error containing a category and human-readable message.
pub struct VastError {
    /// Broad error category.
    pub error_type: VastErrorType,
    /// Error details suitable for logging or display.
    pub message: String,
}

impl VastError {
    pub fn new(error_type: VastErrorType, message: String) -> Self {
        Self {
            error_type,
            message,
        }
    }
}

impl Display for VastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VastError: {} ({:?})", self.message, self.error_type)
    }
}

pub type VastResult<T> = result::Result<T, VastError>;
