#include "structural_heap.h"

#include <cstdint>
#include <mutex>
#if defined(__GLIBC__)
#include <malloc.h>
#endif
#include "perf.h"
#include "structural_profile.h"

namespace lexicon::clang_frontend {
void reclaim_unused_heap(std::size_t rank) {
#if defined(__GLIBC__)
#if __GLIBC_PREREQ(2, 33)
  // A failed try_lock is an observed skip, not a blocking allocator walk.
  static std::mutex mutex;
  std::unique_lock lock(mutex, std::try_to_lock);
  if (!lock.owns_lock()) {
    emit_perf("c-family.clang.heap_reclaim", PerfClock::duration{},
              {{"rank", rank}, {"attempted", 0}, {"trimmed", 0},
               {"lock_busy", 1}});
    return;
  }
  constexpr std::size_t threshold = 64 * 1024 * 1024;
  static std::uint64_t last_reclaimed_rss = 0;
  const auto free_before = mallinfo2().fordblks;
  const auto rss_before = current_rss_bytes();
  const bool eligible = free_before >= threshold &&
      rss_before >= last_reclaimed_rss &&
      rss_before - last_reclaimed_rss >= threshold;
  int trimmed = 0;
  PerfClock::duration elapsed{};
  if (eligible) {
    const auto started = PerfClock::now();
    trimmed = malloc_trim(0);
    elapsed = PerfClock::now() - started;
    last_reclaimed_rss = current_rss_bytes();
  }
  emit_perf("c-family.clang.heap_reclaim", elapsed,
            {{"rank", rank}, {"attempted", static_cast<std::uint64_t>(eligible)},
             {"trimmed", static_cast<std::uint64_t>(trimmed != 0)},
             {"lock_busy", 0}, {"free_heap_before_bytes", free_before},
             {"free_heap_after_bytes", mallinfo2().fordblks},
             {"current_rss_bytes", current_rss_bytes()}});
#else
  emit_perf("c-family.clang.heap_reclaim", PerfClock::duration{},
            {{"rank", rank}, {"supported", 0}, {"attempted", 0},
             {"trimmed", 0}});
#endif
#else
  emit_perf("c-family.clang.heap_reclaim", PerfClock::duration{},
            {{"rank", rank}, {"supported", 0}, {"attempted", 0},
             {"trimmed", 0}});
#endif
}
} // namespace lexicon::clang_frontend
