use std::collections::HashMap;
use std::fmt;
use std::fmt::Write;

use crate::analog3 as a3;
use crate::can_controller::{CanController, CanMessage};
use crate::operation::{Operation, Request, RequestParam, Response};

#[derive(Debug, Clone)]
pub enum ErrorType {
    A3OpCodeUnknown,
    A3OpCodeMissing,
    UserCommandUnknown,
    UserCommandStreamIdMissing,
    UserCommandInvalidRequest,
    RuntimeError,
}

#[derive(Debug, Clone)]
pub struct ModuleManagementError {
    pub error_type: ErrorType,
    pub message: String,
}

impl fmt::Display for ModuleManagementError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}; {}", self.error_type, self.message)
    }
}

impl std::error::Error for ModuleManagementError {}

type Result<T> = std::result::Result<T, ModuleManagementError>;

pub struct ModuleManager<'a> {
    can_controller: &'a CanController,
    modules_by_id: HashMap<u8, Module>,
    modules_by_uid: HashMap<u32, Module>,
    next_stream_id: u8,
    streams: HashMap<u8, u32>,
}

#[derive(Debug, Clone)]
struct Module {
    id: u8,
    uid: u32,
}

impl<'a> ModuleManager<'a> {
    pub fn new(can_controller: &'a CanController) -> Result<Self> {
        let new_instance = Self {
            can_controller: can_controller,
            modules_by_uid: HashMap::new(),
            modules_by_id: HashMap::new(),
            next_stream_id: 0,
            streams: HashMap::new(),
        };
        new_instance.sign_in()?;
        return Ok(new_instance);
    }

    pub fn handle_message(&mut self, message: CanMessage) -> Result<Option<Result<Response>>> {
        log::debug!("Message received: id={:08x}", message.id());
        if message.data_length() == 0 {
            return Err(ModuleManagementError {
                error_type: ErrorType::A3OpCodeMissing,
                message: "".to_string(),
            });
        }
        let opcode = message.get_data(0);
        if message.is_extended() {
            return match opcode {
                a3::A3_ADMIN_SIGN_IN => self.handle_remote_sign_in(message),
                a3::A3_ADMIN_NOTIFY_ID => self.handle_remote_id_notification(message),
                a3::A3_ADMIN_REQ_UID_CANCEL => self.handle_uid_cancel_req(message),
                _ => {
                    return Err(ModuleManagementError {
                        error_type: ErrorType::A3OpCodeUnknown,
                        message: format!("{opcode:02x}"),
                    });
                }
            };
        }
        return match opcode {
            a3::A3_IM_PING_REPLY => self.handle_ping_reply(message),
            _ => {
                return Err(ModuleManagementError {
                    error_type: ErrorType::A3OpCodeUnknown,
                    message: format!("{opcode:02x}"),
                });
            }
        };
    }

    fn handle_remote_sign_in(
        &mut self,
        in_message: CanMessage,
    ) -> Result<Option<Result<Response>>> {
        let remote_uid = in_message.id();
        let remote_id = match self.modules_by_uid.get(&remote_uid) {
            Some(module) => module.id,
            None => self.find_available_id(),
        };
        self.assign_module_id(remote_uid, remote_id)?;
        let module = Module {
            id: remote_id,
            uid: remote_uid,
        };
        self.modules_by_id.insert(module.id, module.clone());
        self.modules_by_uid.insert(module.uid, module);
        log::info!(
            "Issued module id {:03x} for uid {:08x}",
            remote_id,
            remote_uid
        );
        return Ok(None);
    }

    fn handle_remote_id_notification(
        &mut self,
        in_message: CanMessage,
    ) -> Result<Option<Result<Response>>> {
        let remote_uid = in_message.id();
        let remote_id = in_message.get_data(1);
        log::debug!("Module recognized; id {remote_id:03x} for uid {remote_uid:08x}");
        let module = Module {
            id: remote_id,
            uid: remote_uid,
        };
        self.modules_by_id.insert(module.id, module.clone());
        self.modules_by_uid.insert(module.uid, module);
        return Ok(None);
    }

    fn handle_uid_cancel_req(
        &mut self,
        in_message: CanMessage,
    ) -> Result<Option<Result<Response>>> {
        let remote_uid = in_message.id();
        log::debug!("Module UID cancel requested; uid {remote_uid:08x}");
        if let Some(module) = self.modules_by_uid.remove(&remote_uid) {
            self.modules_by_id.remove(&module.id);
        }
        return Ok(None);
    }

    fn handle_ping_reply(&mut self, in_message: CanMessage) -> Result<Option<Result<Response>>> {
        let remote_id = in_message.id();
        let stream_id = in_message.get_data(1);
        log::debug!("Ping reply received; id {remote_id:03x}");
        match self.streams.remove(&stream_id) {
            Some(client_id) => Ok(Some(Ok(Response {
                client_id: client_id,
                reply: Some(" replied\r\n".to_string()),
            }))),
            None => Err(ModuleManagementError {
                error_type: ErrorType::UserCommandStreamIdMissing,
                message: format!("stream_id: {stream_id}"),
            }),
        }
    }

    // A3 operations: TX //////////////////////////////////////////////////

    fn sign_in(&self) -> Result<()> {
        let mut out_message = self.create_message();
        out_message.set_id(a3::A3_ID_MISSION_CONTROL);
        out_message.set_data_length(1);
        out_message.set_data(0, a3::A3_MC_SIGN_IN);
        self.can_controller.put_message(out_message);
        return Ok(());
    }

    fn assign_module_id(&self, remote_uid: u32, remote_id: u8) -> Result<()> {
        let mut out_message = self.create_message();
        out_message.set_data_length(6);
        out_message.set_data(0, a3::A3_MC_ASSIGN_MODULE_ID);
        out_message.set_data(1, ((remote_uid >> 24) & 0xff) as u8);
        out_message.set_data(2, ((remote_uid >> 16) & 0xff) as u8);
        out_message.set_data(3, ((remote_uid >> 8) & 0xff) as u8);
        out_message.set_data(4, (remote_uid & 0xff) as u8);
        out_message.set_data(5, remote_id);
        self.can_controller.put_message(out_message);
        return Ok(());
    }

    fn ping(&self, remote_id: u8, stream_id: u8) -> Result<()> {
        let mut out_message = self.create_message();
        out_message.set_data_length(3);
        out_message.set_data(0, a3::A3_MC_PING);
        out_message.set_data(1, remote_id);
        out_message.set_data(2, stream_id);
        self.can_controller.put_message(out_message);
        return Ok(());
    }

    fn cancel_uid(&self, remote_uid: u32) -> Result<()> {
        let mut out_message = self.can_controller.create_message();
        out_message.set_id(remote_uid);
        out_message.set_extended(true);
        out_message.set_data_length(1);
        out_message.set_data(0, a3::A3_ADMIN_REQ_UID_CANCEL);
        self.can_controller.put_message(out_message);
        return Ok(());
    }

    fn im_sign_in(&self, remote_uid: u32) -> Result<()> {
        let mut out_message = self.can_controller.create_message();
        out_message.set_id(remote_uid);
        out_message.set_extended(true);
        out_message.set_data_length(1);
        out_message.set_data(0, a3::A3_ADMIN_SIGN_IN);
        self.can_controller.put_message(out_message);
        return Ok(());
    }

    fn im_notify_id(&self, remote_uid: u32, remote_id: u8) -> Result<()> {
        let mut out_message = self.can_controller.create_message();
        out_message.set_id(remote_uid);
        out_message.set_extended(true);
        out_message.set_data_length(2);
        out_message.set_data(0, a3::A3_ADMIN_NOTIFY_ID);
        out_message.set_data(1, remote_id);
        self.can_controller.put_message(out_message);
        return Ok(());
    }

    // User command handling /////////////////////////////////////////////////////

    pub fn user_request(&mut self, request: &Request) -> Result<Response> {
        match request.operation {
            Operation::List => self.process_list(request),
            Operation::Ping => self.process_ping(request),
            Operation::RequestUidCancel => self.process_cancel_uid_request(request),
            Operation::PretendSignIn => self.process_pseudo_sign_in(request),
            Operation::PretendNotifyId => self.process_pseudo_notify_id(request),
            _ => Err(ModuleManagementError {
                error_type: ErrorType::UserCommandUnknown,
                message: format!("Unknown command: {}\r\n", request.command),
            }),
        }
    }

    fn process_list(&mut self, request: &Request) -> Result<Response> {
        let mut out = String::new();
        for (id, module) in &self.modules_by_id {
            if let Err(e) = write!(out, "0x{:02x}: 0x{:08x}\r\n", id, module.uid) {
                return Err(ModuleManagementError {
                    error_type: ErrorType::RuntimeError,
                    message: e.to_string(),
                });
            }
        }
        return Ok(Response {
            client_id: request.client_id,
            reply: Some(out),
        });
    }

    fn process_ping(&mut self, request: &Request) -> Result<Response> {
        let client_id = request.client_id;
        let RequestParam::U8(remote_id) = request.params[0] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The first parameter should be of type u8".to_string(),
            });
        };

        let stream_id = self.next_stream_id;
        self.next_stream_id += 1;
        self.streams.insert(stream_id, client_id);

        self.ping(remote_id, stream_id)?;

        return Ok(Response {
            client_id: client_id,
            reply: None,
        });
    }

    fn process_cancel_uid_request(&mut self, request: &Request) -> Result<Response> {
        let client_id = request.client_id;
        let RequestParam::U32(remote_uid) = request.params[0] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The first parameter should be of type u32".to_string(),
            });
        };
        let mut out = String::new();
        if let Some(module) = self.modules_by_uid.remove(&remote_uid) {
            self.modules_by_id.remove(&module.id);
        } else {
            if let Err(e) = write!(
                out,
                "Warn: The uid 0x{:08x} not found in the module list, sending the message anyway\r\n",
                remote_uid
            ) {
                return Err(ModuleManagementError {
                    error_type: ErrorType::RuntimeError,
                    message: e.to_string(),
                });
            }
        }
        self.cancel_uid(remote_uid)?;
        if let Err(e) = write!(
            out,
            "UID cancel request sent for uid {:08x}\r\n",
            remote_uid
        ) {
            return Err(ModuleManagementError {
                error_type: ErrorType::RuntimeError,
                message: e.to_string(),
            });
        }
        Ok(Response {
            client_id: client_id,
            reply: Some(out),
        })
    }

    fn process_pseudo_sign_in(&mut self, request: &Request) -> Result<Response> {
        let client_id = request.client_id;
        let RequestParam::U32(remote_uid) = request.params[0] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The first parameter should be of type u32".to_string(),
            });
        };
        self.im_sign_in(remote_uid)?;
        Ok(Response {
            client_id: client_id,
            reply: Some(format!("Sign-in sent as uid {:08x}\r\n", remote_uid)),
        })
    }

    fn process_pseudo_notify_id(&mut self, request: &Request) -> Result<Response> {
        let client_id = request.client_id;
        let RequestParam::U32(remote_uid) = request.params[0] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The first parameter should be of type u32".to_string(),
            });
        };
        let RequestParam::U8(remote_id) = request.params[1] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The second parameter should be of type u8".to_string(),
            });
        };
        self.im_notify_id(remote_uid, remote_id)?;
        Ok(Response {
            client_id: client_id,
            reply: Some(format!(
                "Notify ID sent as uid {:08x} id {:02x}\r\n",
                remote_uid, remote_id
            )),
        })
    }

    // Utilities /////////////////////////////////////////////////////

    fn create_message(&self) -> CanMessage {
        let mut message = self.can_controller.create_message();
        message.set_id(a3::A3_ID_MISSION_CONTROL);
        return message;
    }

    fn find_available_id(&self) -> u8 {
        for id in 1..=255 {
            if !self.modules_by_id.contains_key(&id) {
                return id;
            }
        }
        return 0;
    }
}
