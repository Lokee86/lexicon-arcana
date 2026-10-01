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

struct ParseUnit {
  std::size_t rank = 0;
  std::string translation_unit;
  bool synthesized = false;

  bool operator==(const ParseUnit &) const = default;
};

struct ParsePlan {
  std::vector<ParseUnit> primary_units;
  std::vector<std::string> orphan_candidates;
  std::size_t real_units = 0;
  std::size_t synthetic_units = 0;
  std::size_t explicit_header_units = 0;

  bool operator==(const ParsePlan &) const = default;
};

bool header_source(llvm::StringRef file);
std::string language_for(llvm::StringRef file,
                         const std::vector<std::string> &arguments);

ParsePlan build_parse_plan(const std::string &root,
                           CompilationCommands &database,
                           std::vector<std::string> owned_files,
                           std::vector<std::string> context_files);

} // namespace lexicon::clang_frontend
