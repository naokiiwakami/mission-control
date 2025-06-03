use crate::analog3::Value;
use crate::event_type::EventType;
use crate::module_manager::ErrorType;
use crate::operation::{Operation, OperationResult, Request};
use dashmap::DashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

struct ParseParamError {}

struct Spec {
    name: String,
    required: bool,
    parse: fn(&String) -> Result<Value, ParseParamError>,
}

impl Spec {
    pub fn u8(name: &str, required: bool) -> Self {
        Self {
            name: name.to_string(),
            required: required,
            parse: |src| {
                let trimmed = src.trim();
                let parse_u8 = || {
                    if trimmed.starts_with("0x") {
                        u8::from_str_radix(trimmed.trim_start_matches("0x"), 16)
                    } else {
                        u8::from_str_radix(trimmed, 10)
                    }
                };
                return match parse_u8() {
                    Ok(value) => Ok(Value::U8(value)),
                    Err(_) => Err(ParseParamError {}),
                };
            },
        }
    }

    #[allow(dead_code)]
    pub fn u16(name: &str, required: bool) -> Self {
        Self {
            name: name.to_string(),
            required: required,
            parse: |src| {
                let trimmed = src.trim();
                let parse_16 = || {
                    if trimmed.starts_with("0x") {
                        u16::from_str_radix(trimmed.trim_start_matches("0x"), 16)
                    } else {
                        u16::from_str_radix(trimmed, 10)
                    }
                };
                return match parse_16() {
                    Ok(value) => Ok(Value::U16(value)),
                    Err(_) => Err(ParseParamError {}),
                };
            },
        }
    }

    pub fn u32(name: &str, required: bool) -> Self {
        Self {
            name: name.to_string(),
            required: required,
            parse: |src| {
                let trimmed = src.trim();
                let parse_16 = || {
                    if trimmed.starts_with("0x") {
                        u32::from_str_radix(trimmed.trim_start_matches("0x"), 16)
                    } else {
                        u32::from_str_radix(trimmed, 10)
                    }
                };
                return match parse_16() {
                    Ok(value) => Ok(Value::U32(value)),
                    Err(_) => Err(ParseParamError {}),
                };
            },
        }
    }

    #[allow(dead_code)]
    pub fn str(name: String, required: bool) -> Self {
        Self {
            name: name,
            required: required,
            parse: |src| Ok(Value::Text(src.trim().to_string())),
        }
    }

    pub fn bool(name: &str, required: bool) -> Self {
        Self {
            name: name.to_string(),
            required: required,
            parse: |src| {
                return match src.trim().parse() {
                    Ok(value) => Ok(Value::Bool(value)),
                    Err(_) => Err(ParseParamError {}),
                };
            },
        }
    }
}

struct ClientHandler {
    client_id: u32,
    stream: TcpStream,
    notifier: Sender<EventType>,
    request_sender: Sender<Request>,
    result_receiver: Receiver<OperationResult>,
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
                        "list" => self.process(&command, Operation::List, &tokens, &vec![])?,
                        "ping" => self.process(
                            &command,
                            Operation::Ping,
                            &tokens,
                            &vec![Spec::u8("id", true), Spec::bool("visual", false)],
                        )?,
                        "get-name" => self.process(
                            &command,
                            Operation::GetName,
                            &tokens,
                            &vec![Spec::u8("id", true)],
                        )?,
                        "cancel-uid" => self.process(
                            &command,
                            Operation::RequestUidCancel,
                            &tokens,
                            &vec![Spec::u32("uid", true)],
                        )?,
                        "pretend-sign-in" => self.process(
                            &command,
                            Operation::PretendSignIn,
                            &tokens,
                            &vec![Spec::u32("uid", true)],
                        )?,
                        "pretend-notify-id" => self.process(
                            &command,
                            Operation::PretendNotifyId,
                            &tokens,
                            &vec![Spec::u32("uid", true), Spec::u8("id", true)],
                        )?,
                        "quit" => {
                            self.stream.write_all(b"bye!\r\n")?;
                            return Ok(());
                        }
                        "" => {
                            // do nothing
                        }
                        _ => {
                            self.stream.write_all(
                                format!("{}: Unknown command\r\n", command).as_bytes(),
                            )?;
                        }
                    }
                }
            }
        }
    }

    fn usage(&mut self, command: &str, specs: &Vec<Spec>) -> std::io::Result<()> {
        let mut out = String::new();
        out += format!("Usage {}", command).as_str();
        for spec in specs {
            if spec.required {
                out += format!(" <{}>", spec.name).as_str();
            } else {
                out += format!(" [{}]", spec.name).as_str();
            }
        }
        out += "\r\n";
        self.stream.write_all(out.as_bytes())?;
        return Ok(());
    }

    fn process(
        &mut self,
        command: &str,
        operation: Operation,
        tokens: &Vec<String>,
        specs: &Vec<Spec>,
    ) -> std::io::Result<()> {
        // build the request
        let mut params = Vec::new();
        for (i, spec) in specs.iter().enumerate() {
            if tokens.len() <= i + 1 {
                if spec.required {
                    self.usage(command, specs)?;
                    return Ok(());
                }
                break;
            }
            if let Ok(param) = (spec.parse)(&tokens[i + 1]) {
                params.push(param);
            } else {
                self.stream
                    .write_all(format!("Invalid {}\r\n", spec.name).as_bytes())?;
                return Ok(());
            }
        }
        let request = Request {
            client_id: self.client_id,
            operation: operation,
            params: params,
        };

        // send the request
        self.request_sender.send(request).unwrap();
        self.notifier.send(EventType::RequestSent).unwrap();

        return self.handle_result(0);
    }

    fn handle_result(&mut self, stream_id: u8) -> std::io::Result<()> {
        let recv_result = self.result_receiver.recv_timeout(Duration::from_secs(10));
        return match recv_result {
            Ok(operation_result) => self.handle_result_inner(operation_result),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                self.stream.write_all(b" timeout\r\n")?;
                if stream_id > 0 {
                    let request = Request {
                        client_id: self.client_id,
                        operation: Operation::Cancel,
                        params: vec![Value::U8(stream_id)],
                    };
                    self.request_sender.send(request).unwrap();
                    self.notifier.send(EventType::RequestSent).unwrap();
                }
                return Ok(());
            }
            Err(e) => {
                log::error!("Command execution error: {e:?}");
                self.stream.write_all(b"\r\nINTERNAL ERROR!\r\n")?;
                return Ok(());
            }
        };
    }

    fn handle_result_inner(&mut self, operation_result: OperationResult) -> std::io::Result<()> {
        match operation_result {
            Ok(response) => {
                self.stream.write_all(response.reply.as_bytes())?;
                if response.more {
                    self.handle_result(response.stream_id)?;
                }
            }
            Err(e) => match e.error_type {
                ErrorType::RuntimeError => {
                    log::error!("Command execution error: {e:?}");
                    self.stream
                        .write_all(b"An internal error encountered. Check the log.\r\n")?;
                }
                _ => self
                    .stream
                    .write_all(format!("{}\r\n", e.message).as_bytes())?,
            },
        };
        return Ok(());
    }
}

pub fn start_command_processor(
    request_sender: Sender<Request>,
    result_senders: &Arc<DashMap<u32, Sender<OperationResult>>>,
    notifier: Sender<EventType>,
) {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap(); // TODO: Handle error more gracefully
    log::info!("Listening on port 7878");

    let clone_result_senders = Arc::clone(&result_senders);
    thread::spawn(move || {
        let mut next_client_id = 1;
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
                            client_id,
                            stream,
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
