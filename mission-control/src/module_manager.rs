use crate::analog3 as a3;
use crate::analog3::Value;
use crate::can_controller::{CanController, CanMessage};
use crate::operation::{Operation, OperationResult, Request, Response};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Write;
use std::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub enum ErrorType {
    A3OpCodeUnknown,
    A3OpCodeMissing,
    A3StreamConflict,
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

#[derive(Debug, Clone)]
struct Stream {
    consume_reply: fn(&CanMessage, &Stream) -> Result<()>,
    result_sender: Option<Sender<OperationResult>>,
}

pub struct ModuleManager<'a> {
    can_controller: &'a CanController,
    modules_by_id: HashMap<u8, Module>,
    modules_by_uid: HashMap<u32, Module>,
    streams: HashMap<u8, Stream>,
}

#[derive(Debug, Clone)]
struct Module {
    id: u8,
    uid: u32,
}

impl<'a> ModuleManager<'a> {
    pub fn new(can_controller: &'a CanController) -> Result<Self> {
        let new_instance = Self {
            can_controller,
            modules_by_uid: HashMap::new(),
            modules_by_id: HashMap::new(),
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
            a3::A3_IM_REPLY_PING => self.handle_ping_reply(message),
            a3::A3_IM_REPLY_NAME => self.handle_name_reply(message),
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
        let stream_id = (remote_id - a3::A3_ID_INDIVIDUAL_MODULE_BASE) as u8;
        log::debug!("Ping reply received; id {remote_id:03x}");
        match self.streams.remove(&stream_id) {
            Some(mut stream) => {
                (stream.consume_reply)(&in_message, &mut stream).and_then(|_| Ok(None))
            }
            None => Err(ModuleManagementError {
                error_type: ErrorType::UserCommandStreamIdMissing,
                message: format!("stream_id: {stream_id}"),
            }),
        }
    }

    fn handle_name_reply(&mut self, in_message: CanMessage) -> Result<Option<Result<Response>>> {
        let remote_id = (in_message.id() - a3::A3_ID_INDIVIDUAL_MODULE_BASE) as u8;
        let stream_id = remote_id;
        let mut data_dump = String::new();
        for i in 0..in_message.data_length() as usize {
            write!(data_dump, " {:02x}", in_message.data()[i]).unwrap();
        }
        log::debug!(
            "Name reply received; id {:03x} data:{}",
            remote_id,
            data_dump
        );
        match self.streams.get(&stream_id) {
            Some(mut stream) => {
                (stream.consume_reply)(&in_message, &mut stream).and_then(|_| Ok(None))
            }
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

    fn ping(&self, remote_id: u8, enable_visual: bool) -> Result<()> {
        let mut out_message = self.create_message();
        let length = if enable_visual { 3 } else { 2 };
        out_message.set_data_length(length);
        out_message.set_data(0, a3::A3_MC_PING);
        out_message.set_data(1, remote_id);
        if enable_visual {
            out_message.set_data(2, 1);
        }
        self.can_controller.put_message(out_message);
        return Ok(());
    }

    fn request_name(&self, remote_id: u8) -> Result<()> {
        let mut out_message = self.create_message();
        out_message.set_data_length(2);
        out_message.set_data(0, a3::A3_MC_REQUEST_NAME);
        out_message.set_data(1, remote_id);
        self.can_controller.put_message(out_message);
        return Ok(());
    }

    fn continue_name(&self, remote_id: u8) -> Result<()> {
        let mut out_message = self.create_message();
        out_message.set_data_length(2);
        out_message.set_data(0, a3::A3_MC_CONTINUE_NAME);
        out_message.set_data(1, remote_id);
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

    pub fn user_request(
        &mut self,
        request: &Request,
        result_sender: Sender<OperationResult>,
    ) -> Result<()> {
        match request.operation {
            Operation::List => self.process_list(request, result_sender),
            Operation::Ping => self.process_ping(request, result_sender),
            Operation::GetName => self.process_get_name(request, result_sender),
            Operation::AckName => self.process_ack_name(request),
            // Operation::ContinueName => self.process_continue_name(request, result_sender),
            Operation::RequestUidCancel => self.process_cancel_uid_request(request, result_sender),
            Operation::PretendSignIn => self.process_pseudo_sign_in(request, result_sender),
            Operation::PretendNotifyId => self.process_pseudo_notify_id(request, result_sender),
            Operation::Cancel => self.cancel_stream(request),
        }
    }

    fn complete(&self, result_sender: &Sender<OperationResult>, response: Response) -> Result<()> {
        result_sender.send(Ok(response)).unwrap();
        return Ok(());
    }

    fn complete_exceptionally(
        &self,
        result_sender: &Sender<OperationResult>,
        error: ModuleManagementError,
    ) -> Result<()> {
        result_sender.send(Err(error)).unwrap();
        return Ok(());
    }

    fn process_list(
        &mut self,
        _request: &Request,
        result_sender: Sender<OperationResult>,
    ) -> Result<()> {
        let mut reply = String::new();
        for (id, module) in &self.modules_by_id {
            if let Err(e) = write!(reply, "0x{:02x}: 0x{:08x}\r\n", id, module.uid) {
                return Err(ModuleManagementError {
                    error_type: ErrorType::RuntimeError,
                    message: e.to_string(),
                });
            }
        }

        return self.complete(
            &result_sender,
            Response {
                reply: reply.as_bytes().to_vec(),
                more: false,
                stream_id: 0,
            },
        );
    }

    fn process_ping(
        &mut self,
        request: &Request,
        result_sender: Sender<OperationResult>,
    ) -> Result<()> {
        let Value::U8(remote_id) = request.params[0] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The first parameter should be of type u8".to_string(),
            });
        };
        let mut enable_visual = false;
        if request.params.len() >= 2 {
            let Value::Bool(enabled) = request.params[1] else {
                return Err(ModuleManagementError {
                    error_type: ErrorType::UserCommandInvalidRequest,
                    message: "The second parameter should be of type bool".to_string(),
                });
            };
            enable_visual = enabled;
        };

        let stream_id = remote_id;
        if self.streams.contains_key(&stream_id) {
            return self.complete_exceptionally(
                &result_sender,
                ModuleManagementError {
                    error_type: ErrorType::A3StreamConflict,
                    message: format!("Another ping for 0x{:02x} is ongoing", remote_id),
                },
            );
        }
        self.streams.insert(
            stream_id,
            Stream {
                result_sender: Some(result_sender.clone()),
                consume_reply: |message, stream| {
                    log::info!("ping replied from: {:02x}", message.id());
                    let id = message.id() - a3::A3_ID_INDIVIDUAL_MODULE_BASE;
                    if let Some(sender) = &stream.result_sender {
                        let ok: OperationResult = Ok(Response {
                            reply: format!(" id 0x{:02x} replied\r\n", id).as_bytes().to_vec(),
                            more: false,
                            stream_id: 0,
                        });
                        if let Err(e) = sender.send(ok) {
                            log::error!("Failed to send ping reply: {e:?}");
                        }
                    }
                    return Ok(());
                },
            },
        );

        self.ping(remote_id, enable_visual)?;

        return self.complete(
            &result_sender,
            Response {
                reply: format!("sent to id {:02x} ...", remote_id)
                    .as_bytes()
                    .to_vec(),
                more: true,
                stream_id,
            },
        );
    }

    fn process_get_name(
        &mut self,
        request: &Request,
        result_sender: Sender<OperationResult>,
    ) -> Result<()> {
        let Value::U8(remote_id) = request.params[0] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The first parameter should be of type u8".to_string(),
            });
        };

        let stream_id = remote_id;
        if self.streams.contains_key(&stream_id) {
            return Err(ModuleManagementError {
                error_type: ErrorType::A3StreamConflict,
                message: format!("Another ping for 0x{:02x} is ongoing", remote_id),
            });
        }
        self.streams.insert(
            stream_id,
            Stream {
                result_sender: Some(result_sender.clone()),
                consume_reply: |message, stream| {
                    let len = message.data_length() as usize;
                    let mut data_dump = String::new();
                    for i in 0..len {
                        write!(data_dump, " {:02x}", message.data()[i]).unwrap();
                    }
                    log::debug!("handler receives:{}", data_dump);
                    if let Some(sender) = &stream.result_sender {
                        let ok: OperationResult = Ok(Response {
                            reply: message.data()[0..len].to_vec(),
                            more: false,
                            stream_id: 0,
                        });
                        if let Err(e) = sender.send(ok) {
                            log::error!("Failed to send ping reply: {e:?}");
                        }
                    }
                    return Ok(());
                },
            },
        );

        self.request_name(remote_id)?;

        return Ok(());
    }

    fn process_ack_name(&mut self, request: &Request) -> Result<()> {
        let Value::U8(remote_id) = request.params[0] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The first parameter should be of type u8".to_string(),
            });
        };
        let Value::Bool(is_done) = request.params[1] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The second parameter should be of type bool".to_string(),
            });
        };
        log::debug!("ack_name: id={}, done={}", remote_id, is_done);

        let stream_id = remote_id;

        if is_done {
            self.streams.remove(&stream_id);
        } else {
            self.continue_name(remote_id)?;
        }

        return Ok(());
    }

    fn process_cancel_uid_request(
        &mut self,
        request: &Request,
        result_sender: Sender<OperationResult>,
    ) -> Result<()> {
        let Value::U32(remote_uid) = request.params[0] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The first parameter should be of type u32".to_string(),
            });
        };
        let mut reply = String::new();
        if let Some(module) = self.modules_by_uid.remove(&remote_uid) {
            self.modules_by_id.remove(&module.id);
        } else {
            if let Err(e) = write!(
                reply,
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
            reply,
            "UID cancel request sent for uid {:08x}\r\n",
            remote_uid
        ) {
            return Err(ModuleManagementError {
                error_type: ErrorType::RuntimeError,
                message: e.to_string(),
            });
        }
        return self.complete(
            &result_sender,
            Response {
                reply: reply.as_bytes().to_vec(),
                more: false,
                stream_id: 0,
            },
        );
    }

    fn process_pseudo_sign_in(
        &mut self,
        request: &Request,
        result_sender: Sender<OperationResult>,
    ) -> Result<()> {
        let Value::U32(remote_uid) = request.params[0] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The first parameter should be of type u32".to_string(),
            });
        };
        self.im_sign_in(remote_uid)?;
        self.complete(
            &result_sender,
            Response {
                reply: format!("Sign-in sent as uid {:08x}\r\n", remote_uid)
                    .as_bytes()
                    .to_vec(),
                more: false,
                stream_id: 0,
            },
        )
    }

    fn process_pseudo_notify_id(
        &mut self,
        request: &Request,
        result_sender: Sender<OperationResult>,
    ) -> Result<()> {
        let Value::U32(remote_uid) = request.params[0] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The first parameter should be of type u32".to_string(),
            });
        };
        let Value::U8(remote_id) = request.params[1] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The second parameter should be of type u8".to_string(),
            });
        };
        self.im_notify_id(remote_uid, remote_id)?;
        self.complete(
            &result_sender,
            Response {
                reply: format!(
                    "Notify ID sent as uid {:08x} id {:02x}\r\n",
                    remote_uid, remote_id
                )
                .as_bytes()
                .to_vec(),
                more: false,
                stream_id: 0,
            },
        )
    }

    fn cancel_stream(&mut self, request: &Request) -> Result<()> {
        let Value::U8(stream_id) = request.params[0] else {
            return Err(ModuleManagementError {
                error_type: ErrorType::UserCommandInvalidRequest,
                message: "The first parameter should be of type u8".to_string(),
            });
        };
        let result = if let Some(_) = self.streams.remove(&stream_id) {
            "success"
        } else {
            "not found"
        };
        log::debug!("Stream {} cancelled; result={}", stream_id, result);
        return Ok(());
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
