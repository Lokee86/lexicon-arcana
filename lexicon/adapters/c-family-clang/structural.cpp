#include "structural.h"

#include <algorithm>
#include <cstdlib>
#include <filesystem>
#include <memory>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#include "clang/Basic/Version.h"
#include "clang/Tooling/CompilationDatabase.h"
#include "clang/Tooling/Tooling.h"

#include "structural_frontend.h"
#include "perf.h"
#include "protocol.h"
#include "structural_model.h"
#include "structural_plan.h"

namespace {

std::size_t frontend_jobs(std::size_t file_count) {
  if (file_count < 2) {
    return file_count;
  }
  if (const char *configured = std::getenv("LEXICON_CLANG_JOBS")) {
    char *end = nullptr;
    const auto parsed = std::strtoul(configured, &end, 10);
    if (end != configured && *end == '\0' && parsed > 0) {
      return std::min<std::size_t>(parsed, file_count);
    }
  }
  const auto hardware = std::thread::hardware_concurrency();
  const auto available = hardware == 0 ? std::size_t{4}
                                       : static_cast<std::size_t>(hardware);
  return std::min<std::size_t>({std::size_t{8}, available, file_count});
}

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

int run_task(const std::string &root,
             lexicon::clang_frontend::CompilationCommands &database,
             lexicon::clang_frontend::State &state,
             const lexicon::clang_frontend::AnalysisTask &task) {
  const auto absolute =
      (std::filesystem::path(root) /
       std::filesystem::path(task.translation_unit))
          .string();
  auto commands = database.getCompileCommands(absolute);
  if (commands.empty()) {
    return 1;
  }
  const auto &command = commands.front();
  const auto language =
      lexicon::clang_frontend::language_for(task.translation_unit,
                                            command.CommandLine);
  state.translation_units.push_back({
      .path = task.translation_unit,
      .language = language,
      .directory = command.Directory,
      .arguments = command.CommandLine,
      .synthesized = task.synthesized,
  });
  state.file(task.translation_unit, language, task.translation_unit);

  clang::tooling::ClangTool tool(database, {absolute});
  auto factory =
      lexicon::clang_frontend::make_frontend_factory(state, root);
  return tool.run(factory.get());
}

int run_tasks(
    const std::string &root,
    lexicon::clang_frontend::CompilationCommands &database,
    lexicon::clang_frontend::State &state,
    const std::vector<lexicon::clang_frontend::AnalysisTask> &tasks) {
  if (tasks.empty()) {
    return 0;
  }

  const auto jobs = frontend_jobs(tasks.size());
  if (jobs <= 1) {
    int status = 0;
    for (const auto &task : tasks) {
      status |= run_task(root, database, state, task);
    }
    return status;
  }

  std::vector<std::vector<lexicon::clang_frontend::AnalysisTask>> chunks(jobs);
  for (std::size_t index = 0; index < tasks.size(); ++index) {
    chunks[index % jobs].push_back(tasks[index]);
  }

  std::vector<int> statuses(jobs, 0);
  std::vector<std::unique_ptr<lexicon::clang_frontend::State>> results(jobs);
  std::vector<std::thread> workers;
  workers.reserve(jobs);
  for (std::size_t index = 0; index < chunks.size(); ++index) {
    workers.emplace_back([&database, &root, &statuses, &results, index,
                          tasks = std::move(chunks[index])]() mutable {
      auto local = std::make_unique<lexicon::clang_frontend::State>(root);
      for (const auto &task : tasks) {
        statuses[index] |= run_task(root, database, *local, task);
      }
      results[index] = std::move(local);
    });
  }
  for (auto &worker : workers) {
    worker.join();
  }

  int status = 0;
  for (std::size_t index = 0; index < jobs; ++index) {
    status |= statuses[index];
    state.merge(std::move(*results[index]));
  }
  return status;
}

} // namespace

bool emit_structural(const llvm::json::Object &request,
                     llvm::json::Object &response, std::string &error) {
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

  lexicon::clang_frontend::emit_perf(
      "c-family.clang.frontend_plan", std::chrono::nanoseconds(0),
      {{"semantic_tasks", static_cast<std::uint64_t>(plan.tasks.size())},
       {"jobs", static_cast<std::uint64_t>(frontend_jobs(plan.tasks.size()))}});
  const auto frontend_started = lexicon::clang_frontend::PerfClock::now();
  int status = run_tasks(root, database, state, plan.tasks);
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
  response =
      state.response(base != nullptr, clang::getClangFullVersion(),
                     lexicon::clang_frontend::kHelperVersion);
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.observation_emission",
      lexicon::clang_frontend::PerfClock::now() - emission_started,
      {{"observed_files", static_cast<std::uint64_t>(state.files.size())}});
  return true;
}
