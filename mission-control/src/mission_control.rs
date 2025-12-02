mod streams;

use std::time::Duration;

use tokio::{
    sync::{mpsc::Sender, oneshot},
    time::timeout,
};

use crate::{
    a3_message,
    a3_modules::{self, A3Module},
    analog3::{
        self as a3,
        config::{ChunkParser, Property, PropertyEncoder},
    },
    can_controller::CanMessage,
    command::Command,
    error::AppError,
};

type Result<T> = std::result::Result<T, AppError>;

pub struct MissionControl {
    can_tx: Sender<CanMessage>,
    modules_tx: Sender<a3_modules::Operation>,
    streams_tx: Sender<streams::Operation>,
}

impl MissionControl {
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
        if message.is_extended() {
            self.handle_extended_message(message);
        } else {
            self.handle_standard_message(message);
        }
    }

    fn handle_extended_message(&mut self, message: CanMessage) {
        if message.data_length() == 0 {
            log::debug!("no opcode");
            return;
        }
        let opcode = message.get_data(0);
        match opcode {
            a3::A3_ADMIN_SIGN_IN => self.handle_remote_sign_in(message).unwrap(),
            a3::A3_ADMIN_NOTIFY_ID => self.handle_remote_id_notification(message).unwrap(),
            a3::A3_ADMIN_REQ_UID_CANCEL => self.handle_uid_cancel_req(message).unwrap(),
            _ => {
                log::warn!(
                    "Unknown opcode; id={:08x}, opcode={:02x}",
                    message.id(),
                    opcode
                );
            }
        };
    }

    fn handle_standard_message(&mut self, message: CanMessage) {
        let remote_id = message.id();
        if remote_id >= a3::A3_ID_INDIVIDUAL_MODULE_BASE {
            if message.data_length() == 0 {
                log::debug!("no opcode");
                return;
            }
            let opcode = message.get_data(0);
            match opcode {
                a3::A3_IM_REPLY_PING => self.handle_stream_reply("ping", message).unwrap(),
                a3::A3_IM_ID_ASSIGN_ACK => self.handle_stream_reply("id-assign", message).unwrap(),
                _ => {
                    log::warn!(
                        "Unknown opcode; id={:08x}, opcode={:02x}",
                        message.id(),
                        opcode
                    );
                }
            };
        } else if remote_id >= a3::A3_ID_ADMIN_WIRES_BASE {
            self.handle_stream_reply("admin-wire", message).unwrap();
        }
        // else ignore
    }

    fn handle_remote_sign_in(&self, in_message: CanMessage) -> Result<()> {
        let modules_tx = self.modules_tx.clone();
        let streams_tx = self.streams_tx.clone();
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
            let stream_id = remote_id as u32 + a3::A3_ID_INDIVIDUAL_MODULE_BASE;
            log::info!(
                "Assigning module id {:02x} for uid {:08x}",
                remote_id,
                remote_uid
            );
            match assign_remote_id(
                streams_tx.clone(),
                can_tx.clone(),
                stream_id,
                remote_id,
                remote_uid,
            )
            .await
            {
                Ok(_) => {
                    log::info!(
                        "ID confirmed module id {:02x} for uid {:08x}",
                        remote_id,
                        remote_uid
                    );
                }
                Err(error) => {
                    log::warn!(
                        "An error encountered in ID assignment; id={:02x}, uid={:08x}, error={:?}",
                        remote_id,
                        remote_uid,
                        error
                    );
                }
            }
        });
        return Ok(());
    }

    fn handle_uid_cancel_req(&mut self, in_message: CanMessage) -> Result<()> {
        let modules_tx = self.modules_tx.clone();
        tokio::spawn(async move {
            let uid = in_message.id();
            let id = in_message.get_data(1);
            log::debug!("Module recognized; id {id:02x} for uid {uid:08x}");
            modules_tx
                .send(a3_modules::Operation::Deregister { uid })
                .await
                .unwrap();
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
            let stream_id = remote_id;
            log::debug!("{} reply received; id {:02x}", op_name, remote_id);
            let (get_resp_tx, get_resp_rx) = oneshot::channel();
            streams_tx
                .send(streams::Operation::Get {
                    stream_id,
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
            Command::SetConfig { id, props, resp } => self.set_config(id, props, resp),
            Command::RequestUidCancel { uid, resp } => self.request_uid_cancel(uid, resp),
            Command::PretendSignIn { uid, resp } => self.pretend_sign_in(uid, resp),
            Command::PretendNotifyId { uid, id, resp } => self.pretend_notify_id(uid, id, resp),
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

            terminate_stream(streams_tx, id as u32 + a3::A3_ID_INDIVIDUAL_MODULE_BASE).await;
        });
    }

    fn get_name(&mut self, id: u8, resp: oneshot::Sender<Result<String>>) {
        let streams_tx = self.streams_tx.clone();
        let can_tx = self.can_tx.clone();
        tokio::spawn(async move {
            match create_wire(streams_tx.clone()).await {
                Ok((wire_addr, stream_resp_rx)) => {
                    let result =
                        get_name_core(streams_tx.clone(), can_tx, id, wire_addr, stream_resp_rx)
                            .await;
                    if let Err(e) = resp.send(result) {
                        log::error!("Error in sending back the get-name result: {:?}", e);
                    }
                    terminate_stream(streams_tx, wire_addr).await;
                }
                Err(e) => resp.send(Err(e)).unwrap(),
            }
        });
    }

    fn get_config(&mut self, id: u8, resp: oneshot::Sender<Result<Vec<Property>>>) {
        let streams_tx = self.streams_tx.clone();
        let can_tx = self.can_tx.clone();
        tokio::spawn(async move {
            match create_wire(streams_tx.clone()).await {
                Ok((wire_addr, stream_resp_rx)) => {
                    let result =
                        get_config_core(streams_tx.clone(), can_tx, id, wire_addr, stream_resp_rx)
                            .await;
                    if let Err(e) = resp.send(result) {
                        log::error!("Error in sending back the get-name result: {:?}", e);
                    }
                    terminate_stream(streams_tx, wire_addr).await;
                }
                Err(e) => resp.send(Err(e)).unwrap(),
            }
        });
    }

    fn set_config(&mut self, id: u8, props: Vec<Property>, resp: oneshot::Sender<Result<()>>) {
        let streams_tx = self.streams_tx.clone();
        let can_tx = self.can_tx.clone();
        tokio::spawn(async move {
            match create_wire(streams_tx.clone()).await {
                Ok((wire_addr, stream_resp_rx)) => {
                    let result = set_config_core(
                        streams_tx.clone(),
                        can_tx,
                        id,
                        props,
                        wire_addr,
                        stream_resp_rx,
                    )
                    .await;
                    if let Err(e) = resp.send(result) {
                        log::error!("Error in sending back the get-name result: {:?}", e);
                    }
                    terminate_stream(streams_tx, wire_addr).await;
                }
                Err(e) => resp.send(Err(e)).unwrap(),
            }
        });
    }

    fn request_uid_cancel(&mut self, uid: u32, resp: oneshot::Sender<Result<()>>) {
        let can_tx = self.can_tx.clone();
        tokio::spawn(async move {
            a3_message::request_uid_cancel(can_tx, uid).await;
            resp.send(Ok(())).unwrap();
        });
    }

    fn pretend_sign_in(&mut self, uid: u32, resp: oneshot::Sender<Result<()>>) {
        let can_tx = self.can_tx.clone();
        tokio::spawn(async move {
            a3_message::im_sign_in(can_tx, uid).await;
            resp.send(Ok(())).unwrap();
        });
    }

    fn pretend_notify_id(&mut self, uid: u32, id: u8, resp: oneshot::Sender<Result<()>>) {
        let can_tx = self.can_tx.clone();
        tokio::spawn(async move {
            a3_message::im_notify_id(can_tx, uid, id).await;
            resp.send(Ok(())).unwrap();
        });
    }
}

async fn ping_core(
    streams_tx: Sender<streams::Operation>,
    can_tx: Sender<CanMessage>,
    id: u8,
    enable_visual: bool,
) -> Result<()> {
    let stream_id = id as u32 + a3::A3_ID_INDIVIDUAL_MODULE_BASE;

    // start a stream
    let stream_resp_rx = start_stream(streams_tx.clone(), stream_id).await?;

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
    wire_addr: u32,
    init_stream_resp_rx: oneshot::Receiver<CanMessage>,
) -> Result<String> {
    let mut stream_resp_rx = Some(init_stream_resp_rx);
    let wire_id = (wire_addr - a3::A3_ID_ADMIN_WIRES_BASE) as u8;

    // send request message
    a3_message::request_name(can_tx.clone(), id, wire_id).await;

    // control the stream
    let mut chunk_parser = ChunkParser::for_single_field();
    loop {
        let Ok(result) = timeout(Duration::from_secs(10), stream_resp_rx.take().unwrap()).await
        else {
            return Err(AppError::timeout());
        };
        let message = result.unwrap();
        let data = &message.data();
        let size = message.data_length() as usize;
        if size < 1 {
            return Err(AppError::runtime("zero-length data received"));
        }
        match chunk_parser.data(&data.as_slice(), size) {
            Ok(is_done) => {
                if is_done {
                    let properties = chunk_parser.commit().unwrap();
                    return match properties[0].get_value_as_string() {
                        Ok(name) => Ok(name),
                        Err(e) => {
                            log::warn!("Data reading error: {:?}", e);
                            Err(AppError::runtime("Detected corrupted data"))
                        }
                    };
                }
                stream_resp_rx.replace(continue_stream(streams_tx.clone(), wire_addr).await?);
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
    wire_addr: u32,
    init_stream_resp_rx: oneshot::Receiver<CanMessage>,
) -> Result<Vec<Property>> {
    let mut stream_resp_rx = Some(init_stream_resp_rx);
    let wire_id = (wire_addr - a3::A3_ID_ADMIN_WIRES_BASE) as u8;

    // send request message
    a3_message::request_config(can_tx.clone(), id, wire_id).await;

    // control the stream
    let mut chunk_parser = ChunkParser::new();
    loop {
        let Ok(result) = timeout(Duration::from_secs(10), stream_resp_rx.take().unwrap()).await
        else {
            return Err(AppError::timeout());
        };
        let message = result.unwrap();
        let data = &message.data();
        let size = message.data_length() as usize;
        if size < 1 {
            return Err(AppError::runtime("zero-length data received"));
        }
        match chunk_parser.data(&data.as_slice(), size) {
            Ok(is_done) => {
                if is_done {
                    let properties = chunk_parser.commit().unwrap();
                    return Ok(properties);
                }
                stream_resp_rx.replace(continue_stream(streams_tx.clone(), wire_addr).await?);
                a3_message::continue_config(can_tx.clone(), id).await;
            }
            Err(e) => {
                let message = format!("GetName: Data parsing failed: {:?}", e);
                return Err(AppError::runtime(message.as_str()));
            }
        }
    }
}

async fn set_config_core(
    streams_tx: Sender<streams::Operation>,
    can_tx: Sender<CanMessage>,
    id: u8,
    props: Vec<Property>,
    wire_addr: u32,
    init_stream_resp_rx: oneshot::Receiver<CanMessage>,
) -> Result<()> {
    let mut stream_resp_rx = Some(init_stream_resp_rx);
    let wire_id = (wire_addr - a3::A3_ID_ADMIN_WIRES_BASE) as u8;

    // initiate modify config stream
    a3_message::modify_config(can_tx.clone(), id, wire_id).await;

    // control the stream
    let mut encoder = PropertyEncoder::new(&props);
    while !encoder.is_done() {
        let Ok(result) = timeout(Duration::from_secs(10), stream_resp_rx.take().unwrap()).await
        else {
            return Err(AppError::timeout());
        };

        let message = result.unwrap();
        if !message.is_remote() {
            log::warn!("arrived frame is not remote, sending data anyway");
        }
        let mut out_message = CanMessage::new();
        out_message.set_id(wire_addr);
        out_message.set_extended(false);
        out_message.set_remote(false);
        let num_flushed_bytes = encoder.flush(out_message.mut_data());
        out_message.set_data_length(num_flushed_bytes as u8);
        if !encoder.is_done() {
            stream_resp_rx.replace(continue_stream(streams_tx.clone(), wire_addr).await?);
        }
        can_tx.send(out_message).await.unwrap();
    }
    return Ok(());
}

async fn assign_remote_id(
    streams_tx: Sender<streams::Operation>,
    can_tx: Sender<CanMessage>,
    stream_id: u32,
    remote_id: u8,
    remote_uid: u32,
) -> Result<()> {
    let mut timeout_interval = Duration::from_millis(50);
    let mut ret: Result<()> = Ok(());
    for _ in 0..10 {
        a3_message::assign_module_id(can_tx.clone(), remote_uid, remote_id).await;
        let stream_resp_rx = start_stream(streams_tx.clone(), stream_id).await?;
        let result = timeout(timeout_interval, stream_resp_rx).await;
        terminate_stream(streams_tx.clone(), stream_id).await;
        match result {
            Ok(_) => {
                ret = Ok(());
                break;
            }
            Err(_) => {
                log::warn!(
                    "No response from peer for ID assignment, retrying; id={:02x}, uid={:08x}",
                    remote_id,
                    remote_uid
                );
                timeout_interval *= 2;
                ret = Err(AppError::timeout());
            }
        };
    }
    return ret;
}

async fn start_stream(
    streams_tx: Sender<streams::Operation>,
    stream_id: u32,
) -> Result<oneshot::Receiver<CanMessage>> {
    return start_or_continue_stream(streams_tx, stream_id, true).await;
}

async fn continue_stream(
    streams_tx: Sender<streams::Operation>,
    stream_id: u32,
) -> Result<oneshot::Receiver<CanMessage>> {
    return start_or_continue_stream(streams_tx, stream_id, false).await;
}

async fn create_wire(
    streams_tx: Sender<streams::Operation>,
) -> Result<(u32, oneshot::Receiver<CanMessage>)> {
    let (create_resp_tx, create_resp_rx) = oneshot::channel();
    let (stream_resp_tx, stream_resp_rx) = oneshot::channel();
    let operation = streams::Operation::CreateWire {
        op_resp: create_resp_tx,
        stream_resp: stream_resp_tx,
    };

    streams_tx.send(operation).await.unwrap();

    return match create_resp_rx.await.unwrap() {
        Ok(wire_id) => Ok((wire_id, stream_resp_rx)),
        Err(e) => {
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

            Err(error)
        }
    };
}

async fn start_or_continue_stream(
    streams_tx: Sender<streams::Operation>,
    stream_id: u32,
    is_start: bool,
) -> Result<oneshot::Receiver<CanMessage>> {
    let (start_resp_tx, start_resp_rx) = oneshot::channel();
    let (stream_resp_tx, stream_resp_rx) = oneshot::channel();
    let operation = if is_start {
        streams::Operation::Start {
            stream_id,
            op_resp: start_resp_tx,
            stream_resp: stream_resp_tx,
        }
    } else {
        streams::Operation::Continue {
            stream_id,
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

async fn terminate_stream(streams_tx: Sender<streams::Operation>, stream_id: u32) {
    let (term_resp_tx, term_resp_rx) = oneshot::channel();
    streams_tx
        .send(streams::Operation::Terminate {
            stream_id,
            op_resp: term_resp_tx,
        })
        .await
        .unwrap();
    term_resp_rx.await.unwrap().unwrap();
}
