use crate::event_type::EventType;
use crate::module_manager::{ErrorType, ModuleManagementError};
use dashmap::DashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::num::ParseIntError;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

pub type CommandResult = Result<Option<String>, ModuleManagementError>;

#[derive(Debug)]
pub enum Param {
    U8(u8),
    U16(u16),
    U32(u32),
    Text(String),
}

#[derive(Debug)]
pub struct Request {
    pub client_id: u32,
    pub command: String,
    pub params: Vec<Param>,
}

#[derive(Debug)]
pub struct Response {
    pub client_id: u32,
    pub reply: Option<String>,
}

struct ClientHandler {
    client_id: u32,
    stream: TcpStream,
    notifier: Sender<EventType>,
    request_sender: Sender<Request>,
    result_receiver: Receiver<CommandResult>,
}

impl ClientHandler {
    pub fn run(&mut self) -> std::io::Result<()> {
        let mut reader = BufReader::new(self.stream.try_clone()?);
        self.stream
            .write_all(b"welcome to analog3 mission control\r\n")?;

        loop {
            self.stream.write_all(b"analog3> ")?;
            let mut line = String::new();
            match reader.read_line(&mut line)? {
                0 => {
                    log::debug!("Connection closed");
                    return Ok(());
                }
                _ => {
                    let trimmed = line.trim().to_string();
                    log::debug!("Received: {}", trimmed);
                    let tokens: Vec<String> = trimmed.split(" ").map(str::to_string).collect();
                    if tokens.is_empty() {
                        // do nothing
                        continue;
                    }
                    let command = tokens[0].trim();
                    match command {
                        "hello" => {
                            self.stream.write_all(b"hi\r\n")?;
                        }
                        "ping" => {
                            self.ping(&command, &tokens)?;
                        }
                        "cancel-uid" => {
                            self.cancel_uid(&command, &tokens)?;
                        }
                        "pretend-sign-in" => {
                            self.pretend_sign_in(&command, &tokens)?;
                        }
                        "quit" => {
                            self.stream.write_all(b"bye!\r\n")?;
                            return Ok(());
                        }
                        "" => {
                            // do nothing
                        }
                        _ => {
                            let request = Request {
                                client_id: self.client_id,
                                command: trimmed,
                                params: vec![],
                            };
                            self.handle_command(request, true)?;
                        }
                    }
                }
            }
        }
    }

    fn parse_u8(&self, src: &str) -> Result<u8, ParseIntError> {
        if src.starts_with("0x") {
            return u8::from_str_radix(src.trim_start_matches("0x"), 16);
        }
        return u8::from_str_radix(src, 10);
    }

    fn parse_32(&self, src: &str) -> Result<u32, ParseIntError> {
        if src.starts_with("0x") {
            return u32::from_str_radix(src.trim_start_matches("0x"), 16);
        }
        return u32::from_str_radix(src, 10);
    }

    fn ping(&mut self, command: &str, tokens: &Vec<String>) -> std::io::Result<()> {
        if tokens.len() < 2 {
            self.stream.write_all(b"Usage: ping <id>\r\n")?;
            return Ok(());
        }
        let Ok(module_id) = self.parse_u8(&tokens[1]) else {
            self.stream.write_all(b"Invalid module id\r\n")?;
            return Ok(());
        };
        let request = Request {
            client_id: self.client_id,
            command: command.to_string(),
            params: vec![Param::U8(module_id)],
        };
        self.handle_command(request, false)?;
        self.stream
            .write_all(format!("sent to ID 0x{module_id:02x} ...").as_bytes())?;
        self.handle_result(false)?;
        return Ok(());
    }

    fn cancel_uid(&mut self, command: &str, tokens: &Vec<String>) -> std::io::Result<()> {
        if tokens.len() < 2 {
            self.stream.write_all(b"Usage: cancel-uid <uid>\r\n")?;
            return Ok(());
        }
        let Ok(module_uid) = self.parse_32(&tokens[1]) else {
            self.stream.write_all(b"Invalid module uid\r\n")?;
            return Ok(());
        };
        let request = Request {
            client_id: self.client_id,
            command: command.to_string(),
            params: vec![Param::U32(module_uid)],
        };
        return self.handle_command(request, true);
    }

    /// Simulates an individual module sign in.
    ///
    /// # Arguments
    ///
    /// - `command` (`&str`) - Command name.
    /// - `tokens` (`&Vec<String>`) - Source command parameter strings.
    ///
    /// # Returns
    ///
    /// - `std::io::Result<()>` - Success or ModuleManagementError.
    fn pretend_sign_in(&mut self, command: &str, tokens: &Vec<String>) -> std::io::Result<()> {
        if tokens.len() < 2 {
            self.stream.write_all(b"Usage: pretend-sign-in <uid>\r\n")?;
            return Ok(());
        }
        let Ok(module_uid) = self.parse_32(&tokens[1]) else {
            self.stream.write_all(b"Invalid module uid\r\n")?;
            return Ok(());
        };
        let request = Request {
            client_id: self.client_id,
            command: command.to_string(),
            params: vec![Param::U32(module_uid)],
        };
        return self.handle_command(request, true);
    }

    fn handle_command(
        &mut self,
        request: Request,
        continue_on_empty_reply: bool,
    ) -> std::io::Result<()> {
        self.request_sender.send(request).unwrap();
        self.notifier.send(EventType::RequestSent).unwrap();
        return self.handle_result(continue_on_empty_reply);
    }

    fn handle_result(&mut self, continue_on_empty_reply: bool) -> std::io::Result<()> {
        let recv_result = self.result_receiver.recv_timeout(Duration::from_secs(10));
        return match recv_result {
            Ok(command_result) => self.handle_result_inner(command_result, continue_on_empty_reply),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                self.stream.write_all(b" timeout\r\n")?;
                return Ok(());
            }
            Err(e) => {
                log::error!("Command execution error: {e:?}");
                self.stream.write_all(b"\r\nINTERNAL ERROR!\r\n")?;
                return Ok(());
            }
        };
    }

    fn handle_result_inner(
        &mut self,
        command_result: CommandResult,
        continue_on_empty_reply: bool,
    ) -> std::io::Result<()> {
        match command_result {
            Ok(reply_or_none) => match reply_or_none {
                Some(reply) => self.stream.write_all(reply.as_bytes())?,
                None => {
                    if continue_on_empty_reply {
                        // the reply will come later
                        self.handle_result(false)?;
                    }
                }
            },
            Err(e) => match e.error_type {
                ErrorType::UserCommandUnknown => self.stream.write_all(e.message.as_bytes())?,
                _ => {
                    log::error!("Command execution error: {e:?}");
                    self.stream
                        .write_all(b"An internal error encountered. Check the log.\r\n")?;
                }
            },
        };
        return Ok(());
    }
}

pub fn start_command_processor(
    request_sender: Sender<Request>,
    result_senders: &Arc<DashMap<u32, Sender<CommandResult>>>,
    notifier: Sender<EventType>,
) {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap(); // TODO: Handle error more gracefully
    log::info!("Listening on port 7878");

    let clone_result_senders = Arc::clone(&result_senders);
    thread::spawn(move || {
        let mut next_client_id = 0;
        loop {
            let client_id = next_client_id;
            next_client_id += 1;
            match listener.accept() {
                Ok((stream, _)) => {
                    let notifier_clone = notifier.clone();
                    let request_sender_clone = request_sender.clone();
                    let (result_sender, result_receiver) = channel();
                    clone_result_senders.insert(client_id, result_sender);
                    let clone_clone_result_senders = Arc::clone(&clone_result_senders);
                    thread::spawn(move || {
                        let mut client_handler = ClientHandler {
                            client_id: client_id,
                            stream: stream,
                            notifier: notifier_clone,
                            request_sender: request_sender_clone,
                            result_receiver,
                        };
                        if let Err(e) = client_handler.run() {
                            log::error!("Channel error: {e:?}");
                        }
                        clone_clone_result_senders.remove(&client_id);
                    });
                }
                Err(e) => log::error!("couldn't get client: {e:?}"),
            }
        }
    });
}
