pub mod config;
pub mod schema;

use num_enum::{IntoPrimitive, TryFromPrimitive};

// ID assignments /////////////////////////////////
pub const A3_ID_MIDI_TIMING_CLOCK: u16 = 0x100;
pub const A3_ID_MIDI_VOICE_BASE: u16 = 0x101;
pub const A3_ID_MIDI_REAL_TIME: u16 = 0x140;

pub const A3_ID_ADMIN_WIRES_BASE: u16 = 0x680;

pub const A3_ID_MISSION_CONTROL: u16 = 0x700;
pub const A3_ID_INDIVIDUAL_MODULE_BASE: u16 = 0x700;

// Message opcodes //////////////////////////////////

/* MIDI channel voice messages */
pub const A3_VOICE_MSG_SET_NOTE: u8 = 0x07;
pub const A3_VOICE_MSG_GATE_OFF: u8 = 0x08;
pub const A3_VOICE_MSG_GATE_ON: u8 = 0x09;
pub const A3_VOICE_MSG_POLY_KEY_PRESSURE: u8 = 0x0A;

/* MIDI channel messages */
pub const A3_VOICE_MSG_CONTROL_CHANGE: u8 = 0x0B;
pub const A3_VOICE_MSG_PROGRAM_CHANGE: u8 = 0x0C;
pub const A3_VOICE_MSG_CHANNEL_PRESSURE: u8 = 0x0D;
pub const A3_VOICE_MSG_PITCH_BEND: u8 = 0x0E;

/* Module administration opcodes */
pub const A3_ADMIN_SIGN_IN: u8 = 0x01;
pub const A3_ADMIN_NOTIFY_ID: u8 = 0x02;
pub const A3_ADMIN_REQ_UID_CANCEL: u8 = 0x03;

/* Mission control opcodes */
pub const A3_MC_SIGN_IN: u8 = 0x01;
pub const A3_MC_ASSIGN_MODULE_ID: u8 = 0x02;
pub const A3_MC_PING: u8 = 0x03;
pub const A3_MC_REQUEST_NAME: u8 = 0x04;
pub const A3_MC_REQUEST_CONFIG: u8 = 0x05;
pub const A3_MC_CONTINUE_STREAM: u8 = 0x06;
pub const A3_MC_MODIFY_CONFIG: u8 = 0x08;

/* Individual module opcodes */
pub const A3_IM_REPLY_PING: u8 = 0x01;
pub const A3_IM_ID_ASSIGN_ACK: u8 = 0x02;

// Stream ////////////////////////////////

#[derive(Debug, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum StreamStatus {
    Ready = 0x0,
    Busy = 0x1,
    NotSupported = 0x2,
    NoSuchStream = 0x3,
}

pub const A3_STREAM_PAYLOAD_SIZE: usize = 8;

// Properties /////////////////////////////////////

/* Common property types */

#[derive(Debug, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum PropertyId {
    ModuleUid = 0,
    ModuleType = 1,
    Name = 2,
}
