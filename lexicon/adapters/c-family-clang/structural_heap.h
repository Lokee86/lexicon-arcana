#pragma once

#include <cstddef>

namespace lexicon::clang_frontend {
// Called after compiler teardown but before result handoff can block.
// A skipped trim is recorded without changing semantic behaviour.
void reclaim_unused_heap(std::size_t rank);
}
