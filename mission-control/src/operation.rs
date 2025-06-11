use crate::analog3::Value;
use crate::module_manager::ModuleManagementError;

#[derive(Debug, Clone)]
pub enum Operation {
    List,
    Ping,
    GetName,
    AckName,
    GetConfig,
    AckConfig,
    RequestUidCancel,
    Cancel,
    // for testing and debugging
    PretendSignIn,
    PretendNotifyId,
}

#[derive(Debug)]
pub struct Request {
    pub client_id: u32,
    pub operation: Operation,
    pub params: Vec<Value>,
}

#[derive(Debug)]
pub struct Response {
    pub reply: Vec<u8>,
    pub more: bool,
    /// non-zero ID would be returned when more is true
    pub stream_id: u8,
}

pub type OperationResult = Result<Response, ModuleManagementError>;
