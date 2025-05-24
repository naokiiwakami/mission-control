#pragma once

#include <stdint.h>

#include "can-controller/can_message.h"

#define QUEUE_SIZE 16

#ifdef __cplusplus
extern "C" {
#endif

extern volatile uint8_t q_first;
extern volatile uint8_t q_last;
extern can_message_t *queue_array[QUEUE_SIZE];

extern void queue_add(can_message_t *message);
extern can_message_t *queue_remove();
extern uint8_t queue_empty();

#ifdef __cplusplus
}
#endif
