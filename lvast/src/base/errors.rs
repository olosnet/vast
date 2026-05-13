use std::fmt::Display;

#[derive(Debug, Clone, PartialEq)]
pub enum VastErrorType {
    CameraDriverError,
    CameraError,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VastError {
    pub error_type: VastErrorType,
    pub message: String,
}

impl Display for VastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VastError: {} ({:?})", self.message, self.error_type)
    }
}
