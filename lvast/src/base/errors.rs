use std::fmt::Display;

pub enum VastErrorType {
    CameraDriverError,
    CameraError,
}

impl Display for VastErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

pub struct VastError {
    pub error_type: VastErrorType,
    pub message: String,
}

impl Display for VastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "VastError: {} ({:?})",
            self.message,
            self.error_type.to_string()
        )
    }
}
