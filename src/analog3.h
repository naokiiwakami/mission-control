#pragma once

#include <stdint.h>

#include "can-controller/can_message.h"

// ID assignments /////////////////////////////////
#define A3_ID_MIDI_TIMING_CLOCK 0x100
#define A3_ID_MIDI_VOICE_BASE 0x101
#define A3_ID_MIDI_REAL_TIME 0x140

#define A3_ID_MISSION_CONTROL 0x700
#define A3_ID_INDIVIDUAL_MODULE_BASE 0x700

// Message types //////////////////////////////////

/* MIDI channel voice messages */
#define A3_VOICE_MSG_SET_NOTE 0x07
#define A3_VOICE_MSG_GATE_OFF 0x08
#define A3_VOICE_MSG_GATE_ON 0x09
#define A3_VOICE_MSG_POLY_KEY_PRESSURE 0x0A

/* MIDI channel messages */
#define A3_VOICE_MSG_CONTROL_CHANGE 0x0B
#define A3_VOICE_MSG_PROGRAM_CHANGE 0x0C
#define A3_VOICE_MSG_CHANNEL_PRESSURE 0x0D
#define A3_VOICE_MSG_PITCH_BEND 0x0E

/* Module administration messages */
#define A3_ADMIN_REQUEST_ID 0x00

/* Mission control opcodes */
#define A3_MC_REGISTRATION_CHECK_REPLY 0x00
#define A3_MC_ASSIGN_MODULE_ID 0x01
#define A3_MC_PING 0x02

#define A3_DATA_LENGTH 8

extern void A3SendData(can_message_t *message);
