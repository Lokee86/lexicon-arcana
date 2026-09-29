#pragma once

#include "clang/AST/ASTContext.h"
#include "clang/AST/DeclCXX.h"
#include "llvm/ADT/StringRef.h"

#include "structural_model.h"

namespace lexicon::clang_frontend {

void observe_inheritance(State &state, clang::ASTContext &context,
                         clang::CXXRecordDecl &record,
                         llvm::StringRef repository_root,
                         llvm::StringRef translation_unit,
                         llvm::StringRef language);

void observe_overrides(State &state, clang::ASTContext &context,
                       clang::CXXMethodDecl &method,
                       llvm::StringRef repository_root,
                       llvm::StringRef translation_unit,
                       llvm::StringRef language);

} // namespace lexicon::clang_frontend
