#include "structural_commit.h"

#include <algorithm>
#include <array>
#include <chrono>
#include <future>
#include <stdexcept>
#include <string>
#include <thread>
#include <utility>
#include <vector>

namespace {

using lexicon::clang_frontend::ContextIdentity;
using lexicon::clang_frontend::Diagnostic;
using lexicon::clang_frontend::OrderedObservationCommitter;
using lexicon::clang_frontend::State;

void require(bool condition, const std::string &message) {
  if (!condition) {
    throw std::runtime_error(message);
  }
}

State result_for(std::size_t rank) {
  State state("/repo");
  const auto tu = "src" + std::to_string(rank) + ".c";
  state.translation_units.push_back(
      {.path = tu, .language = "c", .directory = "/repo"});
  auto &source = state.file(tu, "c", tu);
  source.declaration_compiler_ids.insert("decl-" + std::to_string(rank));
  state.file("shared.h", "c", tu);
  state.file("context-only.h", "c", tu);

  if (rank == 0 || rank == 2) {
    state.context_identities.push_back({.compiler_id = "symbol",
                                        .path = "context.h",
                                        .kind = "function",
                                        .qualified_name = "api",
                                        .signature = "int api()",
                                        .definition = rank == 2});
  }
  state.context_identities.push_back({.compiler_id = "tie",
                                      .path = "context.h",
                                      .kind = rank == 0   ? "z-kind"
                                              : rank == 1 ? "m-kind"
                                                          : "a-kind",
                                      .qualified_name = "same_name",
                                      .signature = "same_signature",
                                      .definition = true});
  if (rank == 1) {
    state.diagnostics.push_back({.severity = "error",
                                 .message = "synthetic failed TU",
                                 .path = "src1.c"});
  }
  return state;
}

struct RunResult {
  std::vector<std::pair<std::size_t, std::vector<std::string>>> emissions;
  std::vector<std::string> translation_units;
  std::vector<ContextIdentity> identities;
  std::vector<Diagnostic> diagnostics;
  OrderedObservationCommitter::Summary summary;
  std::vector<std::string> claimed;
};

RunResult run_order(const std::array<std::size_t, 3> &order) {
  RunResult output;
  OrderedObservationCommitter committer(
      "/repo", {"src0.c", "src1.c", "src2.c", "shared.h"}, 3, 2,
      [&](std::size_t rank, State &&files) {
        std::vector<std::string> paths;
        for (const auto &[path, _] : files.files) {
          paths.push_back(path);
        }
        output.emissions.emplace_back(rank, std::move(paths));
      });

  for (const auto rank : order) {
    require(committer.submit(rank, result_for(rank), rank == 1 ? 1 : 0),
            "committer rejected a unique planned rank");
  }

  auto metadata = committer.take_metadata_state();
  for (const auto &unit : metadata.translation_units) {
    output.translation_units.push_back(unit.path);
  }
  output.identities = std::move(metadata.context_identities);
  output.diagnostics = std::move(metadata.diagnostics);
  output.summary = committer.summary();
  output.claimed = committer.claimed_files();
  return output;
}

bool same_emissions(const auto &left, const auto &right) {
  if (left.size() != right.size()) {
    return false;
  }
  for (std::size_t index = 0; index < left.size(); ++index) {
    if (left[index].first != right[index].first ||
        left[index].second != right[index].second) {
      return false;
    }
  }
  return true;
}

void saturated_window_blocks_and_wakes() {
  std::vector<std::size_t> committed_ranks;
  OrderedObservationCommitter committer(
      "/repo", {"src0.c", "src1.c", "src2.c", "shared.h"}, 3, 1,
      [&](std::size_t rank, State &&) { committed_ranks.push_back(rank); });

  require(committer.submit(1, result_for(1), 1),
          "failed to fill the one-result pending window");

  std::promise<void> submission_started;
  auto started = submission_started.get_future();
  std::promise<bool> submission_result;
  auto submitted = submission_result.get_future();
  std::thread later_rank([&] {
    submission_started.set_value();
    submission_result.set_value(committer.submit(2, result_for(2)));
  });

  started.wait();
  const bool remained_blocked =
      submitted.wait_for(std::chrono::milliseconds(50)) ==
      std::future_status::timeout;

  // Always deliver the frontier before asserting so an unexpectedly early
  // wakeup cannot strand the joinable submitter.
  const bool frontier_accepted = committer.submit(0, result_for(0));
  const bool later_rank_accepted = submitted.get();
  later_rank.join();

  require(remained_blocked,
          "later rank did not wait while the pending window was saturated");
  require(frontier_accepted && later_rank_accepted,
          "frontier delivery did not release the blocked submitter");
  require(committed_ranks == std::vector<std::size_t>{0, 1, 2},
          "woken submitter did not preserve canonical commit order");
  require(committer.summary().peak_pending_results <= 1 &&
              committer.summary().completed_tus == 3,
          "saturated-window execution exceeded its bound or lost a result");
}

void invalid_and_duplicate_ranks_are_rejected() {
  OrderedObservationCommitter committer("/repo", {"src0.c"}, 1, 1, {});
  require(!committer.submit(1, result_for(0)),
          "out-of-range rank was accepted");
  require(committer.submit(0, result_for(0)),
          "valid rank was rejected after an invalid submission");
  require(!committer.submit(0, result_for(0)),
          "duplicate completed rank was accepted");
  require(committer.summary().completed_tus == 1 &&
              committer.claimed_files() == std::vector<std::string>{"src0.c"},
          "invalid or duplicate rank changed committed output");
}

} // namespace

void staged_source_coverage_tests();

int main() {
  staged_source_coverage_tests();
  saturated_window_blocks_and_wakes();
  invalid_and_duplicate_ranks_are_rejected();

  const auto reverse = run_order({2, 1, 0});
  require(reverse.emissions.size() == 3 && reverse.emissions[0].first == 0 &&
              reverse.emissions[1].first == 1 &&
              reverse.emissions[2].first == 2,
          "reverse completion did not emit in canonical TU order");
  require(reverse.emissions[0].second ==
              std::vector<std::string>{"shared.h", "src0.c"},
          "first canonical TU did not claim its files in path order");
  require(reverse.claimed == std::vector<std::string>{"shared.h", "src0.c",
                                                      "src1.c", "src2.c"},
          "owned file claims are not canonical or include a context-only file");
  require(reverse.summary.completed_tus == 3 &&
              reverse.summary.claimed_owned_files == 4 &&
              reverse.summary.discarded_duplicate_file_observations == 2 &&
              reverse.summary.peak_pending_results <= 2,
          "committer summary violates bounded ordered-claim semantics");
  require(reverse.translation_units ==
              std::vector<std::string>{"src0.c", "src1.c", "src2.c"},
          "translation-unit metadata did not follow canonical commit order");
  const auto symbol = std::find_if(
      reverse.identities.begin(), reverse.identities.end(),
      [](const auto &identity) { return identity.compiler_id == "symbol"; });
  const auto tied = std::find_if(
      reverse.identities.begin(), reverse.identities.end(),
      [](const auto &identity) { return identity.compiler_id == "tie"; });
  require(reverse.identities.size() == 2 &&
              symbol != reverse.identities.end() && symbol->definition &&
              tied != reverse.identities.end() && tied->kind == "a-kind",
          "definition evidence did not win deterministic identity merging");
  require(reverse.diagnostics.size() == 1 &&
              reverse.diagnostics.front().message == "synthetic failed TU",
          "failed TU diagnostic was not retained while advancing the frontier");

  const std::array<std::array<std::size_t, 3>, 6> orders{{
      {{0, 1, 2}},
      {{0, 2, 1}},
      {{1, 0, 2}},
      {{1, 2, 0}},
      {{2, 0, 1}},
      {{2, 1, 0}},
  }};
  for (const auto &order : orders) {
    const auto current = run_order(order);
    const auto current_symbol = std::find_if(
        current.identities.begin(), current.identities.end(),
        [](const auto &identity) { return identity.compiler_id == "symbol"; });
    const auto current_tied = std::find_if(
        current.identities.begin(), current.identities.end(),
        [](const auto &identity) { return identity.compiler_id == "tie"; });
    require(same_emissions(current.emissions, reverse.emissions) &&
                current.translation_units == reverse.translation_units &&
                current.claimed == reverse.claimed &&
                current.summary.completed_tus ==
                    reverse.summary.completed_tus &&
                current.summary.claimed_owned_files ==
                    reverse.summary.claimed_owned_files &&
                current.summary.discarded_duplicate_file_observations ==
                    reverse.summary.discarded_duplicate_file_observations &&
                current.identities.size() == reverse.identities.size() &&
                current_symbol != current.identities.end() &&
                current_symbol->definition &&
                current_tied != current.identities.end() &&
                current_tied->kind == "a-kind" &&
                current.diagnostics.size() == reverse.diagnostics.size(),
            "submission order changed canonical committer output");
  }
  return 0;
}
