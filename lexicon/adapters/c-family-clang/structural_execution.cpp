#include "structural_execution.h"

#include <algorithm>
#include <filesystem>
#include <memory>
#include <set>
#include <string_view>
#include <thread>
#include <utility>
#include <vector>

#include "clang/Basic/Diagnostic.h"
#include "clang/Tooling/Tooling.h"
#include "llvm/ADT/SmallString.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Support/VirtualFileSystem.h"

#include "structural_commit.h"
#include "structural_frontend.h"
#include "structural_heap.h"
#include "structural_profile.h"
#include "structural_memory.h"
#include "perf.h"

namespace lexicon::clang_frontend {
namespace {

class DriverDiagnosticObserver final : public clang::DiagnosticConsumer {
public:
  void HandleDiagnostic(clang::DiagnosticsEngine::Level level,
                        const clang::Diagnostic &info) override {
    clang::DiagnosticConsumer::HandleDiagnostic(level, info);
    if (level == clang::DiagnosticsEngine::Error ||
        level == clang::DiagnosticsEngine::Fatal) {
      failed_ = true;
    }
    llvm::SmallString<256> message;
    info.FormatDiagnostic(message);
    llvm::errs() << message << "\n";
  }

  bool failed() const { return failed_; }

private:
  bool failed_ = false;
};

int run_lanes(const std::string &root, CompilationCommands &database,
              const std::vector<ParseUnit> &units,
              const std::vector<std::string> &owned_files,
              std::size_t active_lanes,
              std::string_view phase,
              OrderedObservationCommitter &committer,
              const std::vector<std::string> &prior_claims) {
  if (units.empty()) {
    return 0;
  }
  std::vector<std::vector<ParseUnit>> lanes(active_lanes);
  for (const auto &unit : units) {
    lanes[unit.rank % active_lanes].push_back(unit);
  }
  std::vector<int> statuses(active_lanes, 0);
  std::vector<std::thread> threads;
  threads.reserve(active_lanes);
  for (std::size_t lane = 0; lane < active_lanes; ++lane) {
    threads.emplace_back([&, lane] {
      ScopedPerfPhase profile_phase(phase);
      std::vector<std::string> paths;
      paths.reserve(lanes[lane].size());
      for (const auto &unit : lanes[lane]) {
        paths.push_back((std::filesystem::path(root) / unit.translation_unit)
                            .lexically_normal().string());
      }
      // Compile commands change the VFS working directory. Give each lane
      // an independent view rather than racing on the process directory.
      auto filesystem = llvm::vfs::createPhysicalFileSystem();
      clang::tooling::ClangTool tool(
          database, paths, std::make_shared<clang::PCHContainerOperations>(),
          llvm::IntrusiveRefCntPtr<llvm::vfs::FileSystem>(filesystem.release()));
      DriverDiagnosticObserver driver_diagnostics;
      tool.setDiagnosticConsumer(&driver_diagnostics);
      auto factory = make_frontend_factory(
          lanes[lane], database, owned_files, root,
          [&](std::size_t rank, State result, int status) {
            statuses[lane] |= status;
            // FrontendActionFactory::runInvocation has returned: AST/Sema and
            // compiler state are destroyed, but result observations remain live.
            report_translation_unit_teardown(rank);
            reclaim_unused_heap(rank);
            const auto bytes = estimated_retained_bytes(result);
            const auto handoff_started = PerfClock::now();
            committer.submit(rank, std::move(result), status);
            emit_perf("c-family.clang.result_handoff",
                      PerfClock::now() - handoff_started,
                      {{"rank", rank}, {"result_estimated_bytes", bytes},
                       {"frontend_status", static_cast<std::uint64_t>(status)}});
          }, prior_claims);
      statuses[lane] |= tool.run(factory.get());
      if (driver_diagnostics.failed()) {
        statuses[lane] |= 1;
      }
      // Driver failures can precede factory invocation. Fill any remaining
      // ranks, including a skipped final input, so ordered commit can drain.
      for (const auto &unit : lanes[lane]) {
        if (committer.has_result(unit.rank)) {
          continue;
        }
        auto result = make_translation_unit_state(root, database, unit,
                                                  owned_files);
        result.add_diagnostic({
            .severity = "error",
            .message = "Clang did not invoke translation unit " +
                       unit.translation_unit,
            .path = unit.translation_unit,
        });
        if (result.owns(unit.translation_unit)) {
          const auto language = result.translation_units.empty()
                                    ? std::string("cpp")
                                    : result.translation_units.front().language;
          result.file(unit.translation_unit, language, unit.translation_unit);
        }
        result.suppress_observations(prior_claims);
        // The driver has finished; no compiler state remains live.
        report_translation_unit_teardown(unit.rank);
        reclaim_unused_heap(unit.rank);
        committer.submit(unit.rank, std::move(result), 1);
        statuses[lane] |= 1;
      }
    });
  }
  for (auto &thread : threads) {
    thread.join();
  }
  int status = 0;
  for (const auto lane_status : statuses) {
    status |= lane_status;
  }
  return status;
}

} // namespace

int execute_parse_plan(const std::string &root, CompilationCommands &database,
                       const std::vector<std::string> &owned_files,
                       const ParsePlan &plan, std::size_t workers, State &state,
                       ExecutionSummary &summary,
                       const FileObservationConsumer &consume_files) {
  summary = {};
  workers = std::max<std::size_t>(workers, 1);
  auto consume = [&](std::size_t rank, State &&files) {
    if (consume_files) {
      consume_files(rank, std::move(files));
    }
  };
  std::set<std::pair<std::string, std::string>> valid_real_coverage;
  auto execute_phase = [&](const std::vector<ParseUnit> &units,
                           const std::vector<std::string> &owned,
                           std::string_view phase,
                           const std::vector<std::string> &prior_claims =
                               std::vector<std::string>{}) {
    const auto active_lanes = std::min(workers, units.size());
    OrderedObservationCommitter committer(
        root, owned, units.size(),
        std::max<std::size_t>(active_lanes * 2, 1), consume, prior_claims);
    const auto status = run_lanes(root, database, units, owned, active_lanes,
                                  phase, committer, prior_claims);
    const auto completed = committer.summary();
    summary.active_clang_lanes =
        std::max(summary.active_clang_lanes, active_lanes);
    summary.completed_tus += completed.completed_tus;
    summary.peak_pending_estimated_bytes = std::max(
        summary.peak_pending_estimated_bytes, completed.peak_pending_estimated_bytes);
    summary.claimed_owned_files += completed.claimed_owned_files;
    summary.discarded_duplicate_file_observations +=
        completed.discarded_duplicate_file_observations;
    const auto coverage = committer.valid_real_coverage();
    valid_real_coverage.insert(coverage.begin(), coverage.end());
    state.merge(committer.take_metadata_state());
    return std::make_pair(status, committer.claimed_files());
  };

  std::vector<ParseUnit> real, synthetic;
  for (const auto &unit : plan.primary_units) {
    if (!unit.synthesized) {
      real.push_back({real.size(), unit.translation_unit, false});
    }
  }
  auto [status, primary_claimed] = execute_phase(real, owned_files, "real");
  for (const auto &unit : plan.primary_units) {
    if (!unit.synthesized) {
      continue;
    }
    if (valid_real_coverage.contains(
            {unit.translation_unit, language_for(unit.translation_unit, {})})) {
      ++summary.skipped_covered_source_units;
    } else {
      synthetic.push_back({synthetic.size(), unit.translation_unit, true});
    }
  }
  summary.primary_synthetic_parse_units = synthetic.size();
  auto synthetic_result = execute_phase(synthetic, owned_files, "synthetic", primary_claimed);
  status |= synthetic_result.first;
  primary_claimed = std::move(synthetic_result.second);

  std::vector<std::string> orphans;
  for (const auto &path : plan.orphan_candidates) {
    if (!std::binary_search(primary_claimed.begin(), primary_claimed.end(), path)) {
      orphans.push_back(path);
    }
  }
  std::sort(orphans.begin(), orphans.end());
  orphans.erase(std::unique(orphans.begin(), orphans.end()), orphans.end());
  std::vector<ParseUnit> fallback;
  fallback.reserve(orphans.size());
  for (const auto &path : orphans) {
    fallback.push_back({fallback.size(), path, database.synthesized(path)});
  }
  summary.orphan_fallback_units = fallback.size();
  const auto primary_completed = summary.completed_tus;
  const auto primary_claimed_count = summary.claimed_owned_files;
  const auto primary_duplicates = summary.discarded_duplicate_file_observations;
  // Restrict ownership so fallback includes cannot publish primary files again.
  status |= execute_phase(fallback, orphans, "orphan").first;
  summary.completed_orphan_tus = summary.completed_tus - primary_completed;
  summary.claimed_orphan_files = summary.claimed_owned_files - primary_claimed_count;
  summary.discarded_duplicate_orphan_observations =
      summary.discarded_duplicate_file_observations - primary_duplicates;
  return status;
}

} // namespace lexicon::clang_frontend
