// use crate::analog3::A3_ADMIN_REQUEST_ID;
use crate::analog3 as a3;
use crate::can_controller::CanController;
use crate::can_controller::CanMessage;

/*
// ID assignments /////////////////////////////////
pub const A3_ID_MIDI_TIMING_CLOCK: u32 = 0x100;
pub const A3_ID_MIDI_VOICE_BASE: u32 = 0x101;
pub const A3_ID_MIDI_REAL_TIME: u32 = 0x140;

pub const A3_ID_MISSION_CONTROL: u32 = 0x700;
pub const A3_ID_INDIVIDUAL_MODULE_BASE: u32 = 0x700;

const A3_ADMIN_REQUEST_ID: u8 = 0x00;

/* Mission control opcodes */
pub const A3_MC_REGISTRATION_CHECK_REPLY: u8 = 0x00;
pub const A3_MC_ASSIGN_MODULE_ID: u8 = 0x01;
pub const A3_MC_PING: u8 = 0x02;
*/

pub struct ModuleManager<'a> {
    can_controller: &'a CanController,
}

impl<'a> ModuleManager<'a> {
    pub fn new(can_controller: &'a CanController) -> Self {
        return Self {
            can_controller: can_controller,
        };
    }

    pub fn handle_message(&self, message: CanMessage) {
        log::debug!("Message received: id={:08x}", message.id());
        if message.data_length() == 0 {
            // TODO: What should we do in this case?
            return;
        }
        if message.is_extended() {
            let opcode = message.get_data(0);
            match opcode {
                a3::A3_ADMIN_REQUEST_ID => self.assign_module_id(message),
                _ => log::warn!("Unknown request {:02x}", opcode),
            }
        }
    }

    fn assign_module_id(&self, in_message: CanMessage) {
        let module_id = 0x03u8;
        let mut out_message = self.can_controller.create_message();
        let remote_uid = in_message.id();
        out_message.set_id(a3::A3_ID_MISSION_CONTROL);
        out_message.set_extended(false);
        out_message.set_remote(false);
        out_message.set_data_length(6);
        out_message.set_data(0, a3::A3_MC_ASSIGN_MODULE_ID);
        out_message.set_data(1, ((remote_uid >> 24) & 0xff) as u8);
        out_message.set_data(2, ((remote_uid >> 16) & 0xff) as u8);
        out_message.set_data(3, ((remote_uid >> 8) & 0xff) as u8);
        out_message.set_data(4, (remote_uid & 0xff) as u8);
        out_message.set_data(5, module_id);
        self.can_controller.send_message(out_message);
        log::info!(
            "Issued module id {:03x} for uid {:08x}",
            module_id,
            remote_uid
        );
    }
}
