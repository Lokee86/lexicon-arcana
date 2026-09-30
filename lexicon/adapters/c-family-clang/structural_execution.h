#pragma once

#include <cstddef>
#include <string>
#include <vector>

#include "structural_model.h"
#include "structural_plan.h"

namespace lexicon::clang_frontend {

struct ExecutionPolicy {
  std::size_t workers;
  std::size_t shards;
  std::size_t merge_fan_in;
};

struct ExecutionSummary {
  std::size_t logical_shards = 0;
  std::size_t worker_limit = 0;
};

int execute_task_plan(const std::string &root, CompilationCommands &database,
                      State &state, const std::vector<AnalysisTask> &tasks,
                      ExecutionPolicy policy, ExecutionSummary &summary);

} // namespace lexicon::clang_frontend
