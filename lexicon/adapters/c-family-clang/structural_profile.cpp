#include "structural_profile.h"

#include <cstdio>
#if defined(__GLIBC__)
#include <malloc.h>
#endif
#if defined(__linux__)
#include <unistd.h>
#elif defined(__APPLE__)
#include <mach/mach.h>
#endif

#include "clang/AST/ASTContext.h"
#include "perf.h"
#include "structural_memory.h"

namespace lexicon::clang_frontend {
std::uint64_t current_rss_bytes() {
#if defined(_WIN32)
  PROCESS_MEMORY_COUNTERS counters {};
  counters.cb = sizeof(counters);
  return GetProcessMemoryInfo(GetCurrentProcess(), &counters, sizeof(counters))
             ? counters.WorkingSetSize : 0;
#elif defined(__linux__)
  auto *input = std::fopen("/proc/self/statm", "r");
  if (!input) return 0;
  unsigned long pages = 0, resident = 0;
  const auto read = std::fscanf(input, "%lu %lu", &pages, &resident);
  std::fclose(input);
  const auto page_size = sysconf(_SC_PAGESIZE);
  return read == 2 && page_size > 0
             ? static_cast<std::uint64_t>(resident) * page_size : 0;
#elif defined(__APPLE__)
  mach_task_basic_info_data_t info {};
  mach_msg_type_number_t count = MACH_TASK_BASIC_INFO_COUNT;
  return task_info(mach_task_self(), MACH_TASK_BASIC_INFO,
                   reinterpret_cast<task_info_t>(&info), &count) == KERN_SUCCESS
             ? info.resident_size : 0;
#else
  return 0;
#endif
}

void profile_translation_unit(std::string_view stage, std::size_t rank,
                              PerfClock::duration elapsed, const State *state,
                              const clang::ASTContext *context) {
  if (!perf_enabled()) return;
  std::uint64_t live_heap = 0, free_heap = 0, mapped_heap = 0, heap_supported = 0;
#if defined(__GLIBC__)
#if __GLIBC_PREREQ(2, 33)
  const auto heap = mallinfo2();
  live_heap = heap.uordblks;
  free_heap = heap.fordblks;
  mapped_heap = heap.hblkhd;
  heap_supported = 1;
#endif
#endif
  emit_perf(stage, elapsed,
            {{"rank", rank}, {"current_rss_bytes", current_rss_bytes()},
             {"peak_rss_bytes", peak_rss_bytes()},
             {"allocator_stats_available", heap_supported},
             {"live_heap_bytes", live_heap}, {"free_heap_bytes", free_heap},
             {"mapped_heap_bytes", mapped_heap},
             {"observation_estimated_bytes",
              state ? estimated_retained_bytes(*state) : 0},
             {"ast_allocated_bytes",
              context ? context->getASTAllocatedMemory() : 0},
             {"ast_side_table_bytes",
              context ? context->getSideTableAllocatedMemory() : 0}});
}
} // namespace lexicon::clang_frontend
