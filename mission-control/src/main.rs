pub mod analog3;
pub mod boundary;
pub mod event_type;
pub mod module_manager;

use env_logger::Env;

use boundary::Boundary;
use module_manager::ModuleManager;

use crate::event_type::EventType;

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    log::info!("Analog3 mission control started");
    let boundary = Boundary::new();
    let message_handler = ModuleManager::new(&boundary);

    // let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    // println!("Listening on port 7878");

    // The event loop
    loop {
        let event_type = boundary.wait_for_event();
        match event_type {
            EventType::MessageRx => {
                if let Some(message) = boundary.get_message() {
                    message_handler.handle_message(message);
                }
            }
            EventType::MessageTx => boundary.send_message(),
            _ => log::warn!("Unknown event: {:?}", event_type),
        }
    }
}
