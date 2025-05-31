#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{LazyLock, Mutex};

use crate::event_type::EventType;

pub struct CanMessage {
    pub message: *mut can_message_t,
    message_attached: bool,
}

impl CanMessage {
    pub fn new(message: *mut can_message_t) -> Self {
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

    /// Detach the inside message after its ownership is moved
    /// into the C can-controller library.
    pub fn detach(&mut self) {
        self.message_attached = false;
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
    rx_sender: Option<Sender<u64>>,
    notifier: Option<Sender<EventType>>,
}

static EVENT_FD_HOLDER: LazyLock<Mutex<FdHolder>> = LazyLock::new(|| {
    Mutex::new(FdHolder {
        rx_sender: None,
        notifier: None,
    })
});

#[unsafe(no_mangle)]
pub extern "C" fn notify_message(message: *mut can_message_t) {
    let holder = EVENT_FD_HOLDER.lock().unwrap();
    if let Some(notifier) = &holder.notifier {
        if let Some(rx_sender) = &holder.rx_sender {
            if let Err(error) = rx_sender.send(message as u64) {
            } else if let Err(error) = notifier.send(EventType::MessageRx) {
                log::error!("Notif error: {error:?}");
            }
        }
    }
}

pub struct CanController {
    rx_receiver: Receiver<u64>,
    tx_sender: Sender<CanMessage>,
    tx_receiver: Receiver<CanMessage>,
    tx_notif: Sender<EventType>,
}

impl CanController {
    pub fn new(notifier: Sender<EventType>) -> Self {
        unsafe {
            let (rx_sender, rx_receiver) = std::sync::mpsc::channel();

            let mut holder = EVENT_FD_HOLDER.lock().unwrap();
            holder.rx_sender = Some(rx_sender);
            holder.notifier = Some(notifier.clone());

            let (tx_sender, tx_receiver) = std::sync::mpsc::channel();

            // OK the CAN interface is ready to be initialized
            can_set_rx_message_consumer(Some(notify_message));
            can_init();

            return Self {
                rx_receiver: rx_receiver,
                tx_sender: tx_sender,
                tx_receiver: tx_receiver,
                tx_notif: notifier,
            };
        }
    }

    pub fn get_message(&self) -> Option<CanMessage> {
        match self.rx_receiver.recv() {
            Ok(data) => unsafe {
                let message: *mut can_message_t = std::mem::transmute(data);
                return Some(CanMessage::new(message));
            },
            Err(e) => {
                log::error!("Error in fetching incoming message from channel: {e:?}");
                return None;
            }
        }
    }

    pub fn create_message(&self) -> CanMessage {
        unsafe {
            let api_message = can_create_message();
            return CanMessage::new(api_message);
        }
    }

    /// Put a message to the TX pipe.
    ///
    /// The CAN interface is not thread safe. We just put the message
    /// into the TX pipe and let the main thread pick up and handle it
    /// in the event loop.
    ///
    /// # Arguments
    ///
    /// - `&self` (`Boundary`) - Myself.
    /// - `mut message` (`CanMessage`) - Message to send.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut message = boundary.create_message();
    ///
    /// // set message contents here
    ///
    /// boundary.put_message(message);
    /// ```
    pub fn put_message(&self, mut message: CanMessage) {
        // The CAN interface will take care of releasing the message.
        // We disconnect the object from the Rust ecosystem here.
        message.detach();
        if let Err(e) = self.tx_sender.send(message) {
            log::error!("Failed to put a tx message to pipe: {e:?}");
            return;
        }
        if let Err(e) = self.tx_notif.send(EventType::MessageTx) {
            log::error!("Failed to send MessageTx notif: {e:?}");
        }
    }

    /// Send a message in the TX pipe if any.
    ///
    /// This method is meant to be called in the main thread in the event loop.
    ///
    /// # Arguments
    ///
    /// - `&self` (`Boundary`) - Myself.
    pub fn send_message(&self) {
        match self.tx_receiver.recv() {
            Ok(message) => {
                // if let Some(message) = self.get_message_from_pipe(self.tx_fd) {
                unsafe {
                    can_send_message(message.message);
                }
                // }
            }
            Err(e) => log::error!("tx message error: {e:?}"),
        }
    }
}
