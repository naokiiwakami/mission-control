use std::collections::HashMap;
use std::fmt;
use std::fmt::Write;

use crate::analog3 as a3;
use crate::can_controller::{CanController, CanMessage};
use crate::command_processor::Response;

#[derive(Debug, Clone)]
pub enum ErrorType {
    A3OpCodeUnknown,
    A3OpCodeMissing,
    UserCommandUnknown,
    UserCommandStreamIdMissing,
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
    pub fn new(can_controller: &'a CanController) -> Self {
        let new_instance = Self {
            can_controller: can_controller,
            modules_by_uid: HashMap::new(),
            modules_by_id: HashMap::new(),
            next_stream_id: 0,
            streams: HashMap::new(),
        };
        new_instance.sign_in();
        return new_instance;
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
                a3::A3_ADMIN_SIGN_IN => self.assign_module_id(message),
                a3::A3_ADMIN_NOTIFY_ID => self.register_module(message),
                _ => {
                    return Err(ModuleManagementError {
                        error_type: ErrorType::A3OpCodeUnknown,
                        message: format!("{opcode:02x}"),
                    });
                }
            };
        }
        return match opcode {
            a3::A3_IM_PING_REPLY => self.ping_reply(message),
            _ => {
                return Err(ModuleManagementError {
                    error_type: ErrorType::A3OpCodeUnknown,
                    message: format!("{opcode:02x}"),
                });
            }
        };
    }

    fn create_message(&self) -> CanMessage {
        let mut message = self.can_controller.create_message();
        message.set_id(a3::A3_ID_MISSION_CONTROL);
        return message;
    }

    fn sign_in(&self) {
        let mut out_message = self.create_message();
        out_message.set_id(a3::A3_ID_MISSION_CONTROL);
        out_message.set_data_length(1);
        out_message.set_data(0, a3::A3_MC_SIGN_IN);
        self.can_controller.put_message(out_message);
    }

    fn find_available_id(&self) -> u8 {
        for id in 1..=255 {
            if !self.modules_by_id.contains_key(&id) {
                return id;
            }
        }
        return 0;
    }

    fn assign_module_id(&mut self, in_message: CanMessage) -> Result<Option<Result<Response>>> {
        let remote_uid = in_message.id();
        let remote_id = match self.modules_by_uid.get(&remote_uid) {
            Some(module) => module.id,
            None => self.find_available_id(),
        };
        let mut out_message = self.create_message();
        out_message.set_data_length(6);
        out_message.set_data(0, a3::A3_MC_ASSIGN_MODULE_ID);
        out_message.set_data(1, ((remote_uid >> 24) & 0xff) as u8);
        out_message.set_data(2, ((remote_uid >> 16) & 0xff) as u8);
        out_message.set_data(3, ((remote_uid >> 8) & 0xff) as u8);
        out_message.set_data(4, (remote_uid & 0xff) as u8);
        out_message.set_data(5, remote_id);
        self.can_controller.put_message(out_message);
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

    fn register_module(&mut self, in_message: CanMessage) -> Result<Option<Result<Response>>> {
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

    fn ping_reply(&mut self, in_message: CanMessage) -> Result<Option<Result<Response>>> {
        let remote_id = in_message.id();
        let stream_id = in_message.get_data(1);
        log::debug!("Ping reply received; id {remote_id:03x}");
        match self.streams.remove(&stream_id) {
            Some(client_id) => Ok(Some(Ok(Response {
                client_id: client_id,
                reply: Some("ok\r\n".to_string()),
            }))),
            None => Err(ModuleManagementError {
                error_type: ErrorType::UserCommandStreamIdMissing,
                message: format!("stream_id: {stream_id}"),
            }),
        }
    }

    pub fn user_request(&mut self, command: &String, client_id: u32) -> Result<Response> {
        match command.as_str() {
            "list" => {
                let mut out = String::new();
                for (id, module) in &self.modules_by_id {
                    write!(out, "{:02x}: {:08x}\r\n", id, module.uid).unwrap();
                }
                Ok(Response {
                    client_id: client_id,
                    reply: Some(out),
                })
            }
            "ping" => {
                let remote_id = 1u8; // PoC yet
                let stream_id = self.next_stream_id;
                self.next_stream_id += 1;
                self.streams.insert(stream_id, client_id);
                let mut out_message = self.create_message();
                out_message.set_data_length(3);
                out_message.set_data(0, a3::A3_MC_PING);
                out_message.set_data(1, remote_id);
                out_message.set_data(2, stream_id);
                self.can_controller.put_message(out_message);
                Ok(Response {
                    client_id: client_id,
                    reply: None,
                })
            }
            _ => Err(ModuleManagementError {
                error_type: ErrorType::UserCommandUnknown,
                message: format!("Unknown command: {command}\r\n"),
            }),
        }
    }
}
