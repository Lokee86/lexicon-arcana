#include "structural_source.h"

#include <cctype>
#include <filesystem>

#include "clang/Lex/Lexer.h"

namespace lexicon::clang_frontend {

std::optional<std::string> repository_path(llvm::StringRef value,
                                           llvm::StringRef root) {
  if (value.empty() || value.starts_with("<")) {
    return std::nullopt;
  }
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

std::optional<std::string> source_path(const clang::SourceManager &sources,
                                       clang::SourceLocation location,
                                       llvm::StringRef root) {
  auto spelling = sources.getSpellingLoc(location);
  return repository_path(sources.getFilename(spelling), root);
}

Span source_span(const clang::SourceManager &sources,
                 const clang::LangOptions &language,
                 clang::SourceRange range, const std::string &path) {
  auto begin = sources.getSpellingLoc(range.getBegin());
  auto end = sources.getSpellingLoc(range.getEnd());
  auto token_end = clang::Lexer::getLocForEndOfToken(end, 0, sources, language);
  if (token_end.isValid()) {
    end = token_end;
  }
  return {
      .path = path,
      .start_line = sources.getSpellingLineNumber(begin),
      .start_column = sources.getSpellingColumnNumber(begin),
      .end_line = sources.getSpellingLineNumber(end),
      .end_column = sources.getSpellingColumnNumber(end),
  };
}

std::string source_text(const clang::SourceManager &sources,
                        const clang::LangOptions &language,
                        clang::SourceRange range) {
  if (range.isInvalid()) {
    return {};
  }
  bool invalid = false;
  auto value = clang::Lexer::getSourceText(
      clang::CharSourceRange::getTokenRange(range), sources, language, &invalid);
  return invalid ? std::string() : value.str();
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
