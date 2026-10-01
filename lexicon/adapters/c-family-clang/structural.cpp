#include "structural.h"

#include <algorithm>
#include <atomic>
#include <chrono>
#include <filesystem>
#include <mutex>
#include <string>
#include <utility>
#include <vector>

#include "clang/Basic/Version.h"
#include "clang/Tooling/CompilationDatabase.h"
#include "clang/Tooling/Tooling.h"

#include "perf.h"
#include "protocol.h"
#include "structural_compilation.h"
#include "structural_execution.h"
#include "structural_model.h"
#include "structural_transport.h"

namespace {

bool canonical_relative(llvm::StringRef value) {
  if (value.empty() || value.contains('\\')) {
    return false;
  }
  std::filesystem::path path(value.str());
  if (path.is_absolute()) {
    return false;
  }
  for (const auto &part : path) {
    if (part == "..") {
      return false;
    }
  }
  return path.lexically_normal().generic_string() == value;
}

struct StructuralInput {
  std::string root;
  std::vector<std::string> owned_files;
  std::vector<std::string> context_files;
  std::size_t workers = 0;
};

bool read_inventory(const llvm::json::Object &request, llvm::StringRef field,
                    std::vector<std::string> &files, std::string &error) {
  const auto *input = request.getArray(field);
  if (!input) {
    error = field.str() + " is required";
    return false;
  }
  for (const auto &entry : *input) {
    auto value = entry.getAsString();
    if (!value || !canonical_relative(*value)) {
      error = field.str() + " must use canonical repository-relative paths";
      return false;
    }
    files.push_back(value->str());
  }
  std::sort(files.begin(), files.end());
  files.erase(std::unique(files.begin(), files.end()), files.end());
  return true;
}

bool read_execution_value(const llvm::json::Object &request,
                          llvm::StringRef field, std::size_t minimum,
                          std::size_t &value, std::string &error) {
  const auto parsed = request.getInteger(field);
  if (!parsed || *parsed < static_cast<std::int64_t>(minimum)) {
    error = field.str() + " must be at least " + std::to_string(minimum);
    return false;
  }
  value = static_cast<std::size_t>(*parsed);
  return true;
}

bool validate_request(const llvm::json::Object &request, StructuralInput &input,
                      std::string &error) {
  auto protocol = request.getInteger("protocol_version");
  auto operation = request.getString("operation");
  auto repository_root = request.getString("repository_root");
  for (const auto &entry : request) {
    const auto field = entry.first;
    if (field != "protocol_version" && field != "operation" &&
        field != "repository_root" && field != "owned_files" &&
        field != "context_files" && field != "workers") {
      error = std::string("unsupported C-family Clang request field: ") +
              field.str();
      return false;
    }
  }
  if (!protocol || *protocol != lexicon::clang_frontend::kProtocolVersion) {
    error = "unsupported C-family Clang protocol version";
    return false;
  }
  if (!operation || *operation != "structural") {
    error = "unsupported C-family Clang operation";
    return false;
  }
  if (!repository_root || repository_root->empty()) {
    error = "repository_root is required";
    return false;
  }

  std::filesystem::path root_path(repository_root->str());
  if (!root_path.is_absolute()) {
    error = "repository_root must be absolute";
    return false;
  }
  input.root = root_path.lexically_normal().string();

  if (!read_inventory(request, "owned_files", input.owned_files, error) ||
      !read_inventory(request, "context_files", input.context_files, error)) {
    return false;
  }
  for (const auto &path : input.context_files) {
    if (std::binary_search(input.owned_files.begin(), input.owned_files.end(),
                           path)) {
      error = "owned_files and context_files must be disjoint";
      return false;
    }
  }

  if (!read_execution_value(request, "workers", 1, input.workers, error)) {
    return false;
  }
  return true;
}


} // namespace

bool emit_structural(const llvm::json::Object &request,
                     llvm::raw_ostream &output, std::string &error) {
  StructuralInput input;
  if (!validate_request(request, input, error)) {
    return false;
  }
  const auto &root = input.root;

  std::string database_error;
  const auto database_started = lexicon::clang_frontend::PerfClock::now();
  auto base = clang::tooling::CompilationDatabase::autoDetectFromDirectory(
      root, database_error);
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.compilation_database",
      lexicon::clang_frontend::PerfClock::now() - database_started);
  lexicon::clang_frontend::CompilationCommands database(root, base.get());
  lexicon::clang_frontend::State state(root);

  const auto planning_started = lexicon::clang_frontend::PerfClock::now();
  const auto plan = lexicon::clang_frontend::build_parse_plan(
      root, database, input.owned_files, input.context_files);
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.parse_plan",
      lexicon::clang_frontend::PerfClock::now() - planning_started,
      {{"discovered_owned_files",
        static_cast<std::uint64_t>(input.owned_files.size())},
       {"context_files", static_cast<std::uint64_t>(input.context_files.size())},
       {"real_units", static_cast<std::uint64_t>(plan.real_units)},
       {"synthetic_units", static_cast<std::uint64_t>(plan.synthetic_units)},
       {"explicit_header_units",
        static_cast<std::uint64_t>(plan.explicit_header_units)}});

  lexicon::clang_frontend::ExecutionSummary execution;
  lexicon::clang_frontend::TransportSummary transport;
  std::atomic<std::uint64_t> emission_ns{0};
  bool transport_ok = true;
  std::string transport_error;
  std::mutex transport_mutex;
  const auto frontend_started = lexicon::clang_frontend::PerfClock::now();
  const int status = lexicon::clang_frontend::execute_parse_plan(
      root, database, input.owned_files, plan, input.workers, state, execution,
      [&](lexicon::clang_frontend::State &&file_state) {
        const auto emission_started = lexicon::clang_frontend::PerfClock::now();
        auto encoded =
            lexicon::clang_frontend::encode_owned_file_frames(file_state);
        {
          std::lock_guard lock(transport_mutex);
          if (transport_ok &&
              !lexicon::clang_frontend::emit_encoded_file_frames(
                  std::move(encoded), output, transport, transport_error)) {
            transport_ok = false;
          }
        }
        emission_ns.fetch_add(
            static_cast<std::uint64_t>(
                std::chrono::duration_cast<std::chrono::nanoseconds>(
                    lexicon::clang_frontend::PerfClock::now() -
                    emission_started)
                    .count()),
            std::memory_order_relaxed);
      });
  if (!transport_ok) {
    error = std::move(transport_error);
    return false;
  }
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.semantic_analysis",
      std::chrono::nanoseconds(state.semantic_analysis_ns));
  if (status != 0) {
    state.add_diagnostic({
        .severity = "error",
        .message = "Clang tooling returned status " + std::to_string(status),
    });
  }
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.execution",
      std::chrono::nanoseconds(0),
      {{"discovered_owned_files",
        static_cast<std::uint64_t>(input.owned_files.size())},
       {"primary_real_parse_units",
        static_cast<std::uint64_t>(plan.real_units)},
       {"primary_synthetic_parse_units",
        static_cast<std::uint64_t>(execution.primary_synthetic_parse_units)},
       {"synthetic_source_candidates",
        static_cast<std::uint64_t>(plan.synthetic_units)},
       {"skipped_covered_source_units",
        static_cast<std::uint64_t>(execution.skipped_covered_source_units)},
       {"explicit_header_compile_units",
        static_cast<std::uint64_t>(plan.explicit_header_units)},
       {"orphan_fallback_units",
        static_cast<std::uint64_t>(execution.orphan_fallback_units)},
       {"completed_orphan_tus",
        static_cast<std::uint64_t>(execution.completed_orphan_tus)},
       {"claimed_orphan_files",
        static_cast<std::uint64_t>(execution.claimed_orphan_files)},
       {"discarded_duplicate_orphan_observations",
        static_cast<std::uint64_t>(
            execution.discarded_duplicate_orphan_observations)},
       {"active_clang_lanes",
        static_cast<std::uint64_t>(execution.active_clang_lanes)},
       {"completed_tus", static_cast<std::uint64_t>(execution.completed_tus)},
       {"peak_pending_estimated_bytes",
        static_cast<std::uint64_t>(execution.peak_pending_estimated_bytes)},
       {"pending_byte_limit", static_cast<std::uint64_t>(64 * 1024 * 1024)},
       {"claimed_owned_files",
        static_cast<std::uint64_t>(execution.claimed_owned_files)},
       {"discarded_duplicate_file_observations",
        static_cast<std::uint64_t>(
            execution.discarded_duplicate_file_observations)}});
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.frontend_work",
      lexicon::clang_frontend::PerfClock::now() - frontend_started,
      {{"completed_tus", static_cast<std::uint64_t>(execution.completed_tus)}});

  const auto metadata_started = lexicon::clang_frontend::PerfClock::now();
  if (!lexicon::clang_frontend::emit_structural_metadata(
          state, base != nullptr, clang::getClangFullVersion(),
          lexicon::clang_frontend::kHelperVersion, output, transport, error)) {
    return false;
  }
  emission_ns.fetch_add(
      static_cast<std::uint64_t>(
          std::chrono::duration_cast<std::chrono::nanoseconds>(
              lexicon::clang_frontend::PerfClock::now() - metadata_started)
              .count()),
      std::memory_order_relaxed);
  const auto peak_helper_rss_bytes =
      lexicon::clang_frontend::peak_rss_bytes();
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.observation_emission",
      std::chrono::nanoseconds(emission_ns.load(std::memory_order_relaxed)),
      {{"observed_files", transport.file_frames},
       {"transport_frames", transport.frames},
       {"transport_bytes", transport.bytes},
       {"framed_transport_bytes", transport.bytes},
       {"peak_rss_bytes", peak_helper_rss_bytes},
       {"peak_helper_rss_bytes", peak_helper_rss_bytes}});
  if (status != 0) {
    error = "Clang tooling returned status " + std::to_string(status);
    return false;
  }
  return true;
}
