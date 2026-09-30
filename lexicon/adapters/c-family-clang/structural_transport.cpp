#include "structural_transport.h"

#include <optional>
#include <string>
#include <utility>

#include "llvm/Support/FormatVariadic.h"
#include "llvm/Support/JSON.h"

#include "protocol.h"

namespace lexicon::clang_frontend {
namespace {

bool emit_frame(llvm::raw_ostream &output, llvm::StringRef kind,
                std::optional<llvm::StringRef> path,
                llvm::json::Object payload, TransportSummary &summary,
                std::string &error) {
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
  output << encoded_header << "\n";
  output.write(encoded.data(), encoded.size());
  output << "\n";
  output.flush();

  summary.frames += 1;
  summary.bytes += encoded_header.size() + 1 + encoded.size() + 1;
  if (kind == "file") {
    summary.file_frames += 1;
  }
  return true;
}

} // namespace

bool emit_structural_frames(State &state, bool compilation_database,
                            std::string clang_version,
                            llvm::StringRef helper_version,
                            llvm::raw_ostream &output,
                            TransportSummary &summary, std::string &error) {
  if (!emit_frame(output, "metadata", std::nullopt,
                  state.metadata_response(compilation_database,
                                          std::move(clang_version),
                                          helper_version),
                  summary, error)) {
    return false;
  }

  for (auto &[path, value] : state.files) {
    if (!state.all_owned_paths.contains(path)) {
      continue;
    }
    if (!emit_frame(output, "file", path, file_json(std::move(value)), summary,
                    error)) {
      return false;
    }
  }
  return true;
}

} // namespace lexicon::clang_frontend
