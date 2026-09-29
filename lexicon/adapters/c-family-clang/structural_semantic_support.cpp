#include "structural_semantic_support.h"

#include <deque>
#include <set>

#include "clang/AST/ParentMapContext.h"

#include "structural_declaration_support.h"
#include "structural_source.h"

namespace lexicon::clang_frontend {

SymbolReference symbol_reference(const clang::NamedDecl *declaration,
                                 const clang::SourceManager &sources,
                                 llvm::StringRef repository_root) {
  if (!declaration) {
    return {};
  }
  auto path = source_path(sources, declaration->getLocation(), repository_root);
  return {
      .compiler_id = compiler_id(declaration, sources),
      .path = path.value_or(std::string()),
      .qualified_name = declaration->getQualifiedNameAsString(),
      .kind = declaration->getDeclKindName(),
      .external = !path.has_value(),
  };
}

const clang::FunctionDecl *enclosing_function(clang::ASTContext &context,
                                              const clang::Stmt &statement) {
  std::deque<clang::DynTypedNode> queue;
  std::set<const void *> seen;
  queue.push_back(clang::DynTypedNode::create(statement));
  while (!queue.empty()) {
    auto current = queue.front();
    queue.pop_front();
    for (const auto &parent : context.getParents(current)) {
      if (const auto *function = parent.get<clang::FunctionDecl>()) {
        return function;
      }
      if (const auto *stmt = parent.get<clang::Stmt>()) {
        if (seen.insert(stmt).second) {
          queue.push_back(parent);
        }
        continue;
      }
      if (const auto *decl = parent.get<clang::Decl>()) {
        if (seen.insert(decl).second) {
          queue.push_back(parent);
        }
      }
    }
  }
  return nullptr;
}

std::vector<SymbolReference>
overload_candidates(const clang::Expr *callee,
                    const clang::SourceManager &sources,
                    llvm::StringRef repository_root) {
  std::vector<SymbolReference> result;
  if (!callee) {
    return result;
  }
  const auto *expression = callee->IgnoreParenImpCasts();
  const auto *overload = llvm::dyn_cast<clang::OverloadExpr>(expression);
  if (!overload) {
    return result;
  }
  for (auto *declaration : overload->decls()) {
    result.push_back(
        symbol_reference(declaration, sources, repository_root));
  }
  return result;
}

std::vector<std::string> argument_texts(const clang::CallExpr &call,
                                        const clang::SourceManager &sources,
                                        const clang::LangOptions &language) {
  std::vector<std::string> result;
  result.reserve(call.getNumArgs());
  for (const auto *argument : call.arguments()) {
    result.push_back(
        normalize_space(source_text(sources, language, argument->getSourceRange())));
  }
  return result;
}

std::vector<std::string>
constructor_argument_texts(const clang::CXXConstructExpr &call,
                           const clang::SourceManager &sources,
                           const clang::LangOptions &language) {
  std::vector<std::string> result;
  result.reserve(call.getNumArgs());
  for (const auto *argument : call.arguments()) {
    result.push_back(
        normalize_space(source_text(sources, language, argument->getSourceRange())));
  }
  return result;
}

} // namespace lexicon::clang_frontend
