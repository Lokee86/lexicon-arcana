#include "structural_plan.h"

#include <algorithm>
#include <filesystem>
#include <fstream>
#include <stdexcept>
#include <string>
#include <vector>

#include "clang/Tooling/CompilationDatabase.h"

namespace {

std::filesystem::path make_fixture() {
  const auto root = std::filesystem::temp_directory_path() /
                    "lexicon-c-family-plan-test";
  std::error_code error;
  std::filesystem::remove_all(root, error);
  std::filesystem::create_directories(root);

  auto write = [&](const char *name, const char *contents) {
    std::ofstream(root / name) << contents;
  };
  write("a.c", "#include \"shared.h\"\n#include \"exact.h\"\n");
  write("b.c", "#include \"shared.h\"\n#include \"exact.h\"\n");
  write("c.c", "#include \"shared.h\"\n");
  write("shared.h", "#pragma once\n");
  write("exact.h", "#pragma once\n");
  write("orphan.h", "#pragma once\n");

  std::ofstream database(root / "compile_commands.json");
  database
      << "["
      << "{\"directory\":\"" << root.generic_string()
      << "\",\"arguments\":[\"clang\",\"-I\",\"" << root.generic_string()
      << "\",\"-c\",\"b.c\"],\"file\":\"b.c\"},"
      << "{\"directory\":\"" << root.generic_string()
      << "\",\"arguments\":[\"clang\",\"-I\",\"" << root.generic_string()
      << "\",\"-c\",\"c.c\"],\"file\":\"c.c\"},"
      << "{\"directory\":\"" << root.generic_string()
      << "\",\"arguments\":[\"clang\",\"-xc\",\"-I\",\""
      << root.generic_string()
      << "\",\"-c\",\"exact.h\"],\"file\":\"exact.h\"}"
      << "]";
  return root;
}

const lexicon::clang_frontend::AnalysisTask &
owner_task(const lexicon::clang_frontend::TaskPlan &plan,
           const std::string &owned) {
  const auto found = std::find_if(
      plan.tasks.begin(), plan.tasks.end(), [&](const auto &task) {
        return std::find(task.owned_files.begin(), task.owned_files.end(),
                         owned) != task.owned_files.end();
      });
  if (found == plan.tasks.end()) {
    throw std::runtime_error("owned file missing from task plan: " + owned);
  }
  return *found;
}

} // namespace

int main() {
  using namespace lexicon::clang_frontend;

  const auto root = make_fixture();
  std::string error;
  auto base = clang::tooling::CompilationDatabase::autoDetectFromDirectory(
      root.string(), error);
  if (!base) {
    throw std::runtime_error("failed to load compilation database: " + error);
  }

  CompilationCommands commands(root.string(), base.get());
  const std::vector<std::string> owned{"orphan.h", "shared.h", "exact.h"};
  const std::vector<std::string> context{"c.c", "a.c", "b.c"};

  const auto first = build_task_plan(root.string(), commands, owned, context);
  const auto second = build_task_plan(
      root.string(), commands,
      std::vector<std::string>{"exact.h", "shared.h", "orphan.h"},
      std::vector<std::string>{"b.c", "c.c", "a.c"});

  if (!(first == second)) {
    throw std::runtime_error("task plan changed under input reordering");
  }
  if (owner_task(first, "exact.h").translation_unit != "exact.h") {
    throw std::runtime_error("exact header compile command was not preferred");
  }
  if (owner_task(first, "exact.h").synthesized) {
    throw std::runtime_error("exact header task was marked synthetic");
  }

  // a.c is lexically first but has no compilation-database command. The
  // canonical real includer is therefore b.c, ahead of c.c.
  const auto &shared_owner = owner_task(first, "shared.h");
  if (shared_owner.translation_unit != "b.c") {
    throw std::runtime_error(
        "canonical real includer was not selected; got " +
        shared_owner.translation_unit +
        (shared_owner.synthesized ? " (synthetic)" : " (real)"));
  }
  if (owner_task(first, "shared.h").synthesized) {
    throw std::runtime_error("real includer task was marked synthetic");
  }

  if (owner_task(first, "orphan.h").translation_unit != "orphan.h") {
    throw std::runtime_error("orphan header did not receive direct fallback");
  }
  if (!owner_task(first, "orphan.h").synthesized) {
    throw std::runtime_error("orphan header fallback was not synthetic");
  }
  if (first.synthetic_header_tasks != 1) {
    throw std::runtime_error("unexpected synthetic header task count");
  }

  std::error_code cleanup_error;
  std::filesystem::remove_all(root, cleanup_error);
  return 0;
}
