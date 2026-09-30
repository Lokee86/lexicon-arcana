#include "structural.h"

#include <algorithm>
#include <cctype>
#include <cstdlib>
#include <filesystem>
#include <limits>
#include <memory>
#include <mutex>
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

namespace {

std::string extension(llvm::StringRef file) {
  auto value = std::filesystem::path(file.str()).extension().string();
  if (value == ".C") {
    return "C";
  }
  std::transform(value.begin(), value.end(), value.begin(),
                 [](unsigned char ch) { return std::tolower(ch); });
  if (!value.empty() && value.front() == '.') {
    value.erase(value.begin());
  }
  return value;
}

bool c_source(llvm::StringRef file) { return extension(file) == "c"; }

bool header_source(llvm::StringRef file) {
  const auto value = extension(file);
  return value == "h" || value == "h++" || value == "hh" ||
         value == "hpp" || value == "hxx" || value == "inc" ||
         value == "inl" || value == "ipp" || value == "tpp";
}

std::string language_for(llvm::StringRef file,
                         const std::vector<std::string> &arguments) {
  std::string joined;
  for (const auto &argument : arguments) {
    if (!joined.empty()) {
      joined.push_back(' ');
    }
    joined += argument;
  }
  std::transform(joined.begin(), joined.end(), joined.begin(),
                 [](unsigned char value) { return std::tolower(value); });
  if (joined.find("-x c++") != std::string::npos ||
      joined.find("-xc++") != std::string::npos ||
      joined.find("clang++") != std::string::npos ||
      joined.find("g++") != std::string::npos) {
    return "cpp";
  }
  if (joined.find("-x c ") != std::string::npos ||
      joined.find("-xc ") != std::string::npos ||
      joined.ends_with("-x c") || joined.ends_with("-xc")) {
    return "c";
  }
  return c_source(file) ? "c" : "cpp";
}

clang::tooling::CompileCommand synthetic_command(const std::string &root,
                                                 llvm::StringRef file,
                                                 bool cpp) {
  clang::tooling::CompileCommand command;
  command.Directory = root;
  command.Filename = file.str();
  command.CommandLine = {
      "clang",
      "-fsyntax-only",
      cpp ? "-xc++" : "-xc",
      cpp ? "-std=c++17" : "-std=c11",
      "-I",
      root,
  };
  const auto root_path = std::filesystem::path(root);
  const auto conventional_include = root_path / "include";
  if (std::filesystem::is_directory(conventional_include)) {
    command.CommandLine.push_back("-I");
    command.CommandLine.push_back(conventional_include.string());
  }
  const auto parent = root_path.parent_path();
  if (parent.filename() == "include" || parent.filename() == "inc") {
    command.CommandLine.push_back("-I");
    command.CommandLine.push_back(parent.string());
  }
  command.CommandLine.push_back(file.str());
  return command;
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

class CompilationCommands final : public clang::tooling::CompilationDatabase {
public:
  CompilationCommands(std::string root,
                      const clang::tooling::CompilationDatabase *base)
      : root_(std::move(root)), base_(base) {}

  std::vector<clang::tooling::CompileCommand>
  getCompileCommands(llvm::StringRef file) const override {
    auto commands = base_commands(file);
    if (!commands.empty()) {
      return commands;
    }
    return {synthetic_command(root_, file, !c_source(file))};
  }

  std::vector<std::string> getAllFiles() const override {
    std::scoped_lock lock(base_mutex_);
    return base_ ? base_->getAllFiles() : std::vector<std::string>{};
  }

  bool synthesized(llvm::StringRef file) const {
    return base_commands(file).empty();
  }

private:
  std::vector<clang::tooling::CompileCommand>
  base_commands(llvm::StringRef file) const {
    std::scoped_lock lock(base_mutex_);
    if (!base_) {
      return {};
    }
    auto commands = base_->getCompileCommands(file);
    if (!commands.empty()) {
      return commands;
    }

    std::error_code error;
    auto relative = std::filesystem::relative(
        std::filesystem::path(file.str()), std::filesystem::path(root_), error);
    if (error || relative.empty()) {
      return {};
    }
    const auto normalized = relative.generic_string();
    if (normalized == ".." || normalized.starts_with("../")) {
      return {};
    }
    commands = base_->getCompileCommands(normalized);
    if (!commands.empty()) {
      return commands;
    }

    const auto requested =
        std::filesystem::path(file.str()).lexically_normal();
    std::vector<clang::tooling::CompileCommand> matched;
    for (const auto &command : base_->getAllCompileCommands()) {
      auto directory = std::filesystem::path(command.Directory);
      if (directory.is_relative()) {
        directory = std::filesystem::path(root_) / directory;
      }
      auto command_file = std::filesystem::path(command.Filename);
      if (command_file.is_relative()) {
        command_file = directory / command_file;
      }
      if (command_file.lexically_normal() == requested) {
        matched.push_back(command);
      }
    }
    return matched;
  }

  std::string root_;
  const clang::tooling::CompilationDatabase *base_;
  mutable std::mutex base_mutex_;
};

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

struct StructuralInput {
  std::string root;
  std::vector<std::string> owned_files;
  std::vector<std::string> context_files;
  std::size_t workers = 0;
  std::size_t shards = 0;
  std::size_t merge_fan_in = 0;

  std::vector<std::string> analysis_files() const {
    auto files = owned_files;
    files.insert(files.end(), context_files.begin(), context_files.end());
    std::sort(files.begin(), files.end());
    files.erase(std::unique(files.begin(), files.end()), files.end());
    return files;
  }
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

int run_batch(const std::string &root, CompilationCommands &database,
              lexicon::clang_frontend::State &state,
              const std::vector<std::string> &files) {
  if (files.empty()) {
    return 0;
  }

  std::vector<std::string> absolute_files;
  absolute_files.reserve(files.size());
  for (const auto &file : files) {
    const auto absolute =
        (std::filesystem::path(root) / std::filesystem::path(file)).string();
    absolute_files.push_back(absolute);
    for (const auto &command : database.getCompileCommands(absolute)) {
      const auto language = language_for(file, command.CommandLine);
      state.translation_units.push_back({
          .path = file,
          .language = language,
          .directory = command.Directory,
          .arguments = command.CommandLine,
          .synthesized = database.synthesized(absolute),
      });
      state.file(file, language, file);
    }
  }

  const auto jobs = frontend_jobs(absolute_files.size());
  if (jobs <= 1) {
    clang::tooling::ClangTool tool(database, absolute_files);
    auto factory = lexicon::clang_frontend::make_frontend_factory(state, root);
    return tool.run(factory.get());
  }

  std::vector<std::vector<std::string>> chunks(jobs);
  for (std::size_t index = 0; index < absolute_files.size(); ++index) {
    chunks[index % jobs].push_back(std::move(absolute_files[index]));
  }

  std::vector<int> statuses(jobs, 0);
  std::vector<std::unique_ptr<lexicon::clang_frontend::State>> results(jobs);
  std::vector<std::thread> workers;
  workers.reserve(jobs);
  for (std::size_t index = 0; index < chunks.size(); ++index) {
    workers.emplace_back([&database, &root, &statuses, &results, index,
                          files = std::move(chunks[index])]() mutable {
      auto local = std::make_unique<lexicon::clang_frontend::State>(root);
      clang::tooling::ClangTool tool(database, files);
      auto factory =
          lexicon::clang_frontend::make_frontend_factory(*local, root);
      statuses[index] = tool.run(factory.get());
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
  const auto files = input.analysis_files();

  std::string database_error;
  const auto database_started = lexicon::clang_frontend::PerfClock::now();
  auto base = clang::tooling::CompilationDatabase::autoDetectFromDirectory(
      root, database_error);
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.compilation_database",
      lexicon::clang_frontend::PerfClock::now() - database_started);
  CompilationCommands database(root, base.get());
  lexicon::clang_frontend::State state(root);

  std::vector<std::string> sources;
  std::vector<std::string> headers;
  for (const auto &file : files) {
    (header_source(file) ? headers : sources).push_back(file);
  }

  lexicon::clang_frontend::emit_perf(
      "c-family.clang.frontend_plan", std::chrono::nanoseconds(0),
      {{"source_translation_units",
        static_cast<std::uint64_t>(sources.size())},
       {"headers", static_cast<std::uint64_t>(headers.size())},
       {"jobs", static_cast<std::uint64_t>(frontend_jobs(sources.size()))}});
  const auto frontend_started = lexicon::clang_frontend::PerfClock::now();
  int status = run_batch(root, database, state, sources);
  std::uint64_t orphan_header_batches = 0;
  std::uint64_t directly_analyzed_headers = 0;
  while (true) {
    std::size_t minimum_depth = std::numeric_limits<std::size_t>::max();
    for (const auto &header : headers) {
      if (state.files.contains(header)) {
        continue;
      }
      const auto depth =
          static_cast<std::size_t>(std::count(header.begin(), header.end(), '/'));
      minimum_depth = std::min(minimum_depth, depth);
    }
    if (minimum_depth == std::numeric_limits<std::size_t>::max()) {
      break;
    }

    std::vector<std::string> orphan_headers;
    for (const auto &header : headers) {
      if (state.files.contains(header)) {
        continue;
      }
      const auto depth =
          static_cast<std::size_t>(std::count(header.begin(), header.end(), '/'));
      if (depth == minimum_depth) {
        orphan_headers.push_back(header);
      }
    }
    if (orphan_headers.empty()) {
      break;
    }
    ++orphan_header_batches;
    directly_analyzed_headers += orphan_headers.size();
    status |= run_batch(root, database, state, orphan_headers);
  }
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.orphan_headers", std::chrono::nanoseconds(0),
      {{"batches", orphan_header_batches},
       {"direct_headers", directly_analyzed_headers}});
  lexicon::clang_frontend::emit_perf(
      "c-family.clang.frontend_work",
      lexicon::clang_frontend::PerfClock::now() - frontend_started,
      {{"translation_units",
        static_cast<std::uint64_t>(state.translation_units.size())},
       {"input_files", static_cast<std::uint64_t>(files.size())}});
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
