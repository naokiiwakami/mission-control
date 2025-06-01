pub mod analog3;
pub mod can_controller;
pub mod command_processor;
pub mod event_type;
pub mod module_manager;
pub mod operation;

use crate::can_controller::CanController;
use crate::command_processor::start_command_processor;
use crate::event_type::EventType;
use crate::module_manager::{ErrorType, ModuleManager};
use crate::operation::{OperationResult, Request};
use dashmap::DashMap;
use env_logger::Env;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    log::info!("Analog3 mission control started");
    let (event_notifier, event_notif_receiver) = std::sync::mpsc::channel();
    let can_controller = CanController::new(event_notifier.clone());
    let mut module_manager = ModuleManager::new(&can_controller).unwrap();

    let (request_sender, request_receiver) = channel::<Request>();
    let result_senders: Arc<DashMap<u32, Sender<OperationResult>>> = Arc::new(DashMap::new());

    start_command_processor(
        request_sender.clone(),
        &result_senders,
        event_notifier.clone(),
    );

    // The event loop
    loop {
        let event_type = event_notif_receiver.recv().unwrap();
        match event_type {
            EventType::MessageRx => {
                if let Some(message) = can_controller.get_message() {
                    if let Err(e) = module_manager.handle_message(message) {
                        match e.error_type {
                            ErrorType::A3OpCodeUnknown => {
                                log::warn!("Can message handling failed; {e}");
                            }
                            _ => {
                                log::error!("Can message handling error; {e}");
                            }
                        }
                    }
                }
            }
            EventType::MessageTx => can_controller.send_message(),
            EventType::RequestSent => {
                let request: Request = request_receiver.recv().unwrap();
                match result_senders.get(&request.client_id) {
                    Some(result_sender) => {
                        match module_manager.user_request(&request, result_sender.clone()) {
                            Ok(response) => result_sender.send(Ok(response)).unwrap(),
                            Err(e) => result_sender.send(Err(e)).unwrap(),
                        }
                    }
                    None => {
                        log::error!("RequestSent: unknown client_id: {}", request.client_id);
                    }
                }
            }
            _ => log::warn!("Unknown event: {:?}", event_type),
        }
    }
}
