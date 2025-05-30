use dashmap::DashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use crate::event_type::EventType;
use crate::user_request::Request;

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
        stream.write_all(b"analog3> ").unwrap();
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
                    "quit" => {
                        stream.write_all(b"bye!\r\n").unwrap();
                        break;
                    }
                    _ => {
                        let request = Request {
                            id: id,
                            command: trimmed,
                        };
                        request_sender.send(request).unwrap();
                        notifier.send(EventType::RequestSent).unwrap();
                        let reply = reply_receiver.recv().unwrap();
                        stream.write_all(reply.as_bytes()).unwrap();
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

pub fn start_command_processor(
    request_sender: Sender<Request>,
    reply_senders: &Arc<DashMap<u32, Sender<String>>>,
    notifier: Sender<EventType>,
) {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
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
                    let (reply_sender, reply_receiver) = channel();
                    clone_senders.insert(client_id, reply_sender);
                    let clone_clone_senders = Arc::clone(&clone_senders);
                    thread::spawn(move || {
                        handle_client(
                            client_id,
                            stream,
                            notifier_clone,
                            request_sender_clone,
                            reply_receiver,
                        );
                        clone_clone_senders.remove(&client_id);
                    });
                }
                Err(e) => log::error!("couldn't get client: {e:?}"),
            }
        }
    });
}
