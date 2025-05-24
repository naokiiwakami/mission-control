#pragma once

#include <stdint.h>

typedef struct can_message {
  uint32_t id;
  uint8_t is_extended;
  uint8_t is_remote;
  uint8_t data_length;
  uint8_t data[8];
} can_message_t;