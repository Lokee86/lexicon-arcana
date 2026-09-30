#include "structural_value_flow.h"

#include "clang/AST/ParentMapContext.h"

#include "structural_declaration_support.h"
#include "structural_semantic_support.h"
#include "structural_source.h"

namespace lexicon::clang_frontend {
namespace {

enum class AccessMode { Read, Write, ReadWrite, Skip };

AccessMode access_mode(clang::ASTContext &context,
                       const clang::Expr &expression) {
  const clang::Stmt *current = &expression;
  while (true) {
    auto parents = context.getParents(*current);
    if (parents.size() != 1) {
      return AccessMode::Read;
    }
    const auto *parent = parents[0].get<clang::Expr>();
    if (!parent) {
      return AccessMode::Read;
    }
    if (llvm::isa<clang::ParenExpr>(parent) ||
        llvm::isa<clang::ImplicitCastExpr>(parent)) {
      current = parent;
      continue;
    }
    if (const auto *member = llvm::dyn_cast<clang::MemberExpr>(parent)) {
      if (member->getBase() == current) {
        return AccessMode::Read;
      }
    }
    if (const auto *subscript =
            llvm::dyn_cast<clang::ArraySubscriptExpr>(parent)) {
      if (subscript->getIdx() == current) {
        return AccessMode::Read;
      }
      if (subscript->getBase() == current) {
        current = parent;
        continue;
      }
    }
    if (const auto *call = llvm::dyn_cast<clang::CallExpr>(parent);
        call && call->getCallee() == current) {
      return AccessMode::Skip;
    }
    if (const auto *binary = llvm::dyn_cast<clang::BinaryOperator>(parent);
        binary && binary->getLHS() == current && binary->isAssignmentOp()) {
      return binary->getOpcode() == clang::BO_Assign ? AccessMode::Write
                                                      : AccessMode::ReadWrite;
    }
    if (const auto *unary = llvm::dyn_cast<clang::UnaryOperator>(parent);
        unary && unary->getSubExpr() == current &&
        unary->isIncrementDecrementOp()) {
      return AccessMode::ReadWrite;
    }
    return AccessMode::Read;
  }
}

void emit_access(State &state, clang::ASTContext &context,
                 clang::Expr &expression, llvm::StringRef repository_root,
                 llvm::StringRef translation_unit, llvm::StringRef language,
                 llvm::StringRef relation) {
  auto &sources = context.getSourceManager();
  auto path = source_path(sources, expression.getExprLoc(), repository_root);
  const auto *source = enclosing_function(context, expression);
  auto target =
      value_reference(&expression, state, context, repository_root);
  if (!path || !state.owns(*path) || !source || source->isImplicit() ||
      !target) {
    return;
  }
  const auto source_id =
      ensure_callable_declaration(state, context, *source, repository_root,
                                  translation_unit, language);
  if (source_id.empty()) {
    return;
  }
  state.file(*path, language.str(), translation_unit.str()).accesses.push_back({
      .source_compiler_id = source_id,
      .target = *target,
      .relation = relation.str(),
      .expression = normalize_space(source_text(
          sources, context.getLangOpts(), expression.getSourceRange())),
      .span = source_span(sources, context.getLangOpts(),
                          expression.getSourceRange(), *path),
  });
}

} // namespace

void observe_value_access(State &state, clang::ASTContext &context,
                          clang::Expr &expression,
                          llvm::StringRef repository_root,
                          llvm::StringRef translation_unit,
                          llvm::StringRef language) {
  const auto mode = access_mode(context, expression);
  if (mode == AccessMode::Read || mode == AccessMode::ReadWrite) {
    emit_access(state, context, expression, repository_root, translation_unit,
                language, "reads");
  }
  if (mode == AccessMode::Write || mode == AccessMode::ReadWrite) {
    emit_access(state, context, expression, repository_root, translation_unit,
                language, "writes");
  }
}

} // namespace lexicon::clang_frontend
