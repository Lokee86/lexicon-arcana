#pragma once

#include "clang/AST/ASTContext.h"
#include "clang/AST/Decl.h"
#include "clang/AST/Expr.h"
#include "llvm/ADT/StringRef.h"

#include "structural_model.h"

namespace lexicon::clang_frontend {

void observe_value_access(State &state, clang::ASTContext &context,
                          clang::Expr &expression,
                          llvm::StringRef repository_root,
                          llvm::StringRef translation_unit,
                          llvm::StringRef language,
                          const clang::FunctionDecl *source);

void observe_variable(State &state, clang::ASTContext &context,
                      clang::VarDecl &declaration,
                      llvm::StringRef repository_root,
                      llvm::StringRef translation_unit,
                      llvm::StringRef language,
                      const clang::FunctionDecl *source);

void observe_pointer_field(State &state, clang::ASTContext &context,
                           clang::FieldDecl &declaration,
                           llvm::StringRef repository_root,
                           llvm::StringRef translation_unit,
                           llvm::StringRef language);

void observe_pointer_assignment(State &state, clang::ASTContext &context,
                                clang::BinaryOperator &assignment,
                                llvm::StringRef repository_root,
                                llvm::StringRef translation_unit,
                                llvm::StringRef language);

void observe_designated_pointer(State &state, clang::ASTContext &context,
                                clang::DesignatedInitExpr &initializer,
                                llvm::StringRef repository_root,
                                llvm::StringRef translation_unit,
                                llvm::StringRef language);

} // namespace lexicon::clang_frontend
