#include "module_manager.h"

#include <stdio.h>

#include "analog3.h"
#include "can-controller/api.h"
#include "can-controller/device/mcp2515.h"

namespace analog3 {

void ModuleManager::HandleMessage(can_message_t *message) {
  if (message->is_extended && !message->is_remote && message->data_length > 0) {
    // This is an A3 administration request
    switch (message->data[0]) {
      case A3_ADMIN_REQUEST_ID:
        AssignModuleId(message);
        break;
      default:
        // TODO: introduce a logger
        fprintf(stderr, "Unsupported opcode %x\n", message->data[0]);
    }
  }
}

void ModuleManager::AssignModuleId(can_message_t *message) {
  uint8_t module_id = 0x03;

  // TODO: Make an A3 API?
  auto *response = can_create_message();
  response->id = A3_ID_MISSION_CONTROL;
  response->is_extended = 0;
  response->is_remote = 0;
  response->data_length = 6;
  response->data[0] = A3_MC_ASSIGN_MODULE_ID;  // opcode
  response->data[1] = message->id >> 24;       // the target unique ID
  response->data[2] = message->id >> 16;
  response->data[3] = message->id >> 8;
  response->data[4] = message->id;
  response->data[5] = module_id;  // assigned module ID
  can_send_message(response);
}

}  // namespace analog3