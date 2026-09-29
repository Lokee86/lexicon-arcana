#pragma once

#include <string>

#include "llvm/Support/JSON.h"

bool emit_structural(const llvm::json::Object &request,
                     llvm::json::Object &response, std::string &error);
