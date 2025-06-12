use std::fmt;
use std::fmt::Write;

#[derive(Debug, Clone)]
pub enum ErrorType {
    A3OpCodeUnknown,
    A3OpCodeMissing,
    A3StreamConflict,
    UserCommandUnknown,
    UserCommandStreamIdMissing,
    UserCommandInvalidRequest,
    RuntimeError,
}

#[derive(Debug, Clone)]
pub struct ModuleManagementError {
    pub error_type: ErrorType,
    pub message: String,
}

impl fmt::Display for ModuleManagementError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}; {}", self.error_type, self.message)
    }
}

impl std::error::Error for ModuleManagementError {}
