#pragma once

#include <cstddef>
#include "structural_model.h"

namespace lexicon::clang_frontend {
// Includes retained vector/string capacities and estimated tree-node overhead.
// This is a queue admission estimate, not a process RSS measurement.
std::size_t estimated_retained_bytes(const State &state);
} // namespace lexicon::clang_frontend
