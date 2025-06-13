use std::{collections::HashMap, fmt};

use tokio::{
    sync::{
        mpsc::{Receiver, Sender, channel},
        oneshot,
    },
    task::JoinHandle,
};

use crate::can_controller::CanMessage;

type Result<T> = std::result::Result<T, StreamError>;

pub enum Operation {
    Start {
        remote_id: u8,
        op_resp: oneshot::Sender<Result<()>>,
        stream_resp: oneshot::Sender<CanMessage>,
    },
    Get {
        remote_id: u8,
        op_resp: oneshot::Sender<Result<oneshot::Sender<CanMessage>>>,
    },
    Continue {
        remote_id: u8,
        op_resp: oneshot::Sender<Result<()>>,
        stream_resp: oneshot::Sender<CanMessage>,
    },
    Terminate {
        remote_id: u8,
        op_resp: oneshot::Sender<Result<()>>,
    },
}

#[derive(Debug, Clone)]
pub enum ErrorType {
    Busy,
    NoSuchStream,
    Stale,
}

#[derive(Debug, Clone)]
pub struct StreamError {
    pub error_type: ErrorType,
}

impl StreamError {
    pub fn new(error_type: ErrorType) -> Self {
        Self { error_type }
    }
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.error_type)
    }
}

impl std::error::Error for StreamError {}

pub struct Stream {
    pub stream_resp: Option<oneshot::Sender<CanMessage>>,
}

impl Stream {
    pub fn new(stream_resp: oneshot::Sender<CanMessage>) -> Self {
        Self {
            stream_resp: Some(stream_resp),
        }
    }
}

pub fn start() -> (Sender<Operation>, JoinHandle<()>) {
    let (operation_tx, operation_rx) = channel(8);
    let handle = tokio::spawn(async move {
        handle_requests(operation_rx).await;
    });
    return (operation_tx, handle);
}

async fn handle_requests(mut operation_rx: Receiver<Operation>) {
    let mut streams = HashMap::<u8, Stream>::new();
    loop {
        if let Some(request) = operation_rx.recv().await {
            match request {
                Operation::Start {
                    remote_id,
                    op_resp,
                    stream_resp,
                } => {
                    let response = if streams.contains_key(&remote_id) {
                        Err(StreamError::new(ErrorType::Busy))
                    } else {
                        streams.insert(remote_id, Stream::new(stream_resp));
                        Ok(())
                    };
                    op_resp.send(response).unwrap();
                }
                Operation::Get { remote_id, op_resp } => {
                    let response = match streams.get_mut(&remote_id) {
                        Some(stream) => match stream.stream_resp.take() {
                            Some(stream_resp) => Ok(stream_resp),
                            None => Err(StreamError::new(ErrorType::Stale)),
                        },
                        None => Err(StreamError::new(ErrorType::NoSuchStream)),
                    };
                    op_resp.send(response).unwrap();
                }
                Operation::Continue {
                    remote_id,
                    op_resp,
                    stream_resp,
                } => {
                    let response = match streams.get_mut(&remote_id) {
                        Some(stream) => {
                            stream.stream_resp.replace(stream_resp);
                            Ok(())
                        }
                        None => Err(StreamError::new(ErrorType::NoSuchStream)),
                    };
                    op_resp.send(response).unwrap();
                }
                Operation::Terminate { remote_id, op_resp } => {
                    let response = match streams.remove(&remote_id) {
                        Some(_) => Ok(()),
                        None => Err(StreamError::new(ErrorType::NoSuchStream)),
                    };
                    op_resp.send(response).unwrap();
                }
            }
        }
    }
}
