pub mod analog3;
pub mod boundary;
pub mod event_type;
pub mod module_manager;

use env_logger::Env;

use boundary::{Boundary, CanController};
use module_manager::ModuleManager;

use crate::event_type::EventType;

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::AsRawFd;
use std::thread;

fn handle_client(mut stream: TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    stream
        .write_all(b"welcome to analog3 mission control\r\n")
        .unwrap();

    loop {
        stream.write_all(b"a3> ").unwrap();
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // connection closed
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
                        stream.write_all(b"command not recognized\r\n").unwrap();
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to read from socket: {}", e);
                break;
            }
        }
    }
}

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    log::info!("Analog3 mission control started");
    let mut boundary = Boundary::new();
    let can_controller = CanController::new(&mut boundary);
    let message_handler = ModuleManager::new(&can_controller);

    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    boundary.add_event_type(listener.as_raw_fd(), EventType::UserConnection);
    println!("Listening on port 7878");

    // The event loop
    loop {
        let event_type = boundary.wait_for_event();
        match event_type {
            EventType::MessageRx => {
                if let Some(message) = can_controller.get_message() {
                    message_handler.handle_message(message);
                }
            }
            EventType::MessageTx => can_controller.send_message(),
            EventType::UserConnection => match listener.accept() {
                Ok((stream, _)) => {
                    thread::spawn(|| handle_client(stream));
                }
                Err(e) => log::error!("couldn't get client: {e:?}"),
            },
            _ => log::warn!("Unknown event: {:?}", event_type),
        }
    }
}
