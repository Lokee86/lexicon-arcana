#include "structural_commit.h"

#include <stdexcept>
#include <string>
#include <utility>

namespace {
using namespace lexicon::clang_frontend;

void require(bool condition, const char *message) {
  if (!condition) {
    throw std::runtime_error(message);
  }
}

State observation(bool synthesized = false, std::string language = "c") {
  State state("/repo");
  state.translation_units.push_back({.path = "unity.c",
                                    .language = language,
                                    .synthesized = synthesized});
  auto &file = state.file("part.c", language, "unity.c");
  file.declarations.push_back({.compiler_id = "part", .name = "part"});
  return state;
}

bool covered(State state, int status = 0) {
  OrderedObservationCommitter committer("/repo", {"part.c"}, 1, 1, {});
  committer.submit(0, std::move(state), status);
  return committer.valid_real_coverage().contains({"part.c", "c"});
}
} // namespace

void staged_source_coverage_tests() {
  require(covered(observation()), "valid real source observations lost coverage");
  require(!covered(observation(true)), "synthetic TU incorrectly supplied real coverage");
  require(!covered(observation(), 1), "failed frontend supplied coverage");
  require(!covered(observation(false, "cpp")), "C++ evidence counted as C coverage");
  auto recovery = observation();
  recovery.add_diagnostic({.severity = "error", .message = "invalid AST"});
  require(!covered(std::move(recovery)), "recovery AST supplied coverage");
  auto fatal = observation();
  fatal.add_diagnostic({.severity = "fatal", .message = "missing include"});
  require(!covered(std::move(fatal)), "fatal diagnostic supplied coverage");
  auto warning = observation();
  warning.add_diagnostic({.severity = "warning", .message = "warning only"});
  require(covered(std::move(warning)), "warning-only real context lost coverage");

  auto empty = observation();
  empty.files.at("part.c").declarations.clear();
  require(!covered(std::move(empty)), "empty file entry supplied semantic coverage");
  auto macro = observation();
  macro.files.at("part.c").declarations.clear();
  macro.files.at("part.c").macros.push_back({.name = "PART"});
  require(covered(std::move(macro)), "macro-only source lost semantic coverage");

  OrderedObservationCommitter claims("/repo", {"part.c"}, 2, 2, {});
  auto first = observation();
  first.add_diagnostic({.severity = "error", .message = "first owner failed"});
  claims.submit(1, observation());
  claims.submit(0, std::move(first));
  require(claims.valid_real_coverage().empty(),
          "later duplicate upgraded an invalid canonical observation");

  std::size_t emitted = 0;
  OrderedObservationCommitter next(
      "/repo", {"part.c", "other.c"}, 1, 1,
      [&](std::size_t, State &&files) {
        emitted += files.files.size();
        require(!files.files.contains("part.c"), "prior claim emitted twice");
      },
      {"part.c"});
  auto fallback = observation(true);
  fallback.file("other.c", "c", "other.c");
  next.submit(0, std::move(fallback));
  require(emitted == 1 && next.summary().claimed_owned_files == 1 &&
              next.summary().discarded_duplicate_file_observations == 1 &&
              next.claimed_files() == std::vector<std::string>{"other.c", "part.c"},
          "phase transition did not preserve global claims and new-claim counts");
}
