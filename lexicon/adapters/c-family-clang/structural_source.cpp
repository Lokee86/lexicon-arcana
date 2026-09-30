#include "structural_source.h"

#include <cctype>
#include <filesystem>
#include <tuple>
#include <unordered_map>

#include "clang/Lex/Lexer.h"

#include "structural_hot_path.h"

namespace lexicon::clang_frontend {
namespace {

std::optional<std::string> resolve_repository_path(llvm::StringRef value,
                                                   llvm::StringRef root) {
  std::error_code error;
  auto path = std::filesystem::path(value.str());
  if (path.is_relative()) {
    path = std::filesystem::absolute(path, error);
  }
  if (error) {
    return std::nullopt;
  }
  auto base = std::filesystem::weakly_canonical(root.str(), error);
  if (error) {
    return std::nullopt;
  }
  path = std::filesystem::weakly_canonical(path, error);
  if (error) {
    return std::nullopt;
  }
  auto relative = std::filesystem::relative(path, base, error);
  if (error || relative.empty()) {
    return std::nullopt;
  }
  auto text = relative.generic_string();
  if (text == ".." || text.starts_with("../")) {
    return std::nullopt;
  }
  return text;
}

} // namespace

std::optional<std::string> repository_path(llvm::StringRef value,
                                           llvm::StringRef root) {
  if (value.empty() || value.starts_with("<")) {
    return std::nullopt;
  }

  // Source attribution is on the AST hot path. Clang repeatedly reports the
  // same physical files for declarations, references, calls, and PP events;
  // resolving those paths through the filesystem for every node dominates
  // large translation units. Cache per worker thread while preserving the
  // canonical/symlink-aware first resolution.
  thread_local std::unordered_map<std::string, std::optional<std::string>> cache;
  std::string key;
  key.reserve(root.size() + value.size() + 1);
  key.append(root.data(), root.size());
  key.push_back('\0');
  key.append(value.data(), value.size());

  if (const auto found = cache.find(key); found != cache.end()) {
    record_repository_path_cache(true);
    return found->second;
  }
  record_repository_path_cache(false);
  auto resolved = resolve_repository_path(value, root);
  cache.emplace(std::move(key), resolved);
  return resolved;
}

std::optional<std::string> source_path(const clang::SourceManager &sources,
                                       clang::SourceLocation location,
                                       llvm::StringRef root) {
  auto spelling = sources.getSpellingLoc(location);
  if (auto path = repository_path(sources.getFilename(spelling), root)) {
    return path;
  }
  auto expansion = sources.getExpansionLoc(location);
  return repository_path(sources.getFilename(expansion), root);
}

Span source_span(const clang::SourceManager &sources,
                 const clang::LangOptions &language,
                 clang::SourceRange range, const std::string &path) {
  auto begin = sources.getSpellingLoc(range.getBegin());
  auto end = sources.getSpellingLoc(range.getEnd());
  if (begin.isInvalid() || end.isInvalid() ||
      sources.getFileID(begin) != sources.getFileID(end) ||
      sources.getFileOffset(end) < sources.getFileOffset(begin)) {
    begin = sources.getExpansionLoc(range.getBegin());
    end = sources.getExpansionLoc(range.getEnd());
  }

  auto token_end = clang::Lexer::getLocForEndOfToken(end, 0, sources, language);
  if (token_end.isValid() &&
      sources.getFileID(token_end) == sources.getFileID(begin) &&
      sources.getFileOffset(token_end) >= sources.getFileOffset(begin)) {
    end = token_end;
  }
  auto start_line = sources.getSpellingLineNumber(begin);
  auto start_column = sources.getSpellingColumnNumber(begin);
  auto end_line = sources.getSpellingLineNumber(end);
  auto end_column = sources.getSpellingColumnNumber(end);

  // Some macro-generated declarations have a valid source-written start but
  // an end location that resolves to no presumed file position. Preserve the
  // known anchor rather than publishing an invalid zero-coordinate span.
  if (start_line != 0 && start_column != 0 &&
      (end_line == 0 || end_column == 0 ||
       std::tie(end_line, end_column) < std::tie(start_line, start_column))) {
    end_line = start_line;
    end_column = start_column;
  }

  return {
      .path = path,
      .start_line = start_line,
      .start_column = start_column,
      .end_line = end_line,
      .end_column = end_column,
  };
}

std::string source_text(const clang::SourceManager &sources,
                        const clang::LangOptions &language,
                        clang::SourceRange range) {
  if (range.isInvalid()) {
    return {};
  }
  const auto begin = static_cast<std::uint64_t>(range.getBegin().getRawEncoding());
  const auto end = static_cast<std::uint64_t>(range.getEnd().getRawEncoding());
  const auto key = (begin << 32) | end;
  if (auto cached = cached_source_text(key)) {
    return *cached;
  }

  bool invalid = false;
  auto value = clang::Lexer::getSourceText(
      clang::CharSourceRange::getTokenRange(range), sources, language, &invalid);
  auto text = invalid ? std::string() : value.str();
  store_source_text(key, text);
  return text;
}

std::string normalize_space(llvm::StringRef value) {
  std::string result;
  bool pending_space = false;
  for (unsigned char byte : value.trim()) {
    if (std::isspace(byte)) {
      pending_space = !result.empty();
      continue;
    }
    if (pending_space) {
      result.push_back(' ');
      pending_space = false;
    }
    result.push_back(static_cast<char>(byte));
  }
  return result;
}

} // namespace lexicon::clang_frontend
