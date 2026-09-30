#include "structural_hot_path.h"

#include <unordered_map>
#include <utility>

namespace lexicon::clang_frontend {
namespace {

struct HotPathCache {
  bool profile = false;
  HotPathMetrics metrics;
  std::unordered_map<const clang::Decl *, std::string> compiler_ids;
  std::unordered_map<const clang::NamedDecl *, std::string> qualified_names;
  std::unordered_map<std::uint64_t, std::string> source_texts;
};

thread_local HotPathCache cache;

template <typename Key>
std::optional<std::string>
lookup(const std::unordered_map<Key, std::string> &values, Key key,
       std::uint64_t &hits, std::uint64_t &misses) {
  if (const auto found = values.find(key); found != values.end()) {
    if (cache.profile) {
      ++hits;
    }
    return found->second;
  }
  if (cache.profile) {
    ++misses;
  }
  return std::nullopt;
}

} // namespace

void reset_hot_path_context(bool profile) {
  cache = {};
  cache.profile = profile;
}

bool hot_path_profiling() { return cache.profile; }

HotPathMetrics hot_path_metrics() { return cache.metrics; }

void record_repository_path_cache(bool hit) {
  if (!cache.profile) {
    return;
  }
  if (hit) {
    ++cache.metrics.repository_path_hits;
  } else {
    ++cache.metrics.repository_path_misses;
  }
}

std::optional<std::string> cached_compiler_id(const clang::Decl *declaration) {
  return lookup(cache.compiler_ids, declaration, cache.metrics.compiler_id_hits,
                cache.metrics.compiler_id_misses);
}

void store_compiler_id(const clang::Decl *declaration, std::string value,
                       std::uint64_t elapsed_ns) {
  cache.compiler_ids.emplace(declaration, std::move(value));
  if (cache.profile) {
    cache.metrics.compiler_id_ns += elapsed_ns;
  }
}

std::optional<std::string>
cached_qualified_name(const clang::NamedDecl *declaration) {
  return lookup(cache.qualified_names, declaration,
                cache.metrics.qualified_name_hits,
                cache.metrics.qualified_name_misses);
}

void store_qualified_name(const clang::NamedDecl *declaration,
                          std::string value) {
  cache.qualified_names.emplace(declaration, std::move(value));
}

std::optional<std::string> cached_source_text(std::uint64_t range_key) {
  return lookup(cache.source_texts, range_key, cache.metrics.source_text_hits,
                cache.metrics.source_text_misses);
}

void store_source_text(std::uint64_t range_key, std::string value) {
  cache.source_texts.emplace(range_key, std::move(value));
}

void record_parent_chain(std::uint64_t steps, std::uint64_t elapsed_ns) {
  if (!cache.profile) {
    return;
  }
  ++cache.metrics.parent_chain_queries;
  cache.metrics.parent_chain_steps += steps;
  cache.metrics.parent_chain_ns += elapsed_ns;
}

} // namespace lexicon::clang_frontend
