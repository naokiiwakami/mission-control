#pragma once

#include "can-controller/can_message.h"

namespace analog3 {

class ModuleManager {
 public:
  ModuleManager() {}
  ~ModuleManager() = default;

  void HandleMessage(can_message_t *message);

 private:
  void AssignModuleId(can_message_t *message);
};

}  // namespace analog3