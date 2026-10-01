#pragma once

#include <cstdint>
#include <string>
#include <vector>

#include "llvm/Support/raw_ostream.h"

#include "structural_model.h"

namespace lexicon::clang_frontend {

struct TransportSummary {
  std::uint64_t frames = 0;
  std::uint64_t bytes = 0;
  std::uint64_t file_frames = 0;
};

struct EncodedFileFrames {
  std::vector<std::string> frames;
  std::uint64_t bytes = 0;
};

bool emit_structural_metadata(State &state, bool compilation_database,
                              std::string clang_version,
                              llvm::StringRef helper_version,
                              llvm::raw_ostream &output,
                              TransportSummary &summary, std::string &error);

EncodedFileFrames encode_owned_file_frames(State &state);

bool emit_encoded_file_frames(EncodedFileFrames frames,
                              llvm::raw_ostream &output,
                              TransportSummary &summary, std::string &error);

bool emit_owned_file_frames(State &state, llvm::raw_ostream &output,
                            TransportSummary &summary, std::string &error);

bool emit_structural_frames(State &state, bool compilation_database,
                            std::string clang_version,
                            llvm::StringRef helper_version,
                            llvm::raw_ostream &output,
                            TransportSummary &summary, std::string &error);

} // namespace lexicon::clang_frontend