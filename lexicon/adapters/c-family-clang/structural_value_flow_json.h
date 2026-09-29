#pragma once

#include "llvm/Support/JSON.h"

#include "structural_model.h"

namespace lexicon::clang_frontend {

void normalize_value_flow(File &file);
llvm::json::Array pointer_bindings_json(const File &file);
llvm::json::Array accesses_json(const File &file);

} // namespace lexicon::clang_frontend
