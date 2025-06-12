pub mod analog3;
pub mod can_controller;
pub mod command_processor;
pub mod error;
pub mod operation;

use crate::operation::OperationResult;
use dashmap::DashMap;
use env_logger::Env;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    log::info!("Analog3 mission control started");

    // CAN controller
    let (can_tx, mut can_rx, _can_tx_handle) = can_controller::start();

    // Command processor
    let sessions = Arc::new(DashMap::<u32, Sender<OperationResult>>::new());
    let (mut command_rx, _command_handle) = command_processor::start(sessions.clone());

    println!("waiting for a message ...");

    let handle = tokio::spawn(async move {
        loop {
            if let Some(message) = can_rx.recv().await {
                println!(" received! {:08x}", message.id());
            }
        }
    });

    handle.await.unwrap();
}
