#include "module_manager.h"

#include <stdio.h>

#include "analog3.h"
#include "mcp2515.h"

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
  uint8_t local_buf[16];
  int index = mcp2515_set_can_id_std(local_buf, A3_ID_MISSION_CONTROL, 6);
  local_buf[index++] = A3_MC_ASSIGN_MODULE_ID;
  local_buf[index++] = message->id >> 24;
  local_buf[index++] = message->id >> 16;
  local_buf[index++] = message->id >> 8;
  local_buf[index++] = message->id;
  local_buf[index++] = module_id;
  mcp2515_message_request_to_send_txb0(local_buf, index);
}

}  // namespace analog3