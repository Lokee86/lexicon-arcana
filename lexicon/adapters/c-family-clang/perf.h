#pragma once

#include <chrono>
#include <cstdlib>
#include <cstdint>
#include <initializer_list>
#include <mutex>
#include <string>
#include <string_view>
#include <utility>

#if defined(_WIN32)
#include <windows.h>
#include <psapi.h>
#else
#include <sys/resource.h>
#endif

#include "llvm/Support/FormatVariadic.h"
#include "llvm/Support/raw_ostream.h"

namespace lexicon::clang_frontend {

using PerfClock = std::chrono::steady_clock;

// Phase is thread-local: different Clang workers may parse separate phases,
// but their records must retain the phase-local rank without global races.
inline std::string_view &current_perf_phase() {
  static thread_local std::string_view phase;
  return phase;
}

class ScopedPerfPhase {
public:
  explicit ScopedPerfPhase(std::string_view phase)
      : previous_(current_perf_phase()) { current_perf_phase() = phase; }
  ~ScopedPerfPhase() { current_perf_phase() = previous_; }
  ScopedPerfPhase(const ScopedPerfPhase &) = delete;
  ScopedPerfPhase &operator=(const ScopedPerfPhase &) = delete;
private:
  std::string_view previous_;
};

inline bool perf_enabled() {
  const char *value = std::getenv("LEXICON_PERF");
  return value != nullptr && value[0] != '\0' &&
         std::string_view(value) != "0" && std::string_view(value) != "false" &&
         std::string_view(value) != "off" && std::string_view(value) != "no";
}

inline std::uint64_t peak_rss_bytes() {
#if defined(_WIN32)
  PROCESS_MEMORY_COUNTERS counters {};
  counters.cb = sizeof(counters);
  if (!GetProcessMemoryInfo(GetCurrentProcess(), &counters, sizeof(counters))) {
    return 0;
  }
  return static_cast<std::uint64_t>(counters.PeakWorkingSetSize);
#else
  struct rusage usage {};
  if (getrusage(RUSAGE_SELF, &usage) != 0) {
    return 0;
  }
#if defined(__APPLE__)
  return static_cast<std::uint64_t>(usage.ru_maxrss);
#else
  return static_cast<std::uint64_t>(usage.ru_maxrss) * 1024ULL;
#endif
#endif
}

inline void emit_perf(
    std::string_view stage, PerfClock::duration elapsed,
    std::initializer_list<std::pair<std::string_view, std::uint64_t>> counters = {}) {
  if (!perf_enabled()) {
    return;
  }
  const double elapsed_ms =
      std::chrono::duration<double, std::milli>(elapsed).count();
  // Compose the complete record privately; do not interleave partial metric
  // tokens when workers emit concurrently.
  std::string line;
  llvm::raw_string_ostream output(line);
  output << "[lexicon-perf] stage=" << stage
         << " elapsed_ms=" << llvm::formatv("{0:F3}", elapsed_ms);
  if (!current_perf_phase().empty()) {
    output << " phase=" << current_perf_phase();
  }
  for (const auto &[name, value] : counters) {
    output << " " << name << "=" << value;
  }
  output << "\n";
  output.flush();
  static std::mutex output_mutex;
  std::lock_guard lock(output_mutex);
  llvm::errs().write(line.data(), line.size());
  llvm::errs().flush();
}
} // namespace lexicon::clang_frontend
