#pragma once

#include "clang/AST/ASTContext.h"
#include "clang/AST/Expr.h"
#include "clang/AST/ExprCXX.h"
#include "llvm/ADT/StringRef.h"

#include "structural_model.h"

namespace lexicon::clang_frontend {

void observe_call(State &state, clang::ASTContext &context,
                  clang::CallExpr &call, llvm::StringRef repository_root,
                  llvm::StringRef translation_unit, llvm::StringRef language);

void observe_constructor(State &state, clang::ASTContext &context,
                         clang::CXXConstructExpr &call,
                         llvm::StringRef repository_root,
                         llvm::StringRef translation_unit,
                         llvm::StringRef language);

} // namespace lexicon::clang_frontend
