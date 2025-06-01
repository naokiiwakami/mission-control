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
}

#[derive(Debug)]
pub struct Request {
    pub client_id: u32,
    pub command: String,
    pub operation: Operation,
    pub params: Vec<RequestParam>,
}

#[derive(Debug)]
pub struct Response {
    pub client_id: u32,
    pub reply: Option<String>,
}

pub type OperationResult = Result<Response, ModuleManagementError>;
