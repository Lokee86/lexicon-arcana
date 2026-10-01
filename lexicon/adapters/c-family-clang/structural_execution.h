#pragma once

#include <cstddef>
#include <functional>
#include <string>
#include <vector>

#include "structural_model.h"
#include "structural_compilation.h"

namespace lexicon::clang_frontend {

struct ExecutionSummary {
  std::size_t active_clang_lanes = 0;
  std::size_t completed_tus = 0;
  std::size_t peak_pending_estimated_bytes = 0;
  std::size_t primary_synthetic_parse_units = 0;
  std::size_t skipped_covered_source_units = 0;
  std::size_t orphan_fallback_units = 0;
  std::size_t completed_orphan_tus = 0;
  std::size_t claimed_orphan_files = 0;
  std::size_t discarded_duplicate_orphan_observations = 0;
  std::size_t claimed_owned_files = 0;
  std::size_t discarded_duplicate_file_observations = 0;
};

using FileObservationConsumer =
    std::function<void(std::size_t rank, State &&)>;

int execute_parse_plan(const std::string &root, CompilationCommands &database,
                       const std::vector<std::string> &owned_files,
                       const ParsePlan &plan, std::size_t workers, State &state,
                       ExecutionSummary &summary,
                       const FileObservationConsumer &consume_files);

} // namespace lexicon::clang_frontend
