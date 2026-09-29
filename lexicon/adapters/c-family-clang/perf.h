#pragma once

#include <chrono>
#include <cstdlib>
#include <cstdint>
#include <initializer_list>
#include <string_view>
#include <utility>

#include "llvm/Support/FormatVariadic.h"
#include "llvm/Support/raw_ostream.h"

namespace lexicon::clang_frontend {

using PerfClock = std::chrono::steady_clock;

inline bool perf_enabled() {
  const char *value = std::getenv("LEXICON_PERF");
  return value != nullptr && value[0] != '\0' &&
         std::string_view(value) != "0" && std::string_view(value) != "false" &&
         std::string_view(value) != "off" && std::string_view(value) != "no";
}

inline void emit_perf(
    std::string_view stage, PerfClock::duration elapsed,
    std::initializer_list<std::pair<std::string_view, std::uint64_t>> counters = {}) {
  if (!perf_enabled()) {
    return;
  }
  const double elapsed_ms =
      std::chrono::duration<double, std::milli>(elapsed).count();
  llvm::errs() << "[lexicon-perf] stage=" << stage
               << " elapsed_ms=" << llvm::formatv("{0:F3}", elapsed_ms);
  for (const auto &[name, value] : counters) {
    llvm::errs() << " " << name << "=" << value;
  }
  llvm::errs() << "\n";
}

} // namespace lexicon::clang_frontend