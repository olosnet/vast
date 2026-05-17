use std::fmt::Display;

#[derive(Debug, Clone, PartialEq)]
/// High-level category for a [`VastError`].
pub enum VastErrorType {
    /// Error returned by a native camera SDK or driver implementation.
    CameraDriverError,
    /// Error caused by invalid camera state or camera-level operation failure.
    CameraError,
    /// Error while reading or writing files.
    FileError,
}

#[derive(Debug, Clone, PartialEq)]
/// Library error containing a category and human-readable message.
pub struct VastError {
    /// Broad error category.
    pub error_type: VastErrorType,
    /// Error details suitable for logging or display.
    pub message: String,
}

impl Display for VastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VastError: {} ({:?})", self.message, self.error_type)
    }
}
