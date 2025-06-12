use crate::{
    a3_message, a3_modules, analog3 as a3, can_controller::CanMessage,
    error::ModuleManagementError, operation::Command,
};
use std::collections::HashMap;
use tokio::sync::{mpsc::Sender, oneshot};

type Result<T> = std::result::Result<T, ModuleManagementError>;

pub struct ModuleManager {
    can_tx: Sender<CanMessage>,
    modules_tx: Sender<a3_modules::Operation>,
}

impl ModuleManager {
    pub fn new(can_tx: Sender<CanMessage>, modules_tx: Sender<a3_modules::Operation>) -> Self {
        Self { can_tx, modules_tx }
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
            // a3::A3_IM_REPLY_PING => self.handle_ping_reply(message),
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

    // Command handling ///////////////////////////////////////////////////////////////

    pub fn handle_command(&mut self, command: Command) {
        let modules_tx = self.modules_tx.clone();
        tokio::spawn(async move {
            match command {
                Command::Hi { resp } => {
                    resp.send("hello\r\n".to_string()).unwrap();
                }
                Command::List { resp } => {
                    let (tx, rx) = oneshot::channel();
                    modules_tx
                        .send(a3_modules::Operation::List { resp: tx })
                        .await
                        .unwrap();
                    match rx.await.unwrap() {
                        Ok(list) => {
                            resp.send(list);
                        }
                        Err(e) => {
                            log::error!("An error encountered while listing modules: {:?}", e);
                        }
                    }
                }
                _ => {
                    log::error!("Operation not implemented: {:?}", command);
                }
            }
        });
    }
}
