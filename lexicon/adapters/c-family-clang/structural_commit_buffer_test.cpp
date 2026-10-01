#include "structural_commit.h"
#include "structural_memory.h"

#include <chrono>
#include <future>
#include <stdexcept>
#include <thread>

namespace {
using namespace lexicon::clang_frontend;
using namespace std::chrono_literals;
void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}
State sample(std::size_t rank) {
  State value("/repo");
  auto path = "unit" + std::to_string(rank) + ".c";
  value.file(path, "c", path).macros.push_back(
      {.name = "DATA", .replacement = std::string(4096, 'x')});
  return value;
}
void byte_admission(bool oversized) {
  const auto size = estimated_retained_bytes(sample(1));
  const auto limit = oversized ? size - 1 : 2 * size - 1;
  OrderedObservationCommitter committer(
      "/repo", {"unit0.c", "unit1.c", "unit2.c"}, 3, 8, {}, {}, limit);
  if (!oversized) committer.submit(1, sample(1));
  const auto rank = oversized ? 1 : 2;
  std::promise<void> started;
  auto start = started.get_future();
  auto later = std::async(std::launch::async, [&] {
    started.set_value();
    return committer.submit(rank, sample(rank));
  });
  start.wait();
  const bool blocked = later.wait_for(50ms) == std::future_status::timeout;
  committer.submit(0, sample(0));
  require(later.get(), "byte-blocked result was rejected after frontier delivery");
  if (oversized) committer.submit(2, sample(2));
  require(blocked, "byte limit did not apply backpressure");
  require(committer.summary().completed_tus == 3 &&
              committer.summary().peak_pending_estimated_bytes <= limit,
          "byte-bounded drain exceeded budget or lost a result");
}
void encoder_does_not_hold_commit_mutex() {
  std::promise<void> entered, release;
  auto entered_future = entered.get_future();
  auto release_future = release.get_future().share();
  std::vector<std::size_t> emitted;
  OrderedObservationCommitter *current = nullptr;
  OrderedObservationCommitter committer(
      "/repo", {"unit0.c", "unit1.c"}, 2, 2,
      [&](std::size_t rank, State &&) {
        // Also verifies that read-only committer inspection can re-enter.
        current->summary();
        emitted.push_back(rank);
        if (rank == 0) {
          entered.set_value();
          release_future.wait();
        }
      });
  current = &committer;
  auto first = std::async(std::launch::async, [&] {
    return committer.submit(0, sample(0));
  });
  entered_future.wait();
  auto second = std::async(std::launch::async, [&] {
    return committer.submit(1, sample(1));
  });
  const bool enqueued = second.wait_for(100ms) == std::future_status::ready;
  release.set_value();
  require(first.get() && second.get(), "unlocked emission rejected input");
  require(enqueued, "encoder retained the commit mutex");
  require(emitted == std::vector<std::size_t>{0, 1},
          "encoding outside the lock reordered output");
}
} // namespace

void commit_buffer_tests() {
  byte_admission(false);
  byte_admission(true);
  encoder_does_not_hold_commit_mutex();
  auto small = sample(0), big = sample(0);
  big.files.begin()->second.macros.front().replacement.reserve(32768);
  require(estimated_retained_bytes(big) > estimated_retained_bytes(small),
          "retained string capacity was ignored");
  small.suppress_observations({"unit0.c"});
  require(!small.owns("unit0.c") && small.contains_owned("unit0.c") == false,
          "unowned sample unexpectedly acquired ownership");
  State owned("/repo");
  owned.set_owned_files({"unit0.c"});
  owned.file("unit0.c", "c", "unit0.c");
  owned.suppress_observations({"unit0.c"});
  require(!owned.owns("unit0.c") && owned.contains_owned("unit0.c") &&
              owned.files.empty() && owned.all_owned_paths.contains("unit0.c"),
          "prior claim pruning lost global ownership or kept observations");
}
