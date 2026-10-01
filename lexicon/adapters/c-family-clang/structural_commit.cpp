#include "structural_commit.h"

#include <algorithm>
#include <iterator>
#include <tuple>
#include <utility>

namespace lexicon::clang_frontend {

OrderedObservationCommitter::OrderedObservationCommitter(
    std::string repository_root, std::vector<std::string> owned_files,
    std::size_t total_results, std::size_t pending_limit,
    FileConsumer consume_files)
    : repository_root_(std::move(repository_root)),
      owned_files_(owned_files.begin(), owned_files.end()),
      total_results_(total_results),
      pending_limit_(std::max<std::size_t>(pending_limit, 1)),
      consume_files_(std::move(consume_files)),
      submitted_ranks_(total_results, false),
      metadata_state_(repository_root_) {
  metadata_state_.set_owned_files(owned_files);
}

bool OrderedObservationCommitter::submit(std::size_t rank, State result,
                                         int status) {
  std::unique_lock lock(mutex_);
  if (rank >= total_results_ || rank < next_rank_ || submitted_ranks_[rank]) {
    return false;
  }
  changed_.wait(lock, [&] {
    return pending_.size() < pending_limit_ || rank == next_rank_ ||
           rank < next_rank_ || submitted_ranks_[rank];
  });
  if (rank < next_rank_ || submitted_ranks_[rank]) {
    return false;
  }

  submitted_ranks_[rank] = true;
  if (rank == next_rank_) {
    // The frontier commits directly without exceeding a full pending window.
    commit_one_locked(rank, PendingResult{std::move(result), status});
    ++next_rank_;
    commit_ready_locked();
    changed_.notify_all();
    return true;
  }
  pending_.emplace(rank, PendingResult{std::move(result), status});
  summary_.peak_pending_results =
      std::max(summary_.peak_pending_results, pending_.size());
  commit_ready_locked();
  changed_.notify_all();
  return true;
}

bool OrderedObservationCommitter::has_result(std::size_t rank) const {
  std::lock_guard lock(mutex_);
  return rank < total_results_ &&
         (rank < next_rank_ || submitted_ranks_[rank]);
}

void OrderedObservationCommitter::commit_ready_locked() {
  while (true) {
    auto next = pending_.find(next_rank_);
    if (next == pending_.end()) {
      return;
    }
    auto result = std::move(next->second);
    pending_.erase(next);
    commit_one_locked(next_rank_, std::move(result));
    ++next_rank_;
    changed_.notify_all();
  }
}

void OrderedObservationCommitter::commit_one_locked(std::size_t rank,
                                                    PendingResult result) {
  auto &state = result.state;
  aggregate_status_ |= result.status;
  summary_.completed_tus += 1;
  metadata_state_.semantic_analysis_ns += state.semantic_analysis_ns;
  metadata_state_.translation_units.insert(
      metadata_state_.translation_units.end(),
      std::make_move_iterator(state.translation_units.begin()),
      std::make_move_iterator(state.translation_units.end()));
  metadata_state_.diagnostics.insert(
      metadata_state_.diagnostics.end(),
      std::make_move_iterator(state.diagnostics.begin()),
      std::make_move_iterator(state.diagnostics.end()));

  for (auto &identity : state.context_identities) {
    const auto key = std::make_pair(identity.compiler_id, identity.path);
    auto [found, inserted] =
        context_identities_.try_emplace(key, std::move(identity));
    if (!inserted) {
      const auto &existing = found->second;
      if ((!existing.definition && identity.definition) ||
          (existing.definition == identity.definition &&
           std::tie(identity.kind, identity.qualified_name, identity.signature) <
               std::tie(existing.kind, existing.qualified_name, existing.signature))) {
        found->second = std::move(identity);
      }
    }
  }

  State file_observations(repository_root_);
  for (auto &[path, file] : state.files) {
    if (!owned_files_.contains(path)) {
      continue;
    }
    if (!claimed_files_.insert(path).second) {
      ++summary_.discarded_duplicate_file_observations;
      continue;
    }
    ++summary_.claimed_owned_files;
    file_observations.all_owned_paths.insert(path);
    file_observations.files.emplace(path, std::move(file));
  }
  if (!file_observations.files.empty() && consume_files_) {
    consume_files_(rank, std::move(file_observations));
  }
}

State OrderedObservationCommitter::take_metadata_state() {
  std::lock_guard lock(mutex_);
  if (next_rank_ != total_results_ || !pending_.empty()) {
    return State(repository_root_);
  }
  metadata_state_.context_identities.reserve(context_identities_.size());
  for (auto &[_, identity] : context_identities_) {
    metadata_state_.context_identities.push_back(std::move(identity));
  }
  return std::move(metadata_state_);
}

OrderedObservationCommitter::Summary
OrderedObservationCommitter::summary() const {
  std::lock_guard lock(mutex_);
  return summary_;
}

std::vector<std::string> OrderedObservationCommitter::claimed_files() const {
  std::lock_guard lock(mutex_);
  return {claimed_files_.begin(), claimed_files_.end()};
}

} // namespace lexicon::clang_frontend
