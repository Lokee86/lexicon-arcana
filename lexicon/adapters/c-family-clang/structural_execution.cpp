#include "structural_execution.h"

#include <algorithm>
#include <filesystem>
#include <memory>
#include <thread>
#include <utility>
#include <vector>

#include "clang/Tooling/Tooling.h"

#include "structural_frontend.h"

namespace lexicon::clang_frontend {
namespace {

int run_task(const std::string &root, CompilationCommands &database,
             State &state, const AnalysisTask &task) {
  state.set_owned_files(task.owned_files);
  const auto absolute =
      (std::filesystem::path(root) /
       std::filesystem::path(task.translation_unit))
          .string();
  auto commands = database.getCompileCommands(absolute);
  if (commands.empty()) {
    return 1;
  }
  const auto &command = commands.front();
  const auto language = language_for(task.translation_unit, command.CommandLine);
  state.translation_units.push_back({
      .path = task.translation_unit,
      .language = language,
      .directory = command.Directory,
      .arguments = command.CommandLine,
      .synthesized = task.synthesized,
  });
  if (state.owns(task.translation_unit)) {
    state.file(task.translation_unit, language, task.translation_unit);
  }

  clang::tooling::ClangTool tool(database, {absolute});
  auto factory = make_frontend_factory(state, root);
  return tool.run(factory.get());
}

std::unique_ptr<State>
run_shard(const std::string &root, CompilationCommands &database,
          const std::vector<const AnalysisTask *> &tasks, int &status) {
  auto shard = std::make_unique<State>(root);
  for (const auto *task : tasks) {
    auto task_state = std::make_unique<State>(root);
    status |= run_task(root, database, *task_state, *task);
    shard->merge(std::move(*task_state));
  }
  return shard;
}

std::unique_ptr<State>
reduce_states(std::vector<std::unique_ptr<State>> states,
              std::size_t merge_fan_in) {
  while (states.size() > 1) {
    std::vector<std::unique_ptr<State>> next;
    next.reserve((states.size() + merge_fan_in - 1) / merge_fan_in);
    for (std::size_t begin = 0; begin < states.size();
         begin += merge_fan_in) {
      auto merged = std::move(states[begin]);
      const auto end = std::min(states.size(), begin + merge_fan_in);
      for (std::size_t index = begin + 1; index < end; ++index) {
        merged->merge(std::move(*states[index]));
      }
      next.push_back(std::move(merged));
    }
    states = std::move(next);
  }
  return states.empty() ? nullptr : std::move(states.front());
}

std::vector<std::vector<const AnalysisTask *>>
partition_tasks(const std::vector<AnalysisTask> &tasks,
                std::size_t logical_shards) {
  std::vector<std::vector<const AnalysisTask *>> result(logical_shards);
  for (std::size_t index = 0; index < tasks.size(); ++index) {
    result[index % logical_shards].push_back(&tasks[index]);
  }
  return result;
}

} // namespace

int execute_task_plan(const std::string &root, CompilationCommands &database,
                      State &state, const std::vector<AnalysisTask> &tasks,
                      ExecutionPolicy policy, ExecutionSummary &summary) {
  if (tasks.empty()) {
    summary = {};
    return 0;
  }

  summary.logical_shards = std::min(policy.shards, tasks.size());
  summary.worker_limit = std::min(policy.workers, summary.logical_shards);
  const auto shards = partition_tasks(tasks, summary.logical_shards);

  int status = 0;
  for (std::size_t wave = 0; wave < shards.size();
       wave += summary.worker_limit) {
    const auto wave_size =
        std::min(summary.worker_limit, shards.size() - wave);
    std::vector<int> statuses(wave_size, 0);
    std::vector<std::unique_ptr<State>> results(wave_size);
    std::vector<std::thread> workers;
    workers.reserve(wave_size);

    for (std::size_t index = 0; index < wave_size; ++index) {
      const auto shard_index = wave + index;
      workers.emplace_back([&database, &root, &results, &shards, &statuses,
                            index, shard_index]() {
        results[index] = run_shard(root, database, shards[shard_index],
                                   statuses[index]);
      });
    }
    for (auto &worker : workers) {
      worker.join();
    }

    for (const auto value : statuses) {
      status |= value;
    }
    if (auto merged = reduce_states(std::move(results), policy.merge_fan_in)) {
      state.merge(std::move(*merged));
    }
  }
  return status;
}

} // namespace lexicon::clang_frontend
