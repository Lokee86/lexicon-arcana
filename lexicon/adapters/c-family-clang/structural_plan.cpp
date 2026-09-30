#include "structural_plan.h"

#include <algorithm>
#include <cctype>
#include <filesystem>
#include <map>
#include <optional>
#include <set>
#include <tuple>
#include <utility>

#include "clang/Tooling/DependencyScanning/DependencyScanningService.h"
#include "clang/Tooling/DependencyScanning/DependencyScanningTool.h"
#include "llvm/ADT/DenseSet.h"
#include "llvm/Support/Error.h"

namespace lexicon::clang_frontend {
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

clang::tooling::CompileCommand synthetic_command(const std::string &root,
                                                 llvm::StringRef file) {
  clang::tooling::CompileCommand command;
  command.Directory = root;
  command.Filename = file.str();
  const bool cpp = !c_source(file);
  command.CommandLine = {
      "clang",
      "-fsyntax-only",
      cpp ? "-xc++" : "-xc",
      cpp ? "-std=c++17" : "-std=c11",
      "-I",
      root,
  };
  const auto conventional_include = std::filesystem::path(root) / "include";
  if (std::filesystem::is_directory(conventional_include)) {
    command.CommandLine.push_back("-I");
    command.CommandLine.push_back(conventional_include.string());
  }
  command.CommandLine.push_back(file.str());
  return command;
}

std::string absolute_file(const std::string &root, llvm::StringRef relative) {
  return (std::filesystem::path(root) / std::filesystem::path(relative.str()))
      .lexically_normal()
      .string();
}

std::optional<std::string>
repository_relative_dependency(const std::string &root,
                               const clang::tooling::CompileCommand &command,
                               llvm::StringRef dependency) {
  if (dependency.empty()) {
    return std::nullopt;
  }
  std::error_code error;
  auto root_path = std::filesystem::weakly_canonical(root, error);
  if (error) {
    return std::nullopt;
  }

  auto path = std::filesystem::path(dependency.str());
  if (path.is_relative()) {
    auto directory = std::filesystem::path(command.Directory);
    if (directory.is_relative()) {
      directory = root_path / directory;
    }
    path = directory / path;
  }
  path = std::filesystem::weakly_canonical(path, error);
  if (error) {
    return std::nullopt;
  }
  auto relative = std::filesystem::relative(path, root_path, error);
  if (error || relative.empty()) {
    return std::nullopt;
  }
  const auto text = relative.lexically_normal().generic_string();
  if (text == ".." || text.starts_with("../")) {
    return std::nullopt;
  }
  return text;
}

struct DependencyCandidate {
  std::string translation_unit;
  bool synthesized = false;
  std::set<std::string> dependencies;
};

std::optional<DependencyCandidate>
scan_candidate(const std::string &root, CompilationCommands &database,
               clang::tooling::dependencies::DependencyScanningTool &scanner,
               llvm::StringRef translation_unit) {
  const auto absolute = absolute_file(root, translation_unit);
  auto commands = database.getCompileCommands(absolute);
  if (commands.empty()) {
    return std::nullopt;
  }
  const auto &command = commands.front();

  llvm::DenseSet<clang::tooling::dependencies::ModuleID> already_seen;
  auto dependencies = scanner.getTranslationUnitDependencies(
      command.CommandLine, command.Directory, already_seen,
      [](const clang::tooling::dependencies::ModuleID &,
         clang::tooling::dependencies::ModuleOutputKind) {
        return std::string();
      });
  if (!dependencies) {
    llvm::consumeError(dependencies.takeError());
    return std::nullopt;
  }

  DependencyCandidate candidate{
      .translation_unit = translation_unit.str(),
      .synthesized = database.synthesized(absolute),
  };
  for (const auto &dependency : dependencies->FileDeps) {
    if (auto relative =
            repository_relative_dependency(root, command, dependency)) {
      candidate.dependencies.insert(std::move(*relative));
    }
  }
  return candidate;
}

AnalysisTask &ensure_task(std::map<std::string, AnalysisTask> &tasks,
                          const std::string &root,
                          CompilationCommands &database,
                          const std::string &translation_unit) {
  auto [entry, inserted] = tasks.try_emplace(
      translation_unit,
      AnalysisTask{
          .translation_unit = translation_unit,
          .synthesized =
              database.synthesized(absolute_file(root, translation_unit)),
      });
  return entry->second;
}

void own(AnalysisTask &task, const std::string &path) {
  task.owned_files.push_back(path);
}

} // namespace

CompilationCommands::CompilationCommands(
    std::string root, const clang::tooling::CompilationDatabase *base)
    : root_(std::move(root)) {
  if (!base) {
    return;
  }
  for (auto command : base->getAllCompileCommands()) {
    auto directory = std::filesystem::path(command.Directory);
    if (directory.is_relative()) {
      directory = std::filesystem::path(root_) / directory;
    }
    auto file = std::filesystem::path(command.Filename);
    if (file.is_relative()) {
      file = directory / file;
    }
    exact_commands_[file.lexically_normal().string()].push_back(
        std::move(command));
  }
}

std::vector<clang::tooling::CompileCommand>
CompilationCommands::getCompileCommands(llvm::StringRef file) const {
  auto commands = real_commands(file);
  if (commands.empty()) {
    return {synthetic_command(root_, file)};
  }
  std::sort(commands.begin(), commands.end(),
            [](const auto &left, const auto &right) {
              return std::tie(left.Directory, left.Filename, left.CommandLine) <
                     std::tie(right.Directory, right.Filename,
                              right.CommandLine);
            });
  commands.resize(1);
  return commands;
}

std::vector<std::string> CompilationCommands::getAllFiles() const {
  std::vector<std::string> files;
  files.reserve(exact_commands_.size());
  for (const auto &[path, _] : exact_commands_) {
    files.push_back(path);
  }
  return files;
}

bool CompilationCommands::synthesized(llvm::StringRef file) const {
  return !has_real_command(file);
}

bool CompilationCommands::has_real_command(llvm::StringRef file) const {
  return !real_commands(file).empty();
}

std::vector<clang::tooling::CompileCommand>
CompilationCommands::real_commands(llvm::StringRef file) const {
  auto path = std::filesystem::path(file.str());
  if (path.is_relative()) {
    path = std::filesystem::path(root_) / path;
  }
  const auto found = exact_commands_.find(path.lexically_normal().string());
  return found == exact_commands_.end()
             ? std::vector<clang::tooling::CompileCommand>{}
             : found->second;
}

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

TaskPlan build_task_plan(const std::string &root,
                         CompilationCommands &database,
                         std::vector<std::string> owned_files,
                         std::vector<std::string> context_files) {
  std::sort(owned_files.begin(), owned_files.end());
  owned_files.erase(std::unique(owned_files.begin(), owned_files.end()),
                    owned_files.end());
  std::sort(context_files.begin(), context_files.end());
  context_files.erase(std::unique(context_files.begin(), context_files.end()),
                      context_files.end());

  std::map<std::string, AnalysisTask> tasks;
  std::vector<std::string> owned_headers;
  std::vector<std::string> candidate_sources;

  for (const auto &path : owned_files) {
    if (header_source(path)) {
      owned_headers.push_back(path);
    } else {
      own(ensure_task(tasks, root, database, path), path);
      candidate_sources.push_back(path);
    }
  }
  for (const auto &path : context_files) {
    if (!header_source(path)) {
      candidate_sources.push_back(path);
    }
  }
  std::sort(candidate_sources.begin(), candidate_sources.end());
  candidate_sources.erase(
      std::unique(candidate_sources.begin(), candidate_sources.end()),
      candidate_sources.end());

  std::vector<std::string> pending_headers;
  for (const auto &header : owned_headers) {
    if (database.has_real_command(absolute_file(root, header))) {
      own(ensure_task(tasks, root, database, header), header);
    } else {
      pending_headers.push_back(header);
    }
  }

  TaskPlan plan;
  if (!pending_headers.empty() && !candidate_sources.empty()) {
    using namespace clang::tooling::dependencies;
    DependencyScanningService service(ScanningMode::DependencyDirectivesScan,
                                      ScanningOutputFormat::Full);
    DependencyScanningTool scanner(service);
    std::vector<DependencyCandidate> candidates;
    candidates.reserve(candidate_sources.size());
    for (const auto &source : candidate_sources) {
      ++plan.dependency_scan_attempts;
      if (auto candidate = scan_candidate(root, database, scanner, source)) {
        candidates.push_back(std::move(*candidate));
      } else {
        ++plan.dependency_scan_failures;
      }
    }

    for (const auto &header : pending_headers) {
      const DependencyCandidate *selected = nullptr;
      for (const auto &candidate : candidates) {
        if (!candidate.dependencies.contains(header)) {
          continue;
        }
        if (!selected ||
            std::tie(candidate.synthesized, candidate.translation_unit) <
                std::tie(selected->synthesized, selected->translation_unit)) {
          selected = &candidate;
        }
      }
      if (selected) {
        own(ensure_task(tasks, root, database, selected->translation_unit),
            header);
      } else {
        auto &task = ensure_task(tasks, root, database, header);
        task.synthesized = true;
        own(task, header);
        ++plan.synthetic_header_tasks;
      }
    }
  } else {
    for (const auto &header : pending_headers) {
      auto &task = ensure_task(tasks, root, database, header);
      task.synthesized = true;
      own(task, header);
      ++plan.synthetic_header_tasks;
    }
  }

  for (auto &[_, task] : tasks) {
    std::sort(task.owned_files.begin(), task.owned_files.end());
    task.owned_files.erase(
        std::unique(task.owned_files.begin(), task.owned_files.end()),
        task.owned_files.end());
    plan.tasks.push_back(std::move(task));
  }
  return plan;
}

} // namespace lexicon::clang_frontend
