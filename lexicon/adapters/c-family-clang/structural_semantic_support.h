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
                                 State &state, clang::ASTContext &context,
                                 llvm::StringRef repository_root);
std::optional<SymbolReference>
value_reference(const clang::Expr *expression, State &state,
                clang::ASTContext &context, llvm::StringRef repository_root);
std::optional<SymbolReference>
callable_reference(const clang::Expr *expression, State &state,
                   clang::ASTContext &context,
                   llvm::StringRef repository_root);
bool semantic_source_function(const clang::FunctionDecl &function);
std::vector<SymbolReference>
overload_candidates(const clang::Expr *callee, State &state,
                    clang::ASTContext &context,
                    llvm::StringRef repository_root);
std::vector<SemanticArgument>
semantic_arguments(const clang::CallExpr &call, State &state,
                   clang::ASTContext &context,
                   llvm::StringRef repository_root);
std::vector<SemanticArgument>
semantic_arguments(const clang::CXXConstructExpr &call, State &state,
                   clang::ASTContext &context,
                   llvm::StringRef repository_root);

} // namespace lexicon::clang_frontend
