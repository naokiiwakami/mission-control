mod streams;

use crate::{
    a3_message,
    a3_modules::{self, A3Module},
    analog3::{
        self as a3, PropertyId, StreamStatus,
        config::{ChunkParser, Property, PropertyEncoder},
        schema::{MODULES_SCHEMA, ModuleDef, ValueType},
    },
    can_controller::CanMessage,
    command::Command,
    error::{AppError, ErrorType},
};

use tokio::{
    sync::{mpsc::Sender, oneshot},
    time::{Duration, sleep, timeout},
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
        if message.is_extended() {
            log::debug!("Message received: id={:08x}", message.id());
            self.handle_extended_message(message);
        } else {
            if log::log_enabled!(log::Level::Debug) {
                let mut data_elements = Vec::<String>::new();
                for i in 0..message.data_length() as usize {
                    data_elements.push(format!("{:02x}", message.data()[i]));
                }
                log::debug!(
                    "Message received: id={:04x} data={}",
                    message.id() as u16,
                    data_elements.join(" ")
                );
            }
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
        let remote_id = message.id() as u16;
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
            let stream_id = remote_id as u16 + a3::A3_ID_INDIVIDUAL_MODULE_BASE;
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
            let remote_id = in_message.id() as u16;
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
            Command::GetModule { id, resp } => self.get_module(id, resp),
            Command::GetSchema { id, resp } => self.get_schema(id, resp),
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

    fn get_module(&mut self, id: u8, resp: oneshot::Sender<Result<A3Module>>) {
        let modules_tx = self.modules_tx.clone();
        tokio::spawn(async move {
            let (tx, rx) = oneshot::channel();
            modules_tx
                .send(a3_modules::Operation::GetById { id, resp: tx })
                .await
                .unwrap();
            resp.send(rx.await.unwrap()).unwrap();
        });
    }

    fn get_schema(&mut self, id: u8, resp: oneshot::Sender<Result<ModuleDef>>) {
        let modules_tx = self.modules_tx.clone();
        tokio::spawn(async move {
            let (tx, rx) = oneshot::channel();
            modules_tx
                .send(a3_modules::Operation::GetById { id, resp: tx })
                .await
                .unwrap();
            let result: Result<ModuleDef> = match rx.await.unwrap() {
                Ok(module) => match module.module_type_id {
                    Some(tid) => match MODULES_SCHEMA.get(&tid) {
                        Some(value) => Ok(value.clone()),
                        None => Err(AppError::new(
                            ErrorType::A3SchemaError,
                            "Schema unknown".to_string(),
                        )),
                    },
                    None => Err(AppError::new(
                        ErrorType::A3SchemaError,
                        "Module type could not be resolved. Consider running get-config"
                            .to_string(),
                    )),
                },
                Err(e) => Err(e),
            };
            resp.send(result).unwrap();
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

            terminate_stream(streams_tx, id as u16 + a3::A3_ID_INDIVIDUAL_MODULE_BASE).await;
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
        let modules_tx = self.modules_tx.clone();
        let streams_tx = self.streams_tx.clone();
        let can_tx = self.can_tx.clone();
        tokio::spawn(async move {
            match create_wire(streams_tx.clone()).await {
                Ok((wire_addr, stream_resp_rx)) => {
                    let result = get_config_core(
                        streams_tx.clone(),
                        can_tx,
                        modules_tx,
                        id,
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

    fn set_config(&mut self, id: u8, props: Vec<Property>, resp: oneshot::Sender<Result<()>>) {
        let streams_tx = self.streams_tx.clone();
        let can_tx = self.can_tx.clone();
        let modules_tx = self.modules_tx.clone();
        tokio::spawn(async move {
            match create_wire(streams_tx.clone()).await {
                Ok((wire_addr, stream_resp_rx)) => {
                    let result = set_config_core(
                        streams_tx.clone(),
                        can_tx,
                        modules_tx,
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
    let stream_id = id as u16 + a3::A3_ID_INDIVIDUAL_MODULE_BASE;

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
    wire_id: u16,
    init_stream_resp_rx: oneshot::Receiver<CanMessage>,
) -> Result<String> {
    let mut stream_resp_rx = Some(init_stream_resp_rx);
    let wire_addr = (wire_id - a3::A3_ID_ADMIN_WIRES_BASE) as u8;

    // send request message
    initiate_stream(
        &streams_tx,
        &can_tx,
        a3::A3_MC_REQUEST_NAME,
        id,
        wire_id,
        &mut stream_resp_rx,
    )
    .await?;

    // control the stream
    let mut chunk_parser = ChunkParser::for_single_field();
    loop {
        stream_resp_rx.replace(continue_stream(streams_tx.clone(), wire_id).await?);
        a3_message::continue_stream(can_tx.clone(), id, wire_addr).await;
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
    modules_tx: Sender<a3_modules::Operation>,
    id: u8,
    wire_id: u16,
    init_stream_resp_rx: oneshot::Receiver<CanMessage>,
) -> Result<Vec<Property>> {
    let mut stream_resp_rx = Some(init_stream_resp_rx);
    let wire_num = (wire_id - a3::A3_ID_ADMIN_WIRES_BASE) as u8;

    initiate_stream(
        &streams_tx,
        &can_tx,
        a3::A3_MC_REQUEST_CONFIG,
        id,
        wire_id,
        &mut stream_resp_rx,
    )
    .await?;

    // control the stream
    let mut chunk_parser = ChunkParser::new();
    loop {
        stream_resp_rx.replace(continue_stream(streams_tx.clone(), wire_id).await?);
        a3_message::continue_stream(can_tx.clone(), id, wire_num).await;
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
                    // TODO: make following a subroutine.
                    let mut name: Option<String> = None;
                    let mut module_type: Option<String> = None;
                    let mut module_type_id: Option<u16> = None;
                    for property in &properties {
                        let Ok(property_id) = PropertyId::try_from(property.id) else {
                            continue;
                        };
                        match property_id {
                            PropertyId::Name => {
                                name.replace(property.get_value_as_string().unwrap());
                            }
                            PropertyId::ModuleType => {
                                let type_id: u16 = property
                                    .get_value_with_type(&ValueType::U16)
                                    .as_u16()
                                    .unwrap();
                                module_type_id.replace(type_id);
                                if let Some(module_def) = MODULES_SCHEMA.get(&type_id) {
                                    module_type.replace(module_def.module_type_name.clone());
                                }
                            }
                            _ => {}
                        }
                    }
                    let modules_op = a3_modules::Operation::SetProperties {
                        id,
                        name,
                        module_type,
                        module_type_id,
                    };
                    modules_tx.send(modules_op).await.unwrap();
                    return Ok(properties);
                }
            }
            Err(e) => {
                let message = format!("GetName: Data parsing failed: {:?}", e);
                return Err(AppError::runtime(message.as_str()));
            }
        }
    }
}

/// send request message and see if the counterpart is ready.
async fn initiate_stream(
    streams_tx: &Sender<streams::Operation>,
    can_tx: &Sender<CanMessage>,
    opcode: u8,
    id: u8,
    wire_id: u16,
    stream_resp_rx: &mut Option<oneshot::Receiver<CanMessage>>,
) -> Result<()> {
    let wire_num = (wire_id - a3::A3_ID_ADMIN_WIRES_BASE) as u8;

    const MAX_TRIALS: usize = 3;
    let mut num_trials = 0usize;
    let mut sleep_millis = 100u64;
    loop {
        a3_message::request_command(can_tx.clone(), opcode, id, wire_num).await;
        if let Ok(resp) = timeout(Duration::from_secs(10), stream_resp_rx.take().unwrap()).await {
            let message = resp.unwrap();
            if message.data_length() < 1 {
                return Err(AppError::new(
                    ErrorType::A3ProtocolError,
                    "Status is missing in response".to_string(),
                ));
            }
            let Ok(status) = StreamStatus::try_from(message.data()[0]) else {
                return Err(AppError::new(
                    ErrorType::A3InvalidValue,
                    format!("status {}", message.data()[0]),
                ));
            };
            match status {
                StreamStatus::Ready => {
                    break;
                }
                StreamStatus::Busy => {
                    // continue
                }
                _ => {
                    return Err(AppError::new(
                        ErrorType::A3CommunicationError,
                        format!("status {:?}", status),
                    ));
                }
            }
        } else {
            return Err(AppError::timeout());
        };
        num_trials += 1;
        if num_trials == MAX_TRIALS {
            return Err(AppError::new(
                ErrorType::A3CommunicationError,
                "Remote peer is busy".to_string(),
            ));
        }
        sleep(Duration::from_millis(sleep_millis)).await;
        sleep_millis *= 2;
        stream_resp_rx.replace(continue_stream(streams_tx.clone(), wire_id).await?);
    }
    Ok(())
}

async fn set_config_core(
    streams_tx: Sender<streams::Operation>,
    can_tx: Sender<CanMessage>,
    modules_tx: Sender<a3_modules::Operation>,
    id: u8,
    props: Vec<Property>,
    wire_addr: u16,
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
        out_message.set_id(wire_addr as u32);
        out_message.set_extended(false);
        out_message.set_remote(false);
        let num_flushed_bytes = encoder.flush(out_message.mut_data());
        out_message.set_data_length(num_flushed_bytes as u8);
        if !encoder.is_done() {
            stream_resp_rx.replace(continue_stream(streams_tx.clone(), wire_addr).await?);
        }
        can_tx.send(out_message).await.unwrap();
    }
    let mut name: Option<String> = None;
    for prop in &props {
        if prop.id == PropertyId::Name as u8 {
            name.replace(prop.get_value_as_string().unwrap());
            break;
        }
    }
    if name.is_some() {
        let module_type: Option<String> = None;
        let module_type_id: Option<u16> = None;
        let modules_op = a3_modules::Operation::SetProperties {
            id,
            name,
            module_type,
            module_type_id,
        };
        modules_tx.send(modules_op).await.unwrap();
    }
    return Ok(());
}

async fn assign_remote_id(
    streams_tx: Sender<streams::Operation>,
    can_tx: Sender<CanMessage>,
    stream_id: u16,
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
    stream_id: u16,
) -> Result<oneshot::Receiver<CanMessage>> {
    return start_or_continue_stream(streams_tx, stream_id, true).await;
}

async fn continue_stream(
    streams_tx: Sender<streams::Operation>,
    stream_id: u16,
) -> Result<oneshot::Receiver<CanMessage>> {
    return start_or_continue_stream(streams_tx, stream_id, false).await;
}

async fn create_wire(
    streams_tx: Sender<streams::Operation>,
) -> Result<(u16, oneshot::Receiver<CanMessage>)> {
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
    stream_id: u16,
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

async fn terminate_stream(streams_tx: Sender<streams::Operation>, stream_id: u16) {
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
