pub mod analog3;
pub mod boundary;
pub mod event_type;
pub mod module_manager;

use env_logger::Env;

use boundary::Boundary;
use module_manager::ModuleManager;

use crate::event_type::EventType;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    log::info!("Analog3 mission control started");
    let boundary = Boundary::new();
    let message_handler = ModuleManager::new(&boundary);
    loop {
        let event_type = boundary.wait_for_event();
        match event_type {
            EventType::MessageReceived => {
                if let Some(message) = boundary.get_message() {
                    message_handler.handle_message(message);
                }
            }
            _ => log::warn!("Unknown event: {:?}", event_type),
        }
    }
}
