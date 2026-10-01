#pragma once

#include <cstddef>
#include <condition_variable>
#include <functional>
#include <map>
#include <mutex>
#include <string>
#include <set>
#include <vector>

#include "structural_model.h"

namespace lexicon::clang_frontend {

class OrderedObservationCommitter final {
public:
  using FileConsumer = std::function<void(std::size_t, State &&)>;

  struct Summary {
    std::size_t completed_tus = 0;
    std::size_t claimed_owned_files = 0;
    std::size_t discarded_duplicate_file_observations = 0;
    std::size_t peak_pending_results = 0;
  };

  OrderedObservationCommitter(std::string repository_root,
                              std::vector<std::string> owned_files,
                              std::size_t total_results,
                              std::size_t pending_limit,
                              FileConsumer consume_files);

  // Returns false only when this rank has already completed or is invalid.
  // A rank at the current commit frontier may always enter a full window.
  bool submit(std::size_t rank, State result, int status = 0);
  bool has_result(std::size_t rank) const;

  // Valid after every rank has been submitted and committed.
  State take_metadata_state();
  Summary summary() const;
  std::vector<std::string> claimed_files() const;

private:
  struct PendingResult {
    State state;
    int status = 0;
  };

  void commit_ready_locked();
  void commit_one_locked(std::size_t rank, PendingResult result);

  std::string repository_root_;
  std::set<std::string> owned_files_;
  std::size_t total_results_;
  std::size_t pending_limit_;
  FileConsumer consume_files_;
  mutable std::mutex mutex_;
  std::condition_variable changed_;
  std::map<std::size_t, PendingResult> pending_;
  std::vector<bool> submitted_ranks_;
  std::set<std::string> claimed_files_;
  std::map<std::pair<std::string, std::string>, ContextIdentity>
      context_identities_;
  State metadata_state_;
  std::size_t next_rank_ = 0;
  Summary summary_;
  int aggregate_status_ = 0;
};

} // namespace lexicon::clang_frontend
