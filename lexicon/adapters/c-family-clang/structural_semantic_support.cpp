#include "structural_semantic_support.h"

#include <deque>
#include <set>

#include "clang/AST/Decl.h"
#include "clang/AST/DeclCXX.h"
#include "clang/AST/ParentMapContext.h"

#include "structural_declaration_support.h"
#include "structural_source.h"
#include "structural_hot_path.h"
#include "perf.h"

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

SemanticArgument argument(const clang::Expr &expression, State &state,
                          clang::ASTContext &context,
                          llvm::StringRef repository_root) {
  auto &sources = context.getSourceManager();
  return {
      .expression = normalize_space(source_text(
          sources, context.getLangOpts(), expression.getSourceRange())),
      .value = value_reference(&expression, state, context, repository_root),
      .callable =
          callable_reference(&expression, state, context, repository_root),
  };
}

} // namespace

SymbolReference symbol_reference(const clang::NamedDecl *declaration,
                                 State &state, clang::ASTContext &context,
                                 llvm::StringRef repository_root) {
  if (!declaration) {
    return {};
  }
  auto &sources = context.getSourceManager();
  auto path = source_path(sources, declaration->getLocation(), repository_root);
  if (path && !state.owns(*path)) {
    record_context_identity(state, context, *declaration, repository_root);
  }
  return {
      .compiler_id = compiler_id(declaration, sources),
      .path = path.value_or(std::string()),
      .qualified_name = qualified_name(*declaration),
      .kind = declaration->getDeclKindName(),
      .external = !path.has_value(),
  };
}

std::optional<SymbolReference>
value_reference(const clang::Expr *expression, State &state,
                clang::ASTContext &context,
                llvm::StringRef repository_root) {
  const auto *declaration = referenced_named_decl(expression);
  if (!declaration || llvm::isa<clang::FunctionDecl>(declaration)) {
    return std::nullopt;
  }
  return symbol_reference(declaration, state, context, repository_root);
}

std::optional<SymbolReference>
callable_reference(const clang::Expr *expression, State &state,
                   clang::ASTContext &context,
                   llvm::StringRef repository_root) {
  const auto *declaration = referenced_named_decl(expression);
  if (!llvm::isa_and_nonnull<clang::FunctionDecl>(declaration)) {
    return std::nullopt;
  }
  return symbol_reference(declaration, state, context, repository_root);
}

const clang::FunctionDecl *
enclosing_function(clang::ASTContext &context, clang::DynTypedNode initial) {
  const auto started = hot_path_profiling() ? PerfClock::now() : PerfClock::time_point{};
  std::uint64_t steps = 0;
  std::deque<clang::DynTypedNode> queue;
  std::set<const void *> seen;
  queue.push_back(initial);
  const clang::FunctionDecl *result = nullptr;
  while (!queue.empty() && !result) {
    auto current = queue.front();
    queue.pop_front();
    const auto parents = context.getParents(current);
    steps += parents.size();
    for (const auto &parent : parents) {
      if (const auto *function = parent.get<clang::FunctionDecl>()) {
        if (!function->isImplicit() && !lambda_call_operator(function)) {
          result = function;
          break;
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
  const auto elapsed_ns = hot_path_profiling()
                              ? static_cast<std::uint64_t>(
                                    std::chrono::duration_cast<std::chrono::nanoseconds>(
                                        PerfClock::now() - started)
                                        .count())
                              : 0;
  record_parent_chain(steps, elapsed_ns);
  return result;
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
overload_candidates(const clang::Expr *callee, State &state,
                    clang::ASTContext &context,
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
        symbol_reference(declaration, state, context, repository_root));
  }
  return result;
}

std::vector<SemanticArgument>
semantic_arguments(const clang::CallExpr &call, State &state,
                   clang::ASTContext &context,
                   llvm::StringRef repository_root) {
  std::vector<SemanticArgument> result;
  result.reserve(call.getNumArgs());
  for (const auto *value : call.arguments()) {
    result.push_back(argument(*value, state, context, repository_root));
  }
  return result;
}

std::vector<SemanticArgument>
semantic_arguments(const clang::CXXConstructExpr &call, State &state,
                   clang::ASTContext &context,
                   llvm::StringRef repository_root) {
  std::vector<SemanticArgument> result;
  result.reserve(call.getNumArgs());
  for (const auto *value : call.arguments()) {
    result.push_back(argument(*value, state, context, repository_root));
  }
  return result;
}

} // namespace lexicon::clang_frontend
