mod streams;

use std::time::Duration;

use tokio::sync::{mpsc::Sender, oneshot};
use tokio::time::timeout;

use crate::a3_message;
use crate::a3_modules;
use crate::a3_modules::A3Module;
use crate::analog3 as a3;
use crate::can_controller::CanMessage;
use crate::error;
use crate::error::ModuleManagementError;
use crate::operation::Command;

type Result<T> = std::result::Result<T, ModuleManagementError>;

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
            a3::A3_IM_REPLY_PING => self.handle_ping_reply(message).unwrap(),
            // a3::A3_IM_REPLY_NAME => self.handle_name_reply(message),
            // a3::A3_IM_REPLY_CONFIG => self.handle_name_reply(message),
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

    fn handle_ping_reply(&mut self, in_message: CanMessage) -> Result<()> {
        let streams_tx = self.streams_tx.clone();
        tokio::spawn(async move {
            let remote_id = in_message.id();
            let stream_id = (remote_id - a3::A3_ID_INDIVIDUAL_MODULE_BASE) as u8;
            log::debug!("Ping reply received; id {remote_id:02x}");
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
                        "An error encountered while finding stream for ping: {:?}",
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
            _ => {
                log::error!("Operation not implemented: {:?}", command);
            }
        }
    }

    fn hi(&mut self, resp: oneshot::Sender<String>) {
        tokio::spawn(async {
            resp.send("hello\r\n".to_string()).unwrap();
        });
    }

    fn list(
        &mut self,
        resp: oneshot::Sender<std::result::Result<Vec<A3Module>, ModuleManagementError>>,
    ) {
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

    fn ping(
        &mut self,
        id: u8,
        enable_visual: bool,
        resp: oneshot::Sender<std::result::Result<(), ModuleManagementError>>,
    ) {
        let streams_tx = self.streams_tx.clone();
        let can_tx = self.can_tx.clone();
        tokio::spawn(async move {
            // start a stream
            let (start_resp_tx, start_resp_rx) = oneshot::channel();
            let (stream_resp_tx, stream_resp_rx) = oneshot::channel();
            let operation = streams::Operation::Start {
                remote_id: id,
                op_resp: start_resp_tx,
                stream_resp: stream_resp_tx,
            };
            streams_tx.send(operation).await.unwrap();
            if let Err(e) = start_resp_rx.await.unwrap() {
                let error = match e.error_type {
                    streams::ErrorType::Busy => ModuleManagementError {
                        error_type: crate::error::ErrorType::A3StreamConflict,
                        message: "busy".to_string(),
                    },
                    _ => ModuleManagementError {
                        error_type: crate::error::ErrorType::RuntimeError,
                        message: format!("{:?}", e),
                    },
                };
                resp.send(Err(error)).unwrap();
                return;
            }
            // ping
            a3_message::ping(can_tx, id, enable_visual).await;

            // wait for the response and
            if let Err(_) = timeout(Duration::from_secs(10), stream_resp_rx).await {
                resp.send(Err(ModuleManagementError::new(
                    error::ErrorType::Timeout,
                    "".to_string(),
                )))
                .unwrap();
            } else {
                resp.send(Ok(())).unwrap();
            }
            let (term_resp_tx, term_resp_rx) = oneshot::channel();

            // terminate the stream
            streams_tx
                .send(streams::Operation::Terminate {
                    remote_id: id,
                    op_resp: term_resp_tx,
                })
                .await
                .unwrap();
            term_resp_rx.await.unwrap().unwrap();
        });
    }
}
