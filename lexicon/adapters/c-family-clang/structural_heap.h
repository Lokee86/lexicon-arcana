#pragma once

namespace lexicon::clang_frontend {
// Called after TU teardown and observation handoff; never affects ownership.
void reclaim_unused_heap();
}
