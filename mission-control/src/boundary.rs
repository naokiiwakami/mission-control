#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use std::collections::HashMap;
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
}

static EVENT_FD_HOLDER: LazyLock<Mutex<FdHolder>> =
    LazyLock::new(|| Mutex::new(FdHolder { fd: 0 }));

#[unsafe(no_mangle)]
pub extern "C" fn notify_message(message: *mut can_message_t) {
    log::debug!("message received: {:#x}", message as u64);
    let holder = EVENT_FD_HOLDER.lock().unwrap();
    unsafe {
        libc::eventfd_write(holder.fd, message as u64);
    }
}

pub struct Boundary {
    epollfd: libc::c_int,
    eventfd: libc::c_int,

    fd_to_event_type: HashMap<libc::c_int, EventType>,
}

// TODO: Make this singleton
impl Boundary {
    pub fn new() -> Self {
        let epollfd: libc::c_int;
        let eventfd: libc::c_int;
        let mut fd_to_event_type = HashMap::new();
        unsafe {
            // set up the epoll event listener
            epollfd = libc::epoll_create1(0);
            if epollfd < 0 {
                // TODO: handle error
            }
            eventfd = libc::eventfd(0, 0);
            log::debug!("eventfd={}", eventfd);
            let mut ev = libc::epoll_event {
                events: libc::EPOLLIN as u32,
                u64: eventfd as u64,
            };
            if libc::epoll_ctl(epollfd, libc::EPOLL_CTL_ADD, eventfd, &mut ev) < 0 {
                // TODO: handle error
            }
            EVENT_FD_HOLDER.lock().unwrap().fd = eventfd;
            fd_to_event_type.insert(eventfd, EventType::MessageReceived);

            can_init();
            can_set_rx_message_consumer(Some(notify_message));
        }
        return Self {
            epollfd: epollfd,
            eventfd: eventfd,
            fd_to_event_type: fd_to_event_type,
        };
    }

    pub fn notify(&self, event_type: char) {
        unsafe {
            let event_type_p: *const char = &event_type;
            let ptr: *const libc::c_void = std::mem::transmute(event_type_p);
            libc::write(self.eventfd, ptr, 1);
        }
    }

    pub fn wait_for_event(&self) -> &EventType {
        unsafe {
            let max_events: libc::c_int = 1; // concurrent dispatch not needed (yet)
            let mut events = libc::epoll_event { events: 0, u64: 0 };
            loop {
                // TODO: get out of this intermittently to check shutdown status
                let nfs = libc::epoll_wait(self.epollfd, &mut events, max_events, -1);
                if nfs < 0 {
                    // TODO: handle error
                    log::error!("epoll error!");
                }
                let fd = (events.u64 & 0xffffffff) as libc::c_int;
                return match self.fd_to_event_type.get(&fd) {
                    Some(event_type) => event_type,
                    None => &EventType::NoEvent,
                };
            }
        }
    }

    pub fn get_message(&self) -> Option<CanMessage> {
        // return QUEUE.lock().unwrap().remove();
        unsafe {
            let mut data = 0;
            let result = libc::eventfd_read(self.eventfd, &mut data);
            if result < 0 {
                let errno = *libc::__errno_location();
                if errno == libc::EAGAIN || errno == libc::EWOULDBLOCK {
                    log::debug!("eagain");
                    return None;
                }
                // TODO: handle error
                log::error!("read eventfd failed, fd={}, errno={}", self.eventfd, errno);
                return None;
            }
            let message: *mut can_message_t = std::mem::transmute(data);
            log::debug!(
                "notif received: data={:#x} message={:#x}",
                data,
                message as u64
            );
            // event_type = (data & 0xff) as u8 as char;
            return Some(CanMessage::new(message));
        }
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
