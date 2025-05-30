use crate::event_type::EventType;
use crate::module_manager::{ErrorType, ModuleManagementError};
use dashmap::DashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

pub type CommandResult = Result<Option<String>, ModuleManagementError>;

#[derive(Debug)]
pub enum Param {
    U8(u8),
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

fn handle_result_inner(
    stream: &mut TcpStream,
    result_receiver: &Receiver<CommandResult>,
    command_result: CommandResult,
    continue_on_empty_reply: bool,
) -> std::io::Result<()> {
    match command_result {
        Ok(reply_or_none) => match reply_or_none {
            Some(reply) => stream.write_all(reply.as_bytes())?,
            None => {
                if continue_on_empty_reply {
                    // the reply will come later
                    handle_result(stream, result_receiver, false)?;
                }
            }
        },
        Err(e) => match e.error_type {
            ErrorType::UserCommandUnknown => stream.write_all(e.message.as_bytes())?,
            _ => {
                log::error!("Command execution error: {e:?}");
                stream.write_all(b"An internal error encountered. Check the log.\r\n")?;
            }
        },
    };
    return Ok(());
}

fn handle_result(
    stream: &mut TcpStream,
    result_receiver: &Receiver<CommandResult>,
    continue_on_empty_reply: bool,
) -> std::io::Result<()> {
    let recv_result = result_receiver.recv_timeout(Duration::from_secs(10));
    return match recv_result {
        Ok(command_result) => handle_result_inner(
            stream,
            result_receiver,
            command_result,
            continue_on_empty_reply,
        ),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            stream.write_all(b" timeout\r\n")?;
            return Ok(());
        }
        Err(e) => {
            log::error!("Command execution error: {e:?}");
            stream.write_all(b"\r\nINTERNAL ERROR!\r\n")?;
            return Ok(());
        }
    };
}

fn handle_command(
    request: Request,
    stream: &mut TcpStream,
    notifier: &Sender<EventType>,
    request_sender: &Sender<Request>,
    result_receiver: &Receiver<CommandResult>,
    continue_on_empty_reply: bool,
) -> std::io::Result<()> {
    request_sender.send(request).unwrap();
    notifier.send(EventType::RequestSent).unwrap();
    return handle_result(stream, result_receiver, continue_on_empty_reply);
}

fn handle_client(
    id: u32,
    mut stream: TcpStream,
    notifier: Sender<EventType>,
    request_sender: Sender<Request>,
    result_receiver: Receiver<CommandResult>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    stream.write_all(b"welcome to analog3 mission control\r\n")?;

    loop {
        stream.write_all(b"analog3> ")?;
        let mut line = String::new();
        match reader.read_line(&mut line)? {
            0 => {
                log::debug!("Connection closed");
                return Ok(());
            }
            _ => {
                let trimmed = line.trim().to_string();
                log::debug!("Received: {}", trimmed);
                let mut tokens = trimmed.split(" ");
                let first_item = tokens.next();
                if first_item == None {
                    // do nothing
                    continue;
                }
                let command = first_item.unwrap();
                match command {
                    "hello" => {
                        stream.write_all(b"hi\r\n")?;
                    }
                    "ping" => {
                        let module_id = 0x02u8;
                        let request = Request {
                            client_id: id,
                            command: trimmed,
                            params: vec![Param::U8(module_id)],
                        };
                        handle_command(
                            request,
                            &mut stream,
                            &notifier,
                            &request_sender,
                            &result_receiver,
                            false,
                        )?;
                        stream.write_all(format!("sent to ID 0x{module_id:02x} ...").as_bytes())?;
                        handle_result(&mut stream, &result_receiver, false)?;
                    }
                    "quit" => {
                        stream.write_all(b"bye!\r\n")?;
                        return Ok(());
                    }
                    "" => {
                        // do nothing
                    }
                    _ => {
                        let request = Request {
                            client_id: id,
                            command: trimmed,
                            params: vec![],
                        };
                        handle_command(
                            request,
                            &mut stream,
                            &notifier,
                            &request_sender,
                            &result_receiver,
                            true,
                        )?;
                    }
                }
            }
        }
    }
}

pub fn start_command_processor(
    request_sender: Sender<Request>,
    reply_senders: &Arc<DashMap<u32, Sender<CommandResult>>>,
    notifier: Sender<EventType>,
) {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap(); // TODO: Handle error more gracefully
    log::info!("Listening on port 7878");

    let clone_senders = Arc::clone(&reply_senders);
    thread::spawn(move || {
        let mut next_client_id = 0;
        loop {
            let client_id = next_client_id;
            next_client_id += 1;
            match listener.accept() {
                Ok((stream, _)) => {
                    let notifier_clone = notifier.clone();
                    let request_sender_clone = request_sender.clone();
                    let (reply_sender, response_receiver) = channel();
                    clone_senders.insert(client_id, reply_sender);
                    let clone_clone_senders = Arc::clone(&clone_senders);
                    thread::spawn(move || {
                        if let Err(e) = handle_client(
                            client_id,
                            stream,
                            notifier_clone,
                            request_sender_clone,
                            response_receiver,
                        ) {
                            log::error!("Channel error: {e:?}");
                        }
                        clone_clone_senders.remove(&client_id);
                    });
                }
                Err(e) => log::error!("couldn't get client: {e:?}"),
            }
        }
    });
}
