pub mod analog3;
pub mod can_controller;
pub mod module_manager;
pub mod queue;

use can_controller::CanController;
use module_manager::ModuleManager;

fn main() {
    println!("Analog3 mission control started");
    let can_controller = CanController::new();
    let message_handler = ModuleManager::new(&can_controller);
    loop {
        if let Some(message) = can_controller.get_message() {
            message_handler.handle_message(message);
        }
    }
}
