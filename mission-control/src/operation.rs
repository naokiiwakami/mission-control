use crate::module_manager::ModuleManagementError;

#[derive(Debug)]
pub enum Operation {
    List,
    Ping,
    RequestUidCancel,
    Cancel,
    // for testing and debugging
    PretendSignIn,
    PretendNotifyId,
}

#[derive(Debug)]
pub enum RequestParam {
    U8(u8),
    U16(u16),
    U32(u32),
    Text(String),
    Bool(bool),
}

#[derive(Debug)]
pub struct Request {
    pub client_id: u32,
    pub operation: Operation,
    pub params: Vec<RequestParam>,
}

#[derive(Debug)]
pub struct Response {
    pub reply: String,
    pub more: bool,
    /// non-zero ID would be returned when more is true
    pub stream_id: u8,
}

pub type OperationResult = Result<Response, ModuleManagementError>;
