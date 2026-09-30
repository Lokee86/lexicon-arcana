#pragma once

#include <cstdint>
#include <optional>
#include <string>

#include "llvm/ADT/StringRef.h"

namespace clang {
class Decl;
class NamedDecl;
}

namespace lexicon::clang_frontend {

struct HotPathMetrics {
  std::uint64_t repository_path_hits = 0;
  std::uint64_t repository_path_misses = 0;
  std::uint64_t compiler_id_hits = 0;
  std::uint64_t compiler_id_misses = 0;
  std::uint64_t compiler_id_ns = 0;
  std::uint64_t qualified_name_hits = 0;
  std::uint64_t qualified_name_misses = 0;
  std::uint64_t source_text_hits = 0;
  std::uint64_t source_text_misses = 0;
  std::uint64_t parent_chain_queries = 0;
  std::uint64_t parent_chain_steps = 0;
  std::uint64_t parent_chain_ns = 0;
};

void reset_hot_path_context(bool profile);
bool hot_path_profiling();
HotPathMetrics hot_path_metrics();

void record_repository_path_cache(bool hit);
std::optional<std::string> cached_compiler_id(const clang::Decl *declaration);
void store_compiler_id(const clang::Decl *declaration, std::string value,
                       std::uint64_t elapsed_ns);
std::optional<std::string>
cached_qualified_name(const clang::NamedDecl *declaration);
void store_qualified_name(const clang::NamedDecl *declaration,
                          std::string value);
std::optional<std::string> cached_source_text(std::uint64_t range_key);
void store_source_text(std::uint64_t range_key, std::string value);
void record_parent_chain(std::uint64_t steps, std::uint64_t elapsed_ns);

} // namespace lexicon::clang_frontend
