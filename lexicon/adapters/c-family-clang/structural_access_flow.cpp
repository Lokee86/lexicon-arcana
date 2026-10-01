#include "structural_value_flow.h"

#include "clang/AST/ParentMapContext.h"

#include "perf.h"
#include "structural_declaration_support.h"
#include "structural_hot_path.h"
#include "structural_semantic_support.h"
#include "structural_source.h"

namespace lexicon::clang_frontend {
namespace {

enum class AccessMode { Read, Write, ReadWrite, Skip };

AccessMode access_mode(clang::ASTContext &context,
                       const clang::Expr &expression) {
  const auto started =
      hot_path_profiling() ? PerfClock::now() : PerfClock::time_point{};
  std::uint64_t steps = 0;
  const clang::Stmt *current = &expression;
  AccessMode result = AccessMode::Read;
  while (true) {
    auto parents = context.getParents(*current);
    steps += parents.size();
    if (parents.size() != 1) {
      break;
    }
    const auto *parent = parents[0].get<clang::Expr>();
    if (!parent) {
      break;
    }
    if (llvm::isa<clang::ParenExpr>(parent) ||
        llvm::isa<clang::ImplicitCastExpr>(parent)) {
      current = parent;
      continue;
    }
    if (const auto *member = llvm::dyn_cast<clang::MemberExpr>(parent)) {
      if (member->getBase() == current) {
        break;
      }
    }
    if (const auto *subscript =
            llvm::dyn_cast<clang::ArraySubscriptExpr>(parent)) {
      if (subscript->getIdx() == current) {
        break;
      }
      if (subscript->getBase() == current) {
        current = parent;
        continue;
      }
    }
    if (const auto *call = llvm::dyn_cast<clang::CallExpr>(parent);
        call && call->getCallee() == current) {
      result = AccessMode::Skip;
      break;
    }
    if (const auto *binary = llvm::dyn_cast<clang::BinaryOperator>(parent);
        binary && binary->getLHS() == current && binary->isAssignmentOp()) {
      result = binary->getOpcode() == clang::BO_Assign ? AccessMode::Write
                                                       : AccessMode::ReadWrite;
      break;
    }
    if (const auto *unary = llvm::dyn_cast<clang::UnaryOperator>(parent);
        unary && unary->getSubExpr() == current &&
        unary->isIncrementDecrementOp()) {
      result = AccessMode::ReadWrite;
      break;
    }
    break;
  }

  const auto elapsed_ns =
      hot_path_profiling()
          ? static_cast<std::uint64_t>(
                std::chrono::duration_cast<std::chrono::nanoseconds>(
                    PerfClock::now() - started)
                    .count())
          : 0;
  record_parent_chain(steps, elapsed_ns);
  return result;
}

} // namespace

void observe_value_access(State &state, clang::ASTContext &context,
                          clang::Expr &expression,
                          llvm::StringRef repository_root,
                          llvm::StringRef translation_unit,
                          llvm::StringRef language,
                          const clang::FunctionDecl *source) {
  const auto mode = access_mode(context, expression);
  if (mode == AccessMode::Skip) {
    return;
  }

  auto &sources = context.getSourceManager();
  auto path = source_path(sources, expression.getExprLoc(), repository_root);
  if (!path || !state.owns(*path)) {
    return;
  }

  auto target = value_reference(&expression, state, context, repository_root);
  if (!source || !target) {
    return;
  }

  const auto source_id =
      ensure_callable_declaration(state, context, *source, repository_root,
                                  translation_unit, language);
  if (source_id.empty()) {
    return;
  }

  const auto expression_text = normalize_space(source_text(
      sources, context.getLangOpts(), expression.getSourceRange()));
  const auto span = source_span(sources, context.getLangOpts(),
                                expression.getSourceRange(), *path);
  auto &accesses =
      state.file(*path, language.str(), translation_unit.str()).accesses;

  auto append = [&](llvm::StringRef relation) {
    accesses.push_back({
        .source_compiler_id = source_id,
        .target = *target,
        .relation = relation.str(),
        .expression = expression_text,
        .span = span,
    });
  };

  if (mode == AccessMode::Read || mode == AccessMode::ReadWrite) {
    append("reads");
  }
  if (mode == AccessMode::Write || mode == AccessMode::ReadWrite) {
    append("writes");
  }
}

} // namespace lexicon::clang_frontend
