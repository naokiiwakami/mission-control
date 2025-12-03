use std::fmt;

#[derive(Debug, Clone)]
pub enum ErrorType {
    A3OpCodeUnknown,
    A3OpCodeMissing,
    A3ModuleNotFound,
    A3SchemaError,
    A3StreamConflict,
    A3InvalidValue,
    UserCommandUnknown,
    UserCommandStreamIdMissing,
    UserCommandInvalidRequest,
    Timeout,
    RuntimeError,
}

#[derive(Debug, Clone)]
pub struct AppError {
    pub error_type: ErrorType,
    pub message: String,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}; {}", self.error_type, self.message)
    }
}

impl std::error::Error for AppError {}

impl AppError {
    pub fn new(error_type: ErrorType, message: String) -> Self {
        Self {
            error_type,
            message,
        }
    }

    pub fn timeout() -> Self {
        Self {
            error_type: ErrorType::Timeout,
            message: "".to_string(),
        }
    }

    pub fn runtime(message: &str) -> Self {
        Self {
            error_type: ErrorType::RuntimeError,
            message: message.to_string(),
        }
    }
}
