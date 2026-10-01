#include "structural_compilation.h"

#include <algorithm>
#include <filesystem>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

namespace {

class FixtureDatabase final : public clang::tooling::CompilationDatabase {
public:
  explicit FixtureDatabase(std::string root) : root_(std::move(root)) {}

  void add(std::string file, std::vector<std::string> arguments,
           std::string output = {}) {
    clang::tooling::CompileCommand command;
    command.Directory = ".";
    command.Filename = std::move(file);
    command.CommandLine = std::move(arguments);
    command.Output = std::move(output);
    commands_.push_back(std::move(command));
  }

  std::vector<clang::tooling::CompileCommand>
  getCompileCommands(llvm::StringRef file) const override {
    std::vector<clang::tooling::CompileCommand> result;
    const auto requested =
        std::filesystem::path(file.str()).lexically_normal().string();
    for (const auto &command : commands_) {
      auto directory = std::filesystem::path(command.Directory);
      if (directory.is_relative()) {
        directory = std::filesystem::path(root_) / directory;
      }
      auto filename = std::filesystem::path(command.Filename);
      if (filename.is_relative()) {
        filename = directory / filename;
      }
      if (std::filesystem::absolute(filename).lexically_normal().string() ==
          requested) {
        result.push_back(command);
      }
    }
    return result;
  }

  std::vector<std::string> getAllFiles() const override {
    std::vector<std::string> result;
    result.reserve(commands_.size());
    for (const auto &command : commands_) {
      auto directory = std::filesystem::path(command.Directory);
      if (directory.is_relative()) {
        directory = std::filesystem::path(root_) / directory;
      }
      auto filename = std::filesystem::path(command.Filename);
      if (filename.is_relative()) {
        filename = directory / filename;
      }
      result.push_back(
          std::filesystem::absolute(filename).lexically_normal().string());
    }
    return result;
  }

private:
  std::string root_;
  std::vector<clang::tooling::CompileCommand> commands_;
};

void require(bool condition, const std::string &message) {
  if (!condition) {
    throw std::runtime_error(message);
  }
}

std::vector<std::string>
translation_units(const lexicon::clang_frontend::ParsePlan &plan) {
  std::vector<std::string> result;
  for (const auto &unit : plan.primary_units) {
    result.push_back(unit.translation_unit);
  }
  return result;
}

} // namespace

int main() {
  using namespace lexicon::clang_frontend;

  const auto root =
      (std::filesystem::current_path() / ".structural-compilation-fixture")
          .string();
  FixtureDatabase base(root);
  // Deliberately add duplicate commands out of lexical order. The adapter
  // must choose one stable exact command for the header.
  base.add("exact.h", {"clang", "-DSELECT_Z", "exact.h"});
  base.add("exact.h", {"clang", "-DSELECT_A", "exact.h"});
  base.add("b.c", {"clang", "-c", "b.c"});
  base.add("ctx/a.cpp", {"clang++", "-c", "ctx/a.cpp"});
  base.add("output.c", {"clang", "-c", "output.c"}, "z.o");
  base.add("output.c", {"clang", "-c", "output.c"}, "a.o");

  CompilationCommands commands(root, &base);
  const auto first = build_parse_plan(
      root, commands, {"z.cpp", "orphan.hpp", "b.c", "exact.h", "orphan.hpp"},
      {"ctx/z.c", "ctx/a.cpp", "ctx/a.cpp"});
  const auto reordered = build_parse_plan(
      root, commands, {"exact.h", "b.c", "orphan.hpp", "z.cpp"},
      {"ctx/a.cpp", "ctx/z.c"});

  require(first == reordered, "parse plan depends on input ordering");
  require(translation_units(first) ==
              std::vector<std::string>{"exact.h", "b.c", "ctx/a.cpp", "ctx/z.c",
                                       "z.cpp"},
          "parse units are not ordered exact-header, real-source, "
          "synthetic-source");
  require(first.primary_units.front().rank == 0 &&
              first.primary_units.front().translation_unit == "exact.h" &&
              !first.primary_units.front().synthesized,
          "exact header compile unit was not selected first");
  require(first.primary_units[1].rank == 1 &&
              first.primary_units[1].translation_unit == "b.c" &&
              !first.primary_units[1].synthesized,
          "real source order or rank is incorrect");
  require(first.primary_units[2].rank == 2 &&
              first.primary_units[2].translation_unit == "ctx/a.cpp" &&
              !first.primary_units[2].synthesized,
          "real source did not precede synthetic sources");
  require(first.primary_units[3].synthesized &&
              first.primary_units[4].synthesized,
          "sources without exact commands were not marked synthetic");
  require(first.orphan_candidates ==
              std::vector<std::string>{"exact.h", "orphan.hpp"},
          "owned headers were not retained as orphan candidates");
  const auto primary_paths = translation_units(first);
  require(std::find(primary_paths.begin(), primary_paths.end(), "orphan.hpp") ==
              primary_paths.end(),
          "ordinary owned header was scheduled as a primary unit");
  require(first.real_units == 3 && first.synthetic_units == 2 &&
              first.explicit_header_units == 1,
          "parse-unit summary counts are incorrect");

  const auto selected = commands.getCompileCommands(
      (std::filesystem::path(root) / "exact.h").string());
  require(selected.size() == 1 &&
              std::find(selected.front().CommandLine.begin(),
                        selected.front().CommandLine.end(),
                        "-DSELECT_A") != selected.front().CommandLine.end(),
          "exact compile-command selection is not deterministic");
  const auto output_tie = commands.getCompileCommands(
      (std::filesystem::path(root) / "output.c").string());
  require(output_tie.size() == 1,
          "output tie-break did not select exactly one compile command");
  require(std::filesystem::path(output_tie.front().Directory)
                  .lexically_normal() ==
              std::filesystem::path(root).lexically_normal(),
          "relative compilation database directory was not normalized: got " +
              output_tie.front().Directory + " expected " +
              std::filesystem::path(root).string());
  require(output_tie.front().Filename ==
              (std::filesystem::path(root) / "output.c").string(),
          "relative compilation database filename was not normalized");
  require(output_tie.front().Output ==
              (std::filesystem::path(root) / "a.o").string(),
          "compile-command output tie-break is unstable");

  FixtureDatabase reversed_base(root);
  reversed_base.add("output.c", {"clang", "-c", "output.c"}, "a.o");
  reversed_base.add("output.c", {"clang", "-c", "output.c"}, "z.o");
  reversed_base.add("exact.h", {"clang", "-DSELECT_A", "exact.h"});
  reversed_base.add("exact.h", {"clang", "-DSELECT_Z", "exact.h"});
  reversed_base.add("ctx/a.cpp", {"clang++", "-c", "ctx/a.cpp"});
  reversed_base.add("b.c", {"clang", "-c", "b.c"});
  CompilationCommands reversed_commands(root, &reversed_base);
  const auto reversed_output_tie = reversed_commands.getCompileCommands(
      (std::filesystem::path(root) / "output.c").string());
  const auto reversed_exact = reversed_commands.getCompileCommands(
      (std::filesystem::path(root) / "exact.h").string());
  require(reversed_output_tie.size() == 1 &&
              reversed_output_tie.front().Output == output_tie.front().Output &&
              reversed_exact.size() == 1 &&
              reversed_exact.front().CommandLine ==
                  selected.front().CommandLine,
          "compile-command selection changed with database entry ordering");

  const auto synthetic_c = commands.getCompileCommands(
      (std::filesystem::path(root) / "generated.c").string());
  const auto synthetic_cpp = commands.getCompileCommands(
      (std::filesystem::path(root) / "generated.cpp").string());
  const auto repeated_synthetic_c = commands.getCompileCommands(
      (std::filesystem::path(root) / "generated.c").string());
  require(synthetic_c.size() == 1 && synthetic_cpp.size() == 1 &&
              repeated_synthetic_c.size() == 1 &&
              synthetic_c.front().Directory ==
                  repeated_synthetic_c.front().Directory &&
              synthetic_c.front().Filename ==
                  repeated_synthetic_c.front().Filename &&
              synthetic_c.front().CommandLine ==
                  repeated_synthetic_c.front().CommandLine &&
              std::find(synthetic_c.front().CommandLine.begin(),
                        synthetic_c.front().CommandLine.end(),
                        "-xc") != synthetic_c.front().CommandLine.end() &&
              std::find(synthetic_c.front().CommandLine.begin(),
                        synthetic_c.front().CommandLine.end(),
                        "-std=c11") != synthetic_c.front().CommandLine.end() &&
              std::find(synthetic_cpp.front().CommandLine.begin(),
                        synthetic_cpp.front().CommandLine.end(),
                        "-xc++") != synthetic_cpp.front().CommandLine.end() &&
              std::find(synthetic_cpp.front().CommandLine.begin(),
                        synthetic_cpp.front().CommandLine.end(),
                        "-std=c++17") !=
                  synthetic_cpp.front().CommandLine.end(),
          "synthetic source commands do not select deterministic languages");

  const auto changed_source_only =
      build_parse_plan(root, commands, {"changed.c"}, {"ctx/a.cpp", "b.c"});
  require(translation_units(changed_source_only) ==
              std::vector<std::string>{"changed.c"},
          "context sources entered a source-only incremental parse plan");
  const auto changed_header =
      build_parse_plan(root, commands, {"changed.h"},
                       {"ctx/z.c", "ctx/a.cpp", "include/context.hpp"});
  require(
      translation_units(changed_header) ==
          std::vector<std::string>{"ctx/a.cpp", "ctx/z.c"},
      "changed-header candidates are not real-before-synthetic and canonical");
  require(changed_header.primary_units[0].rank == 0 &&
              !changed_header.primary_units[0].synthesized &&
              changed_header.primary_units[1].rank == 1 &&
              changed_header.primary_units[1].synthesized,
          "changed-header candidate classes or ranks are incorrect");
  require(changed_header.explicit_header_units == 0 &&
              changed_header.orphan_candidates ==
                  std::vector<std::string>{"changed.h"},
          "ordinary changed header was not held for orphan fallback");
  const auto context_only =
      build_parse_plan(root, commands, {}, {"ctx/a.cpp", "ctx/z.c"});
  require(context_only.primary_units.empty() &&
              context_only.orphan_candidates.empty() &&
              context_only.real_units == 0 &&
              context_only.synthetic_units == 0 &&
              context_only.explicit_header_units == 0,
          "context-only inventory produced primary parse units");

  return 0;
}
