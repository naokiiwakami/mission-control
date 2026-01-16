#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use std::sync::{LazyLock, Mutex};
use tokio::{
    sync::mpsc::{Receiver, Sender, channel},
    task::JoinHandle,
};

#[derive(Debug)]
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

    pub fn is_fd(&self) -> bool {
        unsafe {
            return (*self.message).is_fd != 0;
        }
    }

    pub fn set_fd(&mut self, is_fd: bool) {
        unsafe {
            (*self.message).is_fd = is_fd as u8;
        }
    }

    pub fn brs(&self) -> bool {
        unsafe {
            return (*self.message).brs != 0;
        }
    }

    pub fn set_brs(&mut self, brs: bool) {
        unsafe {
            (*self.message).brs = brs as u8;
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

    pub fn mut_data(&self) -> &mut [u8; 8usize] {
        unsafe {
            return &mut (*self.message).data;
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
            if let Some(mut message) = tx_receiver.recv().await {
                // message.set_fd(true);
                // message.set_brs(true);
                if log::log_enabled!(log::Level::Debug) {
                    let mut data_elements = Vec::<String>::new();
                    for i in 0..message.data_length() as usize {
                        data_elements.push(format!("{:02x}", message.data()[i]));
                    }
                    log::debug!(
                        "Message sending: id={:08x} data={}",
                        message.id(),
                        data_elements.join(" ")
                    );
                }
                unsafe {
                    can_send_message(message.message);
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
        let handle = can_filter_start_config();
        if handle == std::ptr::null_mut() {
            log::error!("Failed to start CAN filter configuration");
            std::process::exit(1);
        }
        let result_clear = can_filter_clear(handle);
        if result_clear != 0 {
            log::error!("Failed to clear CAN filter; error={}", result_clear);
        }
        let result_std = can_filter_add_std_id_gte(handle, 0x680);
        if result_std != 0 {
            log::error!("Failed to set filter for STD IDs; error={}", result_std);
        }
        let result_ext = can_filter_add_ext_id_all(handle);
        if result_ext != 0 {
            log::error!("Failed to set filter for all EXT IDs; error={}", result_ext);
        }
        let result_apply = can_filter_apply_config(handle);
        if result_apply != 0 {
            log::error!("Failed to apply filter settings; error={}", result_apply);
        }
        if can_start() != 0 {
            log::error!("Error encountered while starting CAN controller");
            std::process::exit(1);
        }
    }

    // set up message tx
    let (tx_sender, tx_receiver) = channel(16);

    let handle = run_tx(tx_receiver);

    (tx_sender, rx_receiver, handle)
}
