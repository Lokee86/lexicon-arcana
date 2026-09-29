#pragma once

#include <optional>
#include <string>

#include "clang/AST/Decl.h"
#include "clang/AST/Type.h"
#include "llvm/ADT/StringRef.h"

#include "structural_model.h"

namespace clang {
class ASTContext;
class DeclContext;
class FunctionDecl;
class NamedDecl;
class SourceManager;
}

namespace lexicon::clang_frontend {

std::string compiler_id(const clang::Decl *declaration,
                        const clang::SourceManager &sources);
std::string context_id(const clang::DeclContext *context,
                       const clang::SourceManager &sources);
std::string parent_type_id(const clang::DeclContext *context,
                           const clang::SourceManager &sources);
bool function_pointer(clang::QualType type);
std::string printed_type(clang::QualType type, const clang::ASTContext &context);
std::string function_signature(const clang::FunctionDecl &function,
                               const clang::ASTContext &context);
std::string anonymous_name(llvm::StringRef tag, llvm::StringRef source);
std::string context_qualified_name(const clang::DeclContext *context);
bool internal_linkage(const clang::NamedDecl &declaration);
std::optional<Declaration>
classify_declaration(clang::NamedDecl &named, const std::string &path,
                     clang::ASTContext &context);
std::string ensure_callable_declaration(State &state, clang::ASTContext &context,
                                        const clang::FunctionDecl &function,
                                        llvm::StringRef repository_root,
                                        llvm::StringRef translation_unit,
                                        llvm::StringRef language);

} // namespace lexicon::clang_frontend
