pub mod analog3;
pub mod boundary;
pub mod event_type;
pub mod module_manager;

use dashmap::DashMap;
use env_logger::Env;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use boundary::{Boundary, CanController};
use module_manager::ModuleManager;

use crate::event_type::EventType;

struct Request {
    id: u32,
    command: String,
}

fn handle_client(
    id: u32,
    mut stream: TcpStream,
    notifier: Sender<EventType>,
    request_sender: Sender<Request>,
    reply_receiver: Receiver<String>,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    stream
        .write_all(b"welcome to analog3 mission control\r\n")
        .unwrap();

    loop {
        stream.write_all(b"a3> ").unwrap();
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                log::debug!("Connection closed");
                break;
            }
            Ok(_) => {
                let trimmed = line.trim().to_string();
                log::debug!("Received: {}", trimmed);
                match trimmed.as_str() {
                    "hello" => {
                        stream.write_all(b"hi\r\n").unwrap();
                    }
                    "ping" => {
                        let request = Request {
                            id: id,
                            command: "ping".to_string(),
                        };
                        request_sender.send(request).unwrap();
                        notifier.send(EventType::RequestSent).unwrap();
                        let reply = reply_receiver.recv().unwrap();
                        stream.write_all(reply.as_bytes()).unwrap();
                    }
                    "quit" => {
                        stream.write_all(b"bye!\r\n").unwrap();
                        break;
                    }
                    _ => {
                        stream.write_all(b"command not recognized\r\n").unwrap();
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to read from socket: {e:?}");
                break;
            }
        }
    }
}

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    log::info!("Analog3 mission control started");
    let (notifier, receiver) = std::sync::mpsc::channel();
    let mut boundary = Boundary::new(notifier.clone());
    let can_controller = CanController::new(&mut boundary, notifier.clone());
    let module_manager = ModuleManager::new(&can_controller);

    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    log::info!("Listening on port 7878");

    let (request_sender, request_receiver) = channel::<Request>();
    let reply_senders: Arc<DashMap<u32, Sender<String>>> = Arc::new(DashMap::new());
    let clone_senders = Arc::clone(&reply_senders);
    thread::spawn(move || {
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    let notifier_clone = notifier.clone();
                    let request_sender_clone = request_sender.clone();
                    let (reply_sender, reply_receiver) = channel();
                    clone_senders.insert(0, reply_sender);
                    let clone_clone_senders = Arc::clone(&clone_senders);
                    thread::spawn(move || {
                        handle_client(
                            0,
                            stream,
                            notifier_clone,
                            request_sender_clone,
                            reply_receiver,
                        );
                        clone_clone_senders.remove(&0);
                    });
                }
                Err(e) => log::error!("couldn't get client: {e:?}"),
            }
        }
    });

    // The event loop
    loop {
        let event_type = receiver.recv().unwrap();
        match event_type {
            EventType::MessageRx => {
                if let Some(message) = can_controller.get_message() {
                    module_manager.handle_message(message);
                }
            }
            EventType::MessageTx => can_controller.send_message(),
            EventType::RequestSent => {
                let request: Request = request_receiver.recv().unwrap();
                match reply_senders.get(&request.id) {
                    Some(sender) => {
                        let reply: String = format!("{} received\r\n", request.command);
                        sender.send(reply).unwrap();
                    }
                    None => {}
                }
            }
            _ => log::warn!("Unknown event: {:?}", event_type),
        }
    }
}
