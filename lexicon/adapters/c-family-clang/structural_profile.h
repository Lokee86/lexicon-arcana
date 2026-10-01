#pragma once

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <string_view>

namespace clang { class ASTContext; }
namespace lexicon::clang_frontend {
struct State;
std::uint64_t current_rss_bytes();
void profile_translation_unit(
    std::string_view stage, std::size_t rank,
    std::chrono::steady_clock::duration elapsed, const State *state = nullptr,
    const clang::ASTContext *context = nullptr);
} // namespace lexicon::clang_frontend
