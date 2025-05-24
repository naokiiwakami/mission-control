#include "queue.h"

volatile uint8_t q_first = 0;
volatile uint8_t q_last = 0;
can_message_t *queue_array[QUEUE_SIZE];

void queue_add(can_message_t *item) {
  queue_array[q_last] = item;
  q_last = (q_last + 1) % QUEUE_SIZE;
}

can_message_t *queue_remove() {
  if (q_first == q_last) {
    return nullptr;
  }
  auto *val = queue_array[q_first];
  q_first = (q_first + 1) % QUEUE_SIZE;
  return val;
}

uint8_t queue_empty() { return q_first == q_last; }
