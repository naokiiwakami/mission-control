use tokio::sync::oneshot;

use crate::a3_modules::A3Module;
use crate::analog3::Value;
use crate::error::AppError;

#[derive(Debug)]
pub enum Command {
    List {
        resp: oneshot::Sender<Result<Vec<A3Module>, AppError>>,
    },
    Ping {
        id: u8,
        enable_visual: bool,
        resp: oneshot::Sender<Result<(), AppError>>,
    },
    GetName {
        id: u8,
        resp: oneshot::Sender<Result<String, AppError>>,
    },
    AckName,
    GetConfig,
    AckConfig,
    RequestUidCancel,
    Cancel,
    // for testing and debugging
    PretendSignIn,
    PretendNotifyId,
    // internal diag
    Hi {
        resp: oneshot::Sender<String>,
    },
}

#[derive(Debug)]
pub struct Request {
    pub session_id: u32,
    pub operation: Command,
    pub params: Vec<Value>,
}

#[derive(Debug)]
pub struct Response {
    pub reply: Vec<u8>,
    pub more: bool,
    /// non-zero ID would be returned when more is true
    pub stream_id: u8,
}

pub type OperationResult = Result<Response, AppError>;
