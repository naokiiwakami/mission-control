// ID assignments /////////////////////////////////
pub const A3_ID_MIDI_TIMING_CLOCK: u32 = 0x100;
pub const A3_ID_MIDI_VOICE_BASE: u32 = 0x101;
pub const A3_ID_MIDI_REAL_TIME: u32 = 0x140;

pub const A3_ID_MISSION_CONTROL: u32 = 0x700;
pub const A3_ID_INDIVIDUAL_MODULE_BASE: u32 = 0x700;

// Message types //////////////////////////////////

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

/* Module administration messages */
pub const A3_ADMIN_REQUEST_ID: u8 = 0x00;

/* Mission control opcodes */
pub const A3_MC_REGISTRATION_CHECK_REPLY: u8 = 0x00;
pub const A3_MC_ASSIGN_MODULE_ID: u8 = 0x01;
pub const A3_MC_PING: u8 = 0x02;

pub const A3_DATA_LENGTH: u8 = 8;
