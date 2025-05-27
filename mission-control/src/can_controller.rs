#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use crate::queue::Queue;
use std::sync::{LazyLock, Mutex};

pub struct CanMessage {
    message: *mut can_message_t,
}

impl CanMessage {
    pub fn new(message: *mut can_message_t) -> Self {
        return Self { message: message };
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
        self.message = std::ptr::null_mut();
    }
}

impl Drop for CanMessage {
    fn drop(&mut self) {
        unsafe {
            if !self.message.is_null() {
                can_free_message(self.message);
            }
        }
    }
}

unsafe impl Sync for CanMessage {}
unsafe impl Send for CanMessage {}

static QUEUE: LazyLock<Mutex<Queue<CanMessage>>> = LazyLock::new(|| Mutex::new(Queue::new()));

#[unsafe(no_mangle)]
pub extern "C" fn notify_message(message: *mut can_message_t) {
    QUEUE.lock().unwrap().add(CanMessage::new(message));
}

pub struct CanController {}

// TODO: Make this singleton
impl CanController {
    pub fn new() -> Self {
        unsafe {
            can_init();
            can_set_rx_message_consumer(Some(notify_message));
        }
        return Self {};
    }

    pub fn get_message(&self) -> Option<CanMessage> {
        return QUEUE.lock().unwrap().remove();
    }

    pub fn create_message(&self) -> CanMessage {
        unsafe {
            let api_message = can_create_message();
            return CanMessage::new(api_message);
        }
    }

    pub fn send_message(&self, mut message: CanMessage) {
        unsafe {
            can_send_message(message.message);
        }
        message.detach();
    }
}
