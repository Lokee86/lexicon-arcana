#include "structural_transport.h"

#include <optional>
#include <string>
#include <utility>

#include "llvm/Support/FormatVariadic.h"
#include "llvm/Support/JSON.h"

#include "protocol.h"

namespace lexicon::clang_frontend {
namespace {

std::string encode_frame(llvm::StringRef kind,
                         std::optional<llvm::StringRef> path,
                         llvm::json::Object payload) {
  auto encoded =
      llvm::formatv("{0}", llvm::json::Value(std::move(payload))).str();
  llvm::json::Object header{
      {"protocol_version", kProtocolVersion},
      {"kind", kind},
      {"bytes", static_cast<std::int64_t>(encoded.size())},
  };
  if (path) {
    header["path"] = *path;
  }

  auto encoded_header =
      llvm::formatv("{0}", llvm::json::Value(std::move(header))).str();
  std::string frame;
  frame.reserve(encoded_header.size() + encoded.size() + 2);
  frame.append(encoded_header);
  frame.push_back('\n');
  frame.append(encoded);
  frame.push_back('\n');
  return frame;
}

bool emit_frame(llvm::raw_ostream &output, llvm::StringRef kind,
                std::optional<llvm::StringRef> path,
                llvm::json::Object payload, TransportSummary &summary,
                std::string &error) {
  auto frame = encode_frame(kind, path, std::move(payload));
  output.write(frame.data(), frame.size());
  output.flush();

  summary.frames += 1;
  summary.bytes += frame.size();
  if (kind == "file") {
    summary.file_frames += 1;
  }
  return true;
}

} // namespace

bool emit_structural_metadata(State &state, bool compilation_database,
                              std::string clang_version,
                              llvm::StringRef helper_version,
                              llvm::raw_ostream &output,
                              TransportSummary &summary, std::string &error) {
  return emit_frame(output, "metadata", std::nullopt,
                    state.metadata_response(compilation_database,
                                            std::move(clang_version),
                                            helper_version),
                    summary, error);
}

EncodedFileFrames encode_owned_file_frames(State &state) {
  EncodedFileFrames encoded;
  for (auto &[path, value] : state.files) {
    if (!state.all_owned_paths.contains(path)) {
      continue;
    }
    auto frame =
        encode_frame("file", path, file_json(std::move(value)));
    encoded.bytes += frame.size();
    encoded.frames.push_back(std::move(frame));
  }
  return encoded;
}

bool emit_encoded_file_frames(EncodedFileFrames frames,
                              llvm::raw_ostream &output,
                              TransportSummary &summary, std::string &error) {
  for (const auto &frame : frames.frames) {
    output.write(frame.data(), frame.size());
  }
  output.flush();
  summary.frames += frames.frames.size();
  summary.file_frames += frames.frames.size();
  summary.bytes += frames.bytes;
  return true;
}

bool emit_owned_file_frames(State &state, llvm::raw_ostream &output,
                            TransportSummary &summary, std::string &error) {
  return emit_encoded_file_frames(encode_owned_file_frames(state), output,
                                  summary, error);
}

bool emit_structural_frames(State &state, bool compilation_database,
                            std::string clang_version,
                            llvm::StringRef helper_version,
                            llvm::raw_ostream &output,
                            TransportSummary &summary, std::string &error) {
  if (!emit_structural_metadata(state, compilation_database,
                                std::move(clang_version), helper_version,
                                output, summary, error)) {
    return false;
  }
  return emit_owned_file_frames(state, output, summary, error);
}

} // namespace lexicon::clang_frontend