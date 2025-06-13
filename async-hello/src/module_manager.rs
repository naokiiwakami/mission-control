mod streams;

use std::time::Duration;

use tokio::sync::{mpsc::Sender, oneshot};
use tokio::time::timeout;

use crate::a3_message;
use crate::a3_modules;
use crate::a3_modules::A3Module;
use crate::analog3::{self as a3, ChunkBuilder, Property};
use crate::can_controller::CanMessage;
use crate::error::AppError;
use crate::operation::Command;

type Result<T> = std::result::Result<T, AppError>;

pub struct ModuleManager {
    can_tx: Sender<CanMessage>,
    modules_tx: Sender<a3_modules::Operation>,
    streams_tx: Sender<streams::Operation>,
}

impl ModuleManager {
    pub fn new(can_tx: Sender<CanMessage>, modules_tx: Sender<a3_modules::Operation>) -> Self {
        let (streams_tx, _) = streams::start();
        Self {
            can_tx,
            modules_tx,
            streams_tx,
        }
    }

    // incoming message handling /////////////////////////////////////////////////////////

    pub fn handle_can_message(&mut self, message: CanMessage) {
        log::debug!("Message received: id={:08x}", message.id());
        if message.data_length() == 0 {
            log::debug!("no opcode");
            return;
        }
        let opcode = message.get_data(0);
        if message.is_extended() {
            return match opcode {
                a3::A3_ADMIN_SIGN_IN => self.handle_remote_sign_in(message).unwrap(),
                a3::A3_ADMIN_NOTIFY_ID => self.handle_remote_id_notification(message).unwrap(),
                // a3::A3_ADMIN_REQ_UID_CANCEL => self.handle_uid_cancel_req(message),
                _ => {
                    log::warn!(
                        "Unknown opcode; id={:08x}, opcode={:02x}",
                        message.id(),
                        opcode
                    );
                    return;
                }
            };
        }
        return match opcode {
            a3::A3_IM_REPLY_PING => self.handle_stream_reply("ping", message).unwrap(),
            a3::A3_IM_REPLY_NAME => self.handle_stream_reply("get-name", message).unwrap(),
            a3::A3_IM_REPLY_CONFIG => self.handle_stream_reply("get-config", message).unwrap(),
            _ => {
                log::warn!(
                    "Unknown opcode; id={:08x}, opcode={:02x}",
                    message.id(),
                    opcode
                );
                return;
            }
        };
    }

    fn handle_remote_sign_in(&self, in_message: CanMessage) -> Result<()> {
        let modules_tx = self.modules_tx.clone();
        let can_tx = self.can_tx.clone();
        tokio::spawn(async move {
            let remote_uid = in_message.id();
            let (resp_tx, resp_rx) = oneshot::channel();
            let modules_op = a3_modules::Operation::GetOrCreateIdByUid {
                uid: remote_uid,
                resp: resp_tx,
            };
            modules_tx.send(modules_op).await.unwrap();
            let remote_id = resp_rx.await.unwrap().unwrap();
            a3_message::assign_module_id(can_tx, remote_uid, remote_id).await;
            log::info!(
                "Issued module id {:02x} for uid {:08x}",
                remote_id,
                remote_uid
            );
        });
        return Ok(());
    }

    fn handle_remote_id_notification(&mut self, in_message: CanMessage) -> Result<()> {
        let modules_tx = self.modules_tx.clone();
        tokio::spawn(async move {
            let uid = in_message.id();
            let id = in_message.get_data(1);
            log::debug!("Module recognized; id {id:02x} for uid {uid:08x}");
            modules_tx
                .send(a3_modules::Operation::Register { uid, id })
                .await
                .unwrap();
        });
        return Ok(());
    }

    fn handle_stream_reply(&mut self, op_name_src: &str, in_message: CanMessage) -> Result<()> {
        let streams_tx = self.streams_tx.clone();
        let op_name = String::from(op_name_src);
        tokio::spawn(async move {
            let remote_id = in_message.id();
            let stream_id = (remote_id - a3::A3_ID_INDIVIDUAL_MODULE_BASE) as u8;
            log::debug!("{} reply received; id {:02x}", op_name, remote_id);
            let (get_resp_tx, get_resp_rx) = oneshot::channel();
            streams_tx
                .send(streams::Operation::Get {
                    remote_id: stream_id,
                    op_resp: get_resp_tx,
                })
                .await
                .unwrap();
            match get_resp_rx.await.unwrap() {
                Ok(stream_resp_tx) => stream_resp_tx.send(in_message).unwrap(),
                Err(e) => {
                    log::error!(
                        "An error encountered while finding stream for {}: {:?}",
                        op_name,
                        e
                    );
                }
            }
        });
        return Ok(());
    }

    // Command handling ///////////////////////////////////////////////////////////////

    pub fn handle_command(&mut self, command: Command) {
        match command {
            Command::Hi { resp } => self.hi(resp),
            Command::List { resp } => self.list(resp),
            Command::Ping {
                id,
                enable_visual,
                resp,
            } => self.ping(id, enable_visual, resp),
            Command::GetName { id, resp } => self.get_name(id, resp),
            Command::GetConfig { id, resp } => self.get_config(id, resp),
            _ => {
                log::error!("Operation not implemented: {:?}", command);
            }
        }
    }

    fn hi(&mut self, resp: oneshot::Sender<Result<String>>) {
        tokio::spawn(async {
            resp.send(Ok("hello".to_string())).unwrap();
        });
    }

    fn list(&mut self, resp: oneshot::Sender<Result<Vec<A3Module>>>) {
        let modules_tx = self.modules_tx.clone();
        tokio::spawn(async move {
            let (tx, rx) = oneshot::channel();
            modules_tx
                .send(a3_modules::Operation::List { resp: tx })
                .await
                .unwrap();
            match rx.await.unwrap() {
                Ok(list) => {
                    resp.send(Ok(list)).unwrap();
                }
                Err(e) => {
                    log::error!("An error encountered while listing modules: {:?}", e);
                }
            }
        });
    }

    fn ping(&mut self, id: u8, enable_visual: bool, resp: oneshot::Sender<Result<()>>) {
        let streams_tx = self.streams_tx.clone();
        let can_tx = self.can_tx.clone();
        tokio::spawn(async move {
            let result = ping_core(streams_tx.clone(), can_tx, id, enable_visual).await;
            if let Err(e) = resp.send(result) {
                log::error!("Error in sending back the ping result: {:?}", e);
            }

            terminate_stream(streams_tx, id).await;
        });
    }

    fn get_name(&mut self, id: u8, resp: oneshot::Sender<Result<String>>) {
        let streams_tx = self.streams_tx.clone();
        let can_tx = self.can_tx.clone();
        tokio::spawn(async move {
            let result = get_name_core(streams_tx.clone(), can_tx, id).await;
            if let Err(e) = resp.send(result) {
                log::error!("Error in sending back the get-name result: {:?}", e);
            }

            terminate_stream(streams_tx, id).await;
        });
    }

    fn get_config(&mut self, id: u8, resp: oneshot::Sender<Result<Vec<Property>>>) {
        let streams_tx = self.streams_tx.clone();
        let can_tx = self.can_tx.clone();
        tokio::spawn(async move {
            let result = get_config_core(streams_tx.clone(), can_tx, id).await;
            if let Err(e) = resp.send(result) {
                log::error!("Error in sending back the get-name result: {:?}", e);
            }

            terminate_stream(streams_tx, id).await;
        });
    }
}

async fn ping_core(
    streams_tx: Sender<streams::Operation>,
    can_tx: Sender<CanMessage>,
    id: u8,
    enable_visual: bool,
) -> Result<()> {
    // start a stream
    let stream_resp_rx = start_stream(streams_tx.clone(), id).await?;

    // ping
    a3_message::ping(can_tx, id, enable_visual).await;

    // wait for the response
    return match timeout(Duration::from_secs(10), stream_resp_rx).await {
        Ok(_) => Ok(()),
        Err(_) => Err(AppError::timeout()),
    };
}

async fn get_name_core(
    streams_tx: Sender<streams::Operation>,
    can_tx: Sender<CanMessage>,
    id: u8,
) -> Result<String> {
    // start a stream
    let mut stream_resp_rx = Some(start_stream(streams_tx.clone(), id).await?);

    // send request message
    a3_message::request_name(can_tx.clone(), id).await;

    // control the stream
    let mut chunk_builder = ChunkBuilder::for_single_field();
    loop {
        let Ok(result) = timeout(Duration::from_secs(10), stream_resp_rx.take().unwrap()).await
        else {
            return Err(AppError::timeout());
        };
        let message = result.unwrap();
        let data = &message.data();
        let size = message.data_length() as usize;
        if size < 2 {
            return Err(AppError::runtime("zero-length data received"));
        }
        match chunk_builder.data(&data.as_slice()[1..size], size - 1) {
            Ok(is_done) => {
                if is_done {
                    let properties = chunk_builder.build().unwrap();
                    let name = properties[0].get_value_as_string().unwrap();
                    return Ok(name);
                }
                stream_resp_rx.replace(continue_stream(streams_tx.clone(), id).await?);
                a3_message::continue_name(can_tx.clone(), id).await;
            }
            Err(e) => {
                let message = format!("GetName: Data parsing failed: {:?}", e);
                return Err(AppError::runtime(message.as_str()));
            }
        }
    }
}

async fn get_config_core(
    streams_tx: Sender<streams::Operation>,
    can_tx: Sender<CanMessage>,
    id: u8,
) -> Result<Vec<Property>> {
    // start a stream
    let mut stream_resp_rx = Some(start_stream(streams_tx.clone(), id).await?);

    // send request message
    a3_message::request_config(can_tx.clone(), id).await;

    // control the stream
    let mut chunk_builder = ChunkBuilder::new();
    loop {
        let Ok(result) = timeout(Duration::from_secs(10), stream_resp_rx.take().unwrap()).await
        else {
            return Err(AppError::timeout());
        };
        let message = result.unwrap();
        let data = &message.data();
        let size = message.data_length() as usize;
        if size < 2 {
            return Err(AppError::runtime("zero-length data received"));
        }
        match chunk_builder.data(&data.as_slice()[1..size], size - 1) {
            Ok(is_done) => {
                if is_done {
                    let properties = chunk_builder.build().unwrap();
                    return Ok(properties);
                }
                stream_resp_rx.replace(continue_stream(streams_tx.clone(), id).await?);
                a3_message::continue_config(can_tx.clone(), id).await;
            }
            Err(e) => {
                let message = format!("GetName: Data parsing failed: {:?}", e);
                return Err(AppError::runtime(message.as_str()));
            }
        }
    }
}

async fn start_stream(
    streams_tx: Sender<streams::Operation>,
    remote_id: u8,
) -> Result<oneshot::Receiver<CanMessage>> {
    return start_or_continue_stream(streams_tx, remote_id, true).await;
}

async fn continue_stream(
    streams_tx: Sender<streams::Operation>,
    remote_id: u8,
) -> Result<oneshot::Receiver<CanMessage>> {
    return start_or_continue_stream(streams_tx, remote_id, false).await;
}

async fn start_or_continue_stream(
    streams_tx: Sender<streams::Operation>,
    remote_id: u8,
    is_start: bool,
) -> Result<oneshot::Receiver<CanMessage>> {
    let (start_resp_tx, start_resp_rx) = oneshot::channel();
    let (stream_resp_tx, stream_resp_rx) = oneshot::channel();
    let operation = if is_start {
        streams::Operation::Start {
            remote_id,
            op_resp: start_resp_tx,
            stream_resp: stream_resp_tx,
        }
    } else {
        streams::Operation::Continue {
            remote_id,
            op_resp: start_resp_tx,
            stream_resp: stream_resp_tx,
        }
    };
    streams_tx.send(operation).await.unwrap();
    if let Err(e) = start_resp_rx.await.unwrap() {
        let error = match e.error_type {
            streams::ErrorType::Busy => AppError {
                error_type: crate::error::ErrorType::A3StreamConflict,
                message: "busy".to_string(),
            },
            _ => AppError {
                error_type: crate::error::ErrorType::RuntimeError,
                message: format!("{:?}", e),
            },
        };
        return Err(error);
    }
    return Ok(stream_resp_rx);
}

async fn terminate_stream(streams_tx: Sender<streams::Operation>, id: u8) {
    let (term_resp_tx, term_resp_rx) = oneshot::channel();
    streams_tx
        .send(streams::Operation::Terminate {
            remote_id: id,
            op_resp: term_resp_tx,
        })
        .await
        .unwrap();
    term_resp_rx.await.unwrap().unwrap();
}
