use crate::{
    module_manager::ModuleManager,
    operation::{Command, OperationResult, Request, Response},
};
use tokio::sync::mpsc::Sender;

pub struct CommandHandler {}

impl CommandHandler {
    pub fn new() -> Self {
        Self {}
    }

    pub fn handle(&mut self, command: Command) {
        tokio::spawn(async move {
            match command {
                Command::Hi { resp } => {
                    resp.send("hello\r\n".to_string()).unwrap();
                }
                _ => {
                    log::error!("Operation not implemented: {:?}", command);
                }
            }
        });
    }
}
