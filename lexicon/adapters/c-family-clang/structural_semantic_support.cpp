#include "structural_semantic_support.h"

#include <deque>
#include <set>

#include "clang/AST/Decl.h"
#include "clang/AST/DeclCXX.h"
#include "clang/AST/ParentMapContext.h"

#include "structural_declaration_support.h"
#include "structural_source.h"

namespace lexicon::clang_frontend {
namespace {

bool lambda_call_operator(const clang::FunctionDecl *function) {
  const clang::FunctionDecl *current = function;
  for (unsigned depth = 0; current && depth < 8; ++depth) {
    if (const auto *method = llvm::dyn_cast<clang::CXXMethodDecl>(current)) {
      if (method->getParent()->isLambda()) {
        return true;
      }
      if (const auto *pattern = method->getInstantiatedFromMemberFunction();
          pattern && pattern != current) {
        current = pattern;
        continue;
      }
    }
    if (const auto *pattern = current->getTemplateInstantiationPattern();
        pattern && pattern != current) {
      current = pattern;
      continue;
    }
    break;
  }
  return false;
}

const clang::NamedDecl *referenced_named_decl(const clang::Expr *expression) {
  if (!expression) {
    return nullptr;
  }
  const auto *value = expression->IgnoreParenImpCasts();
  const auto *declaration = value->getReferencedDeclOfCallee();
  return llvm::dyn_cast_or_null<clang::NamedDecl>(declaration);
}

SemanticArgument argument(const clang::Expr &expression,
                          const clang::SourceManager &sources,
                          const clang::LangOptions &language,
                          llvm::StringRef repository_root) {
  return {
      .expression = normalize_space(
          source_text(sources, language, expression.getSourceRange())),
      .value = value_reference(&expression, sources, repository_root),
      .callable = callable_reference(&expression, sources, repository_root),
  };
}

} // namespace

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

std::optional<SymbolReference>
value_reference(const clang::Expr *expression,
                const clang::SourceManager &sources,
                llvm::StringRef repository_root) {
  const auto *declaration = referenced_named_decl(expression);
  if (!declaration || llvm::isa<clang::FunctionDecl>(declaration)) {
    return std::nullopt;
  }
  return symbol_reference(declaration, sources, repository_root);
}

std::optional<SymbolReference>
callable_reference(const clang::Expr *expression,
                   const clang::SourceManager &sources,
                   llvm::StringRef repository_root) {
  const auto *declaration = referenced_named_decl(expression);
  if (!llvm::isa_and_nonnull<clang::FunctionDecl>(declaration)) {
    return std::nullopt;
  }
  return symbol_reference(declaration, sources, repository_root);
}

const clang::FunctionDecl *
enclosing_function(clang::ASTContext &context, clang::DynTypedNode initial) {
  std::deque<clang::DynTypedNode> queue;
  std::set<const void *> seen;
  queue.push_back(initial);
  while (!queue.empty()) {
    auto current = queue.front();
    queue.pop_front();
    for (const auto &parent : context.getParents(current)) {
      if (const auto *function = parent.get<clang::FunctionDecl>()) {
        if (!function->isImplicit() && !lambda_call_operator(function)) {
          return function;
        }
        if (seen.insert(function).second) {
          queue.push_back(parent);
        }
        continue;
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

const clang::FunctionDecl *enclosing_function(clang::ASTContext &context,
                                              const clang::Stmt &statement) {
  return enclosing_function(context, clang::DynTypedNode::create(statement));
}

const clang::FunctionDecl *enclosing_function(clang::ASTContext &context,
                                              const clang::Decl &declaration) {
  return enclosing_function(context, clang::DynTypedNode::create(declaration));
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

std::vector<SemanticArgument>
semantic_arguments(const clang::CallExpr &call,
                   const clang::SourceManager &sources,
                   const clang::LangOptions &language,
                   llvm::StringRef repository_root) {
  std::vector<SemanticArgument> result;
  result.reserve(call.getNumArgs());
  for (const auto *value : call.arguments()) {
    result.push_back(argument(*value, sources, language, repository_root));
  }
  return result;
}

std::vector<SemanticArgument>
semantic_arguments(const clang::CXXConstructExpr &call,
                   const clang::SourceManager &sources,
                   const clang::LangOptions &language,
                   llvm::StringRef repository_root) {
  std::vector<SemanticArgument> result;
  result.reserve(call.getNumArgs());
  for (const auto *value : call.arguments()) {
    result.push_back(argument(*value, sources, language, repository_root));
  }
  return result;
}

} // namespace lexicon::clang_frontend
