pub mod analog3;
pub mod can_controller;
pub mod command_processor;
pub mod event_type;
pub mod module_manager;
pub mod user_request;

use dashmap::DashMap;
use env_logger::Env;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};

use crate::can_controller::CanController;
use crate::command_processor::start_command_processor;
use crate::event_type::EventType;
use crate::module_manager::ModuleManager;
use crate::user_request::Request;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    log::info!("Analog3 mission control started");
    let (event_notifier, event_notif_receiver) = std::sync::mpsc::channel();
    let can_controller = CanController::new(event_notifier.clone());
    let mut module_manager = ModuleManager::new(&can_controller);

    let (request_sender, request_receiver) = channel::<Request>();
    let reply_senders: Arc<DashMap<u32, Sender<String>>> = Arc::new(DashMap::new());

    start_command_processor(
        request_sender.clone(),
        &reply_senders,
        event_notifier.clone(),
    );

    // The event loop
    loop {
        let event_type = event_notif_receiver.recv().unwrap();
        match event_type {
            EventType::MessageRx => {
                if let Some(message) = can_controller.get_message() {
                    if let Some((reply, client_id)) = module_manager.handle_message(message) {
                        if let Some(sender) = reply_senders.get(&client_id) {
                            sender.send(reply).unwrap();
                        }
                    }
                }
            }
            EventType::MessageTx => can_controller.send_message(),
            EventType::RequestSent => {
                let request: Request = request_receiver.recv().unwrap();
                if let Some(sender) = reply_senders.get(&request.id) {
                    if let Some((reply, _)) =
                        module_manager.user_request(&request.command, request.id)
                    {
                        sender.send(reply).unwrap();
                    }
                }
            }
            _ => log::warn!("Unknown event: {:?}", event_type),
        }
    }
}
