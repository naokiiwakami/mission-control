#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use std::sync::mpsc::Sender;
use std::sync::{LazyLock, Mutex};

use crate::event_type::EventType;

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

struct FdHolder {
    fd: libc::c_int,
    notifier: Option<Sender<EventType>>,
}

static EVENT_FD_HOLDER: LazyLock<Mutex<FdHolder>> = LazyLock::new(|| {
    Mutex::new(FdHolder {
        fd: 0,
        notifier: None,
    })
});

#[unsafe(no_mangle)]
pub extern "C" fn notify_message(message: *mut can_message_t) {
    log::debug!("message received: {:#x}", message as u64);
    let holder = EVENT_FD_HOLDER.lock().unwrap();
    unsafe {
        if let Some(sender) = &holder.notifier {
            if holder.fd > 0 {
                libc::eventfd_write(holder.fd, message as u64);
            }
            if let Err(error) = sender.send(EventType::MessageRx) {
                log::error!("Notif error: {error:?}");
            }
        }
    }
}

pub struct CanController {
    rx_fd: libc::c_int,
    tx_fd: libc::c_int,
    tx_notif: Sender<EventType>,
}

impl CanController {
    pub fn new(notifier: Sender<EventType>) -> Self {
        unsafe {
            // set up eventfd for CAN RX pipe
            let rx_fd = libc::eventfd(0, 0);
            log::debug!("rx_fd={}", rx_fd);
            if rx_fd < 0 {
                // TODO: handle error
            }

            let mut holder = EVENT_FD_HOLDER.lock().unwrap();
            holder.fd = rx_fd;
            holder.notifier = Some(notifier.clone());

            // set up eventfd for CAN TX pipe
            let tx_fd = libc::eventfd(0, 0);
            log::debug!("tx_fd={}", tx_fd);
            if tx_fd < 0 {
                // TODO: handle error
            }

            // OK the CAN interface is ready to be initialized
            can_set_rx_message_consumer(Some(notify_message));
            can_init();

            return Self {
                rx_fd: rx_fd,
                tx_fd: tx_fd,
                tx_notif: notifier,
            };
        }
    }

    pub fn get_message(&self) -> Option<CanMessage> {
        return match self.get_message_from_pipe(self.rx_fd) {
            Some(message) => Some(CanMessage::new(message)),
            None => None,
        };
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
        unsafe {
            libc::eventfd_write(self.tx_fd, message.message as u64);
        }
        // The CAN interface will take care of releasing the message.
        // We disconnect the object from the Rust ecosystem here.
        message.detach();
        match self.tx_notif.send(EventType::MessageTx) {
            Ok(_) => {}
            Err(e) => log::error!("Failed to send MessageTx notif: {e:?}"),
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
        match self.get_message_from_pipe(self.tx_fd) {
            Some(message) => {
                unsafe {
                    log::debug!("Sending message, ID={:08x}", (*message).id);
                    can_send_message(message);
                };
            }
            None => {}
        }
    }

    fn get_message_from_pipe(&self, fd: libc::c_int) -> Option<*mut can_message_t> {
        unsafe {
            let mut data = 0;
            let result = libc::eventfd_read(fd, &mut data);
            if result < 0 {
                let errno = *libc::__errno_location();
                if errno == libc::EAGAIN || errno == libc::EWOULDBLOCK {
                    log::debug!("eagain");
                    return None;
                }
                // TODO: handle error
                log::error!("read eventfd failed, fd={}, errno={}", self.rx_fd, errno);
                return None;
            }
            let message: *mut can_message_t = std::mem::transmute(data);
            return Some(message);
        }
    }
}
