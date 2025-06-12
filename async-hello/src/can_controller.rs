#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use std::sync::{LazyLock, Mutex};
use tokio::{
    sync::mpsc::{Receiver, Sender, channel},
    task::JoinHandle,
};

pub struct CanMessage {
    pub message: *mut can_message_t,
    message_attached: bool,
}

impl CanMessage {
    pub fn new() -> Self {
        unsafe {
            let message = can_create_message();
            return Self {
                message,
                message_attached: false,
            };
        }
    }

    pub fn from_raw_message(message: *mut can_message_t) -> Self {
        return Self {
            message: message,
            message_attached: true,
        };
    }

    pub fn id(&self) -> u32 {
        unsafe {
            return (*self.message).id;
        }
    }

    pub fn set_id(&mut self, id: u32) {
        unsafe {
            (*self.message).id = id;
        }
    }

    pub fn is_extended(&self) -> bool {
        unsafe {
            return (*self.message).is_extended != 0;
        }
    }

    pub fn set_extended(&mut self, is_extended: bool) {
        unsafe {
            (*self.message).is_extended = is_extended as u8;
        }
    }

    pub fn is_remote(&self) -> bool {
        unsafe {
            return (*self.message).is_remote != 0;
        }
    }

    pub fn set_remote(&mut self, is_remote: bool) {
        unsafe {
            (*self.message).is_remote = is_remote as u8;
        }
    }

    pub fn data_length(&self) -> u8 {
        unsafe {
            return (*self.message).data_length;
        }
    }

    pub fn set_data_length(&mut self, length: u8) {
        unsafe {
            (*self.message).data_length = length;
        }
    }

    pub fn get_data(&self, index: usize) -> u8 {
        unsafe {
            return (*self.message).data[index];
        }
    }

    pub fn set_data(&mut self, index: usize, value: u8) {
        unsafe {
            (*self.message).data[index] = value;
        }
    }

    pub fn data(&self) -> [u8; 8usize] {
        unsafe {
            return (*self.message).data;
        }
    }

    /// Attach the inside message so that the internal message
    /// is freed on destruction.
    pub fn attach(&mut self) {
        self.message_attached = true;
    }
}

impl Drop for CanMessage {
    fn drop(&mut self) {
        unsafe {
            if self.message_attached {
                can_free_message(self.message);
            }
        }
    }
}

unsafe impl Sync for CanMessage {}
unsafe impl Send for CanMessage {}

struct FdHolder {
    rx_sender: Option<Sender<CanMessage>>,
}

static EVENT_FD_HOLDER: LazyLock<Mutex<FdHolder>> =
    LazyLock::new(|| Mutex::new(FdHolder { rx_sender: None }));

#[unsafe(no_mangle)]
pub extern "C" fn notify_message(message: *mut can_message_t) {
    let holder = EVENT_FD_HOLDER.lock().unwrap();
    if let Some(rx_sender) = &holder.rx_sender {
        if let Err(e) = rx_sender.try_send(CanMessage::from_raw_message(message)) {
            log::error!("Failed to put a new RX message to channel: {e:?}");
        }
    }
}

fn run_tx(mut tx_receiver: Receiver<CanMessage>) -> JoinHandle<()> {
    return tokio::spawn(async move {
        loop {
            if let Some(message) = tx_receiver.recv().await {
                unsafe {
                    can_send_message(message.message);
                    can_free_message(message.message);
                }
            }
        }
    });
}

pub fn start() -> (Sender<CanMessage>, Receiver<CanMessage>, JoinHandle<()>) {
    // Set up message rx
    let (rx_sender, rx_receiver) = channel(16);
    let mut holder = EVENT_FD_HOLDER.lock().unwrap();
    holder.rx_sender = Some(rx_sender);

    unsafe {
        can_set_rx_message_consumer(Some(notify_message));
        if can_init() != 0 {
            log::error!("Error encountered while initializing CAN controller");
            std::process::exit(1);
        }
    }

    // set up message tx
    let (tx_sender, tx_receiver) = channel(16);

    let handle = run_tx(tx_receiver);

    (tx_sender, rx_receiver, handle)
}
