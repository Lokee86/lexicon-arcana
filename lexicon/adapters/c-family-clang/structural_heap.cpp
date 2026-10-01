#include "structural_heap.h"

#include <mutex>
#if defined(__GLIBC__)
#include <malloc.h>
#endif
#include "perf.h"
#include "structural_profile.h"

namespace lexicon::clang_frontend {
void reclaim_unused_heap() {
#if defined(__GLIBC__)
#if __GLIBC_PREREQ(2, 33)
  // Free arenas are process-wide. Serialize trimming, but never wait for a
  // competing lane. Small TUs avoid the allocator walk entirely.
  static std::mutex mutex;
  std::unique_lock lock(mutex, std::try_to_lock);
  if (!lock.owns_lock()) return;
  constexpr std::size_t threshold = 64 * 1024 * 1024;
  static std::uint64_t last_reclaimed_rss = 0;
  const auto before = mallinfo2().fordblks;
  const auto rss = current_rss_bytes();
  // trim may leave free virtual arena chunks intact. Use resident growth,
  // rather than those same chunks, to avoid repeating an ineffective trim.
  if (before < threshold || rss < last_reclaimed_rss ||
      rss - last_reclaimed_rss < threshold) return;
  const auto started = PerfClock::now();
  malloc_trim(0);
  last_reclaimed_rss = current_rss_bytes();
  emit_perf("c-family.clang.heap_reclaim", PerfClock::now() - started,
            {{"free_heap_before_bytes", before},
             {"free_heap_after_bytes", mallinfo2().fordblks}});
#endif
#endif
}
} // namespace lexicon::clang_frontend
