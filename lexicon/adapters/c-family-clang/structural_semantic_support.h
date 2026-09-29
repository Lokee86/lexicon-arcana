#pragma once

#include <optional>
#include <string>
#include <vector>

#include "clang/AST/ASTContext.h"
#include "clang/AST/Expr.h"
#include "clang/AST/ExprCXX.h"

#include "structural_model.h"

namespace lexicon::clang_frontend {

SymbolReference symbol_reference(const clang::NamedDecl *declaration,
                                 const clang::SourceManager &sources,
                                 llvm::StringRef repository_root);
std::optional<SymbolReference>
value_reference(const clang::Expr *expression,
                const clang::SourceManager &sources,
                llvm::StringRef repository_root);
std::optional<SymbolReference>
callable_reference(const clang::Expr *expression,
                   const clang::SourceManager &sources,
                   llvm::StringRef repository_root);
const clang::FunctionDecl *enclosing_function(clang::ASTContext &context,
                                              const clang::Stmt &statement);
const clang::FunctionDecl *enclosing_function(clang::ASTContext &context,
                                              const clang::Decl &declaration);
std::vector<SymbolReference>
overload_candidates(const clang::Expr *callee,
                    const clang::SourceManager &sources,
                    llvm::StringRef repository_root);
std::vector<SemanticArgument>
semantic_arguments(const clang::CallExpr &call,
                   const clang::SourceManager &sources,
                   const clang::LangOptions &language,
                   llvm::StringRef repository_root);
std::vector<SemanticArgument>
semantic_arguments(const clang::CXXConstructExpr &call,
                   const clang::SourceManager &sources,
                   const clang::LangOptions &language,
                   llvm::StringRef repository_root);

} // namespace lexicon::clang_frontend
