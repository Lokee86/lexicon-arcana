#include "structural_commit.h"
#include "structural_memory.h"

#include <algorithm>
#include <iterator>
#include <tuple>
#include <utility>

namespace lexicon::clang_frontend {

OrderedObservationCommitter::OrderedObservationCommitter(
    std::string repository_root, std::vector<std::string> owned_files,
    std::size_t total_results, std::size_t pending_limit,
    FileConsumer consume_files, const std::vector<std::string> &prior_claims,
    std::size_t pending_byte_limit)
    : repository_root_(std::move(repository_root)),
      owned_files_(owned_files.begin(), owned_files.end()),
      total_results_(total_results),
      pending_limit_(std::max<std::size_t>(pending_limit, 1)),
      pending_byte_limit_(std::max<std::size_t>(pending_byte_limit, 1)),
      consume_files_(std::move(consume_files)),
      submitted_ranks_(total_results, false),
      claimed_files_(prior_claims.begin(), prior_claims.end()),
      metadata_state_(repository_root_) {
  metadata_state_.set_owned_files(owned_files);
}

bool OrderedObservationCommitter::submit(std::size_t rank, State result,
                                         int status) {
  const auto bytes = estimated_retained_bytes(result);
  std::unique_lock lock(mutex_);
  if (rank >= total_results_ || rank < next_rank_ || submitted_ranks_[rank]) {
    return false;
  }
  changed_.wait(lock, [&] {
    // Oversized frontier results drain directly; future ones stay in their
    // producer lane instead of entering the pending queue.
    return (pending_.size() < pending_limit_ &&
            bytes <= pending_byte_limit_ - pending_bytes_) ||
           rank == next_rank_ || rank < next_rank_ || submitted_ranks_[rank];
  });
  if (rank < next_rank_ || submitted_ranks_[rank]) {
    return false;
  }
  submitted_ranks_[rank] = true;
  PendingResult current{std::move(result), status, bytes};
  if (rank != next_rank_) {
    pending_.emplace(rank, std::move(current));
    pending_bytes_ += bytes;
    summary_.peak_pending_results =
        std::max(summary_.peak_pending_results, pending_.size());
    summary_.peak_pending_estimated_bytes =
        std::max(summary_.peak_pending_estimated_bytes, pending_bytes_);
    return true;
  }
  // Only the frontier submitter drains. Keep next_rank_ on its in-flight rank
  // while encoding outside mutex_: producers can enqueue bounded results, but
  // cannot start a second emitter or overtake this rank.
  while (true) {
    auto files = commit_one_locked(std::move(current));
    lock.unlock();
    if (!files.files.empty() && consume_files_) {
      consume_files_(rank, std::move(files));
    }
    lock.lock();
    ++next_rank_;
    changed_.notify_all();
    auto next = pending_.find(next_rank_);
    if (next == pending_.end()) {
      return true;
    }
    rank = next_rank_;
    pending_bytes_ -= next->second.estimated_bytes;
    current = std::move(next->second);
    pending_.erase(next);
  }
}

bool OrderedObservationCommitter::has_result(std::size_t rank) const {
  std::lock_guard lock(mutex_);
  return rank < total_results_ &&
         (rank < next_rank_ || submitted_ranks_[rank]);
}

State OrderedObservationCommitter::commit_one_locked(PendingResult result) {
  auto &state = result.state;
  aggregate_status_ |= result.status;
  // A mere file entry (or recovery AST) is not sufficient coverage. Only the
  // canonical claim from a completed, error-free real TU may replace a source
  // fallback; the emitted language must match that fallback's language.
  const auto valid_real_tu =
      result.status == 0 && state.translation_units.size() == 1 &&
      !state.translation_units.front().synthesized &&
      std::none_of(state.diagnostics.begin(), state.diagnostics.end(),
                   [](const auto &diagnostic) {
                     return diagnostic.severity == "error" ||
                            diagnostic.severity == "fatal";
                   });
  const auto language = valid_real_tu
                            ? state.translation_units.front().language
                            : std::string();
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
    if (valid_real_tu && file.languages.contains(language) &&
        (!file.declarations.empty() || !file.macros.empty())) {
      valid_real_coverage_.emplace(path, language);
    }
    ++summary_.claimed_owned_files;
    file_observations.all_owned_paths.insert(path);
    file_observations.files.emplace(path, std::move(file));
  }
  return file_observations;
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

std::set<std::pair<std::string, std::string>>
OrderedObservationCommitter::valid_real_coverage() const {
  std::lock_guard lock(mutex_);
  return valid_real_coverage_;
}

} // namespace lexicon::clang_frontend
