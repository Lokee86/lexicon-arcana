#pragma once

#include <cstdint>
#include <string>

#include "llvm/Support/raw_ostream.h"

#include "structural_model.h"

namespace lexicon::clang_frontend {

struct TransportSummary {
  std::uint64_t frames = 0;
  std::uint64_t bytes = 0;
  std::uint64_t file_frames = 0;
};

bool emit_structural_frames(State &state, bool compilation_database,
                            std::string clang_version,
                            llvm::StringRef helper_version,
                            llvm::raw_ostream &output,
                            TransportSummary &summary, std::string &error);

} // namespace lexicon::clang_frontend
