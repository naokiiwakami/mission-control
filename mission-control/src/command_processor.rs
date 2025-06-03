use crate::analog3::{ChunkBuilder, Value};
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
                        "get-name" => self.get_name(&command, &tokens)?,
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
            operation,
            params,
        };

        // send the request
        self.request_sender.send(request).unwrap();
        self.notifier.send(EventType::RequestSent).unwrap();

        return self.handle_result(0);
    }

    fn get_name(&mut self, command: &str, tokens: &Vec<String>) -> std::io::Result<()> {
        // build the request
        let mut params = Vec::new();
        let specs = vec![Spec::u8("id", true)];
        for (i, spec) in specs.iter().enumerate() {
            if tokens.len() <= i + 1 {
                if spec.required {
                    self.usage(command, &specs)?;
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

        let Value::U8(remote_id) = params[0] else {
            self.stream
                .write_all("something went wrong, remote ID not found in params\r\n".as_bytes())?;
            return Ok(());
        };

        let request = Request {
            client_id: self.client_id,
            operation: Operation::GetName,
            params,
        };

        // send the request
        self.request_sender.send(request).unwrap();
        self.notifier.send(EventType::RequestSent).unwrap();

        let mut chunk_builder = ChunkBuilder::for_single_field();
        return self.handle_result_for_get_name(remote_id, &mut chunk_builder);
    }

    fn handle_result_for_get_name(
        &mut self,
        remote_id: u8,
        chunk_builder: &mut ChunkBuilder,
    ) -> std::io::Result<()> {
        let recv_result = self.result_receiver.recv_timeout(Duration::from_secs(10));
        return match recv_result {
            Ok(operation_result) => {
                self.handle_result_inner_for_get_name(remote_id, operation_result, chunk_builder)
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                self.stream.write_all(b" timeout\r\n")?;
                if remote_id > 0 {
                    let request = Request {
                        client_id: self.client_id,
                        operation: Operation::AckName,
                        params: vec![Value::U8(remote_id), Value::Bool(true)],
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

    fn handle_result_inner_for_get_name(
        &mut self,
        remote_id: u8,
        operation_result: OperationResult,
        chunk_builder: &mut ChunkBuilder,
    ) -> std::io::Result<()> {
        match operation_result {
            Ok(response) => {
                let reply = &response.reply;
                let size = reply.len();
                let mut data_dump = String::new();
                for i in 0..size {
                    data_dump += format!(" {:02x}", reply.as_slice()[i]).as_str();
                }
                log::debug!("processor receives:{}", data_dump);

                match chunk_builder.data(&reply.as_slice()[1..size], size - 1) {
                    Ok(is_done) => {
                        let params = vec![Value::U8(remote_id), Value::Bool(is_done)];
                        let request = Request {
                            client_id: self.client_id,
                            operation: Operation::AckName,
                            params,
                        };

                        let mut force_stop = false;
                        if size < 2 {
                            log::error!("Empty data came");
                            force_stop = true;
                        }

                        if is_done || force_stop {
                            let chunk = chunk_builder.build().unwrap();
                            let name = chunk[0].get_value_as_string().unwrap();
                            let reply = format!(" ok\r\nname = {}\r\n", name);
                            self.stream.write_all(reply.as_bytes())?;
                            self.request_sender.send(request).unwrap();
                            self.notifier.send(EventType::RequestSent).unwrap();
                            return Ok(());
                        }

                        self.request_sender.send(request).unwrap();
                        self.notifier.send(EventType::RequestSent).unwrap();
                        return self.handle_result_for_get_name(remote_id, chunk_builder);
                    }
                    Err(e) => {
                        self.stream
                            .write_all(format!("{:?}\r\n", e.message).as_bytes())?;
                        return Ok(());
                    }
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
                self.stream.write_all(response.reply.as_slice())?;
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
