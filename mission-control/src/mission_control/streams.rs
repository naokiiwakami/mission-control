use std::{collections::HashMap, fmt};

use tokio::{
    sync::{
        mpsc::{Receiver, Sender, channel},
        oneshot,
    },
    task::JoinHandle,
};

use crate::analog3 as a3;
use crate::can_controller::CanMessage;

type Result<T> = std::result::Result<T, StreamError>;

pub enum Operation {
    Start {
        stream_id: u32,
        op_resp: oneshot::Sender<Result<()>>,
        stream_resp: oneshot::Sender<CanMessage>,
    },
    CreateWire {
        op_resp: oneshot::Sender<Result<u32>>,
        stream_resp: oneshot::Sender<CanMessage>,
    },
    Get {
        stream_id: u32,
        op_resp: oneshot::Sender<Result<oneshot::Sender<CanMessage>>>,
    },
    Continue {
        stream_id: u32,
        op_resp: oneshot::Sender<Result<()>>,
        stream_resp: oneshot::Sender<CanMessage>,
    },
    Terminate {
        stream_id: u32,
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
        let mut manager = StreamManager::new();
        manager.handle_requests(operation_rx).await;
    });
    return (operation_tx, handle);
}

struct StreamManager {
    streams: HashMap<u32, Stream>,
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
        }
    }

    pub async fn handle_requests(&mut self, mut operation_rx: Receiver<Operation>) {
        loop {
            if let Some(request) = operation_rx.recv().await {
                match request {
                    Operation::Start {
                        stream_id,
                        op_resp,
                        stream_resp,
                    } => {
                        let response = self.start_stream(stream_id, stream_resp);
                        op_resp.send(response).unwrap();
                    }
                    Operation::CreateWire {
                        op_resp,
                        stream_resp,
                    } => {
                        let response = match self.find_available_wire() {
                            Some(wire_id) => match self.start_stream(wire_id, stream_resp) {
                                Ok(()) => Ok(wire_id),
                                Err(e) => Err(e),
                            },
                            None => Err(StreamError::new(ErrorType::Busy)),
                        };
                        op_resp.send(response).unwrap();
                    }
                    Operation::Get { stream_id, op_resp } => {
                        let response = match self.streams.get_mut(&stream_id) {
                            Some(stream) => match stream.stream_resp.take() {
                                Some(stream_resp) => Ok(stream_resp),
                                None => Err(StreamError::new(ErrorType::Stale)),
                            },
                            None => Err(StreamError::new(ErrorType::NoSuchStream)),
                        };
                        op_resp.send(response).unwrap();
                    }
                    Operation::Continue {
                        stream_id,
                        op_resp,
                        stream_resp,
                    } => {
                        let response = match self.streams.get_mut(&stream_id) {
                            Some(stream) => {
                                stream.stream_resp.replace(stream_resp);
                                Ok(())
                            }
                            None => Err(StreamError::new(ErrorType::NoSuchStream)),
                        };
                        op_resp.send(response).unwrap();
                    }
                    Operation::Terminate { stream_id, op_resp } => {
                        let response = match self.streams.remove(&stream_id) {
                            Some(_) => Ok(()),
                            None => Err(StreamError::new(ErrorType::NoSuchStream)),
                        };
                        op_resp.send(response).unwrap();
                    }
                }
            }
        }
    }

    fn start_stream(
        &mut self,
        stream_id: u32,
        stream_resp: oneshot::Sender<CanMessage>,
    ) -> Result<()> {
        if self.streams.contains_key(&stream_id) {
            Err(StreamError::new(ErrorType::Busy))
        } else {
            self.streams.insert(stream_id, Stream::new(stream_resp));
            Ok(())
        }
    }

    fn find_available_wire(&mut self) -> Option<u32> {
        for id in 0..64 {
            let wire_id = a3::A3_ID_ADMIN_WIRES_BASE + id as u32;
            if !self.streams.contains_key(&wire_id) {
                return Some(wire_id);
            }
        }
        return None;
    }
}
