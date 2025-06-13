pub mod a3_message;
pub mod a3_modules;
pub mod analog3;
pub mod can_controller;
pub mod command;
pub mod error;
pub mod mission_control;
pub mod user_session;

use env_logger::Env;

use crate::mission_control::MissionControl;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    log::info!("Analog3 mission control started");

    // A3 Modules
    let (modules_tx, _modules_handle) = a3_modules::start();

    // CAN controller
    let (can_tx, mut can_rx, _can_tx_handle) = can_controller::start();

    // Mission control
    let mut mission_control = MissionControl::new(can_tx.clone(), modules_tx);

    // User sessions
    let (mut command_rx, _command_handle) = user_session::start();

    a3_message::sign_in(can_tx.clone()).await;

    loop {
        tokio::select! {
        Some(can_message) = can_rx.recv() => {
            mission_control.handle_can_message(can_message);
        }
        Some(user_command) = command_rx.recv() => {
            mission_control.handle_command(user_command);
        }
        }
    }
}
