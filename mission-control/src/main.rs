pub mod analog3;
pub mod can_controller;
pub mod module_manager;
pub mod queue;

use env_logger::Env;

use can_controller::CanController;
use module_manager::ModuleManager;

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    log::info!("Analog3 mission control started");
    let can_controller = CanController::new();
    let message_handler = ModuleManager::new(&can_controller);
    loop {
        if let Some(message) = can_controller.get_message() {
            message_handler.handle_message(message);
        }
    }
}
