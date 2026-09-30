#pragma once

#include <cstddef>
#include <map>
#include <string>
#include <vector>

#include "clang/Tooling/CompilationDatabase.h"
#include "llvm/ADT/StringRef.h"

namespace lexicon::clang_frontend {

class CompilationCommands final : public clang::tooling::CompilationDatabase {
public:
  CompilationCommands(std::string root,
                      const clang::tooling::CompilationDatabase *base);

  std::vector<clang::tooling::CompileCommand>
  getCompileCommands(llvm::StringRef file) const override;
  std::vector<std::string> getAllFiles() const override;

  bool synthesized(llvm::StringRef file) const;
  bool has_real_command(llvm::StringRef file) const;

private:
  std::vector<clang::tooling::CompileCommand>
  real_commands(llvm::StringRef file) const;

  std::string root_;
  std::map<std::string, std::vector<clang::tooling::CompileCommand>>
      exact_commands_;
};

struct AnalysisTask {
  std::string translation_unit;
  std::vector<std::string> owned_files;
  bool synthesized = false;

  bool operator==(const AnalysisTask &) const = default;
};

struct TaskPlan {
  std::vector<AnalysisTask> tasks;
  std::size_t dependency_scan_attempts = 0;
  std::size_t dependency_scan_failures = 0;
  std::size_t synthetic_header_tasks = 0;

  bool operator==(const TaskPlan &) const = default;
};

bool header_source(llvm::StringRef file);
std::string language_for(llvm::StringRef file,
                         const std::vector<std::string> &arguments);

TaskPlan build_task_plan(const std::string &root,
                         CompilationCommands &database,
                         std::vector<std::string> owned_files,
                         std::vector<std::string> context_files);

} // namespace lexicon::clang_frontend
