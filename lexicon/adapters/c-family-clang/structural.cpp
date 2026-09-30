#include "structural.h"

#include <algorithm>
#include <filesystem>
#include <string>
#include <utility>
#include <vector>

#include "clang/Basic/Version.h"
#include "clang/Tooling/CompilationDatabase.h"
#include "clang/Tooling/Tooling.h"

#include "perf.h"
#include "protocol.h"
#include "structural_execution.h"
#include "structural_model.h"
#include "structural_plan.h"
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
  std::size_t shards = 0;
  std::size_t merge_fan_in = 0;
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
  if (request.get("files")) {
    error = "legacy files field is unsupported";
    return false;
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

  if (!read_execution_value(request, "workers", 1, input.workers, error) ||
      !read_execution_value(request, "shards", 1, input.shards, error) ||
      !read_execution_value(request, "merge_fan_in", 2, input.merge_fan_in,
                            error)) {
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
  const auto plan = lexicon::clang_frontend::build_task_plan(
      root, database, input.owned_files, input.context_files);
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.context_planning",
      lexicon::clang_frontend::PerfClock::now() - planning_started,
      {{"owned_files", static_cast<std::uint64_t>(input.owned_files.size())},
       {"context_files", static_cast<std::uint64_t>(input.context_files.size())},
       {"semantic_tasks", static_cast<std::uint64_t>(plan.tasks.size())},
       {"dependency_scans",
        static_cast<std::uint64_t>(plan.dependency_scan_attempts)},
       {"dependency_scan_failures",
        static_cast<std::uint64_t>(plan.dependency_scan_failures)},
       {"synthetic_header_tasks",
        static_cast<std::uint64_t>(plan.synthetic_header_tasks)}});

  lexicon::clang_frontend::ExecutionSummary execution;
  const auto frontend_started = lexicon::clang_frontend::PerfClock::now();
  int status = lexicon::clang_frontend::execute_task_plan(
      root, database, state, plan.tasks,
      {
          .workers = input.workers,
          .shards = input.shards,
          .merge_fan_in = input.merge_fan_in,
      },
      execution);
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.frontend_plan", std::chrono::nanoseconds(0),
      {{"semantic_tasks", static_cast<std::uint64_t>(plan.tasks.size())},
       {"logical_shards",
        static_cast<std::uint64_t>(execution.logical_shards)},
       {"worker_limit", static_cast<std::uint64_t>(execution.worker_limit)},
       {"merge_fan_in", static_cast<std::uint64_t>(input.merge_fan_in)}});
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.frontend_work",
      lexicon::clang_frontend::PerfClock::now() - frontend_started,
      {{"translation_units",
        static_cast<std::uint64_t>(state.translation_units.size())},
       {"semantic_tasks", static_cast<std::uint64_t>(plan.tasks.size())}});
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.semantic_analysis",
      std::chrono::nanoseconds(state.semantic_analysis_ns));
  if (status != 0) {
    state.add_diagnostic({
        .severity = "error",
        .message = "Clang tooling returned status " + std::to_string(status),
    });
  }

  const auto emission_started = lexicon::clang_frontend::PerfClock::now();
  lexicon::clang_frontend::TransportSummary transport;
  if (!lexicon::clang_frontend::emit_structural_frames(
          state, base != nullptr, clang::getClangFullVersion(),
          lexicon::clang_frontend::kHelperVersion, output, transport, error)) {
    return false;
  }
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.observation_emission",
      lexicon::clang_frontend::PerfClock::now() - emission_started,
      {{"observed_files", transport.file_frames},
       {"transport_frames", transport.frames},
       {"transport_bytes", transport.bytes},
       {"peak_rss_bytes", lexicon::clang_frontend::peak_rss_bytes()}});
  return true;
}
