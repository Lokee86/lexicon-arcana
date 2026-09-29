#pragma once

#include <optional>
#include <string>

#include "clang/Basic/LangOptions.h"
#include "clang/Basic/SourceLocation.h"
#include "clang/Basic/SourceManager.h"
#include "llvm/ADT/StringRef.h"

#include "structural_model.h"

namespace lexicon::clang_frontend {

std::optional<std::string> repository_path(llvm::StringRef value,
                                           llvm::StringRef root);
std::optional<std::string> source_path(const clang::SourceManager &sources,
                                       clang::SourceLocation location,
                                       llvm::StringRef root);
Span source_span(const clang::SourceManager &sources,
                 const clang::LangOptions &language,
                 clang::SourceRange range, const std::string &path);
std::string source_text(const clang::SourceManager &sources,
                        const clang::LangOptions &language,
                        clang::SourceRange range);
std::string normalize_space(llvm::StringRef value);

} // namespace lexicon::clang_frontend
