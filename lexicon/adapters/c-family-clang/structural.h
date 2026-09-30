#pragma once

#include <string>

#include "llvm/Support/JSON.h"
#include "llvm/Support/raw_ostream.h"

bool emit_structural(const llvm::json::Object &request,
                     llvm::raw_ostream &output, std::string &error);
