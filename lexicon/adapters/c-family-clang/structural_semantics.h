#pragma once

#include "llvm/Support/JSON.h"

#include "structural_model.h"

namespace lexicon::clang_frontend {

void normalize_semantics(File &file);
llvm::json::Array relationships_json(const File &file);
llvm::json::Array calls_json(const File &file);

} // namespace lexicon::clang_frontend
