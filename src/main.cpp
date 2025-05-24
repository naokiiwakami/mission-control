#include <errno.h>
#include <poll.h>
#include <string.h>
#include <wiringPi.h>
#include <wiringPiSPI.h>

#include <cstdint>
#include <cstdio>

#include "can-controller/api.h"
#include "can-controller/device/mcp2515.h"
#include "module_manager.h"
#include "queue.h"

int main() {
  if (can_init()) {
    return -1;
  }

  printf("Done configuring CAN controller:\n");
  {
    uint8_t buffer[18] = {0};
    for (uint8_t addr = 0; addr < 0x80; addr += 0x10) {
      uint8_t *result = mcp2515_read(addr, buffer, 16);
      for (int i = 0; i < 16; ++i) {
        printf(" %02x", result[i]);
      }
      printf("\n");
    }
  }

  auto module_manager = new analog3::ModuleManager();

  printf("\nlistening...\n");

  while (true) {
    auto message = queue_remove();
    if (message == nullptr) {
      continue;
    }
    if (!message->is_extended) {
      printf("std[ %02x %02x ]: ", static_cast<uint8_t>(message->id >> 8),
             static_cast<uint8_t>(message->id));
    } else {
      printf(
          "ext[ %02x %02x %2x %2x ]:", static_cast<uint8_t>(message->id >> 24),
          static_cast<uint8_t>(message->id >> 16),
          static_cast<uint8_t>(message->id >> 8),
          static_cast<uint8_t>(message->id));
    }
    if (message->is_remote) {
      printf(" REMOTE\n");
    } else {
      for (int i = 0; i < message->data_length; ++i) {
        printf(" %02x", message->data[i]);
      }
      printf("\n");
    }
    module_manager->HandleMessage(message);
    can_free_message(message);
  }

  return 0;
}

// implement the callback function to handle a CAN RX message
void can_consume_rx_message(can_message_t *message) { queue_add(message); }