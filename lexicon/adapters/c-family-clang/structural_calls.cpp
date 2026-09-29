#include "structural_calls.h"

#include <optional>
#include <string>
#include <vector>

#include "clang/AST/DeclCXX.h"
#include "clang/AST/ExprCXX.h"

#include "structural_declaration_support.h"
#include "structural_semantic_support.h"
#include "structural_source.h"

namespace lexicon::clang_frontend {
namespace {

std::string form(const clang::CallExpr &call,
                 const clang::FunctionDecl *target) {
  if (llvm::isa<clang::CXXOperatorCallExpr>(&call)) {
    return "operator";
  }
  if (target && llvm::isa<clang::CXXDestructorDecl>(target)) {
    return "destructor";
  }
  return llvm::isa<clang::CXXMemberCallExpr>(&call) ? "member" : "direct";
}

std::string resolution(const clang::CallExpr &call,
                       const clang::FunctionDecl *target,
                       const std::vector<SymbolReference> &candidates) {
  if (target) {
    return "resolved";
  }
  if (call.isTypeDependent() || call.isValueDependent()) {
    return "dependent";
  }
  if (!candidates.empty()) {
    return "ambiguous";
  }
  const auto *callee = call.getCallee();
  return callee && callee->getType()->isPointerType() ? "indirect" : "missing";
}

std::optional<SymbolReference>
receiver_type(const clang::CallExpr &call, const clang::SourceManager &sources,
              llvm::StringRef root, std::string &name) {
  if (const auto *member = llvm::dyn_cast<clang::CXXMemberCallExpr>(&call)) {
    name = member->getObjectType().getAsString();
    const auto *record = member->getRecordDecl();
    return record ? std::optional<SymbolReference>(
                        symbol_reference(record, sources, root))
                  : std::nullopt;
  }
  const auto *operator_call = llvm::dyn_cast<clang::CXXOperatorCallExpr>(&call);
  if (!operator_call || operator_call->getNumArgs() == 0) {
    return std::nullopt;
  }
  auto type = operator_call->getArg(0)->getType();
  if (type->isPointerType()) {
    type = type->getPointeeType();
  }
  name = type.getAsString();
  const auto *record = type->getAsCXXRecordDecl();
  return record ? std::optional<SymbolReference>(
                      symbol_reference(record, sources, root))
                : std::nullopt;
}

bool overload_selected(const clang::CallExpr &call) {
  const auto *callee = call.getCallee();
  if (!callee) {
    return false;
  }
  const auto *value = callee->IgnoreParenImpCasts();
  if (const auto *reference = llvm::dyn_cast<clang::DeclRefExpr>(value)) {
    return reference->hadMultipleCandidates();
  }
  if (const auto *member = llvm::dyn_cast<clang::MemberExpr>(value)) {
    return member->hadMultipleCandidates();
  }
  return llvm::isa<clang::OverloadExpr>(value);
}

bool virtual_dispatch(const clang::CallExpr &call,
                      const clang::FunctionDecl *target,
                      const clang::LangOptions &language) {
  const auto *method = llvm::dyn_cast_or_null<clang::CXXMethodDecl>(target);
  const auto *member_call = llvm::dyn_cast<clang::CXXMemberCallExpr>(&call);
  if (!method || !method->isVirtual() || !member_call) {
    return false;
  }
  const auto *member = llvm::dyn_cast<clang::MemberExpr>(
      member_call->getCallee()->IgnoreParenImpCasts());
  return member && member->performsVirtualDispatch(language);
}

} // namespace

void observe_call(State &state, clang::ASTContext &context,
                  clang::CallExpr &call, llvm::StringRef repository_root,
                  llvm::StringRef translation_unit, llvm::StringRef language) {
  if (call.getExprLoc().isInvalid()) {
    return;
  }
  auto &sources = context.getSourceManager();
  auto path = source_path(sources, call.getExprLoc(), repository_root);
  const auto *source = enclosing_function(context, call);
  if (!path || !source || source->isImplicit()) {
    return;
  }

  const auto *target = call.getDirectCallee();
  auto candidates =
      overload_candidates(call.getCallee(), sources, repository_root);
  auto target_reference =
      target ? std::optional<SymbolReference>(
                   symbol_reference(target, sources, repository_root))
             : std::nullopt;
  std::string receiver_name;
  auto receiver =
      receiver_type(call, sources, repository_root, receiver_name);
  const auto compiler_count = target ? std::size_t{1} : candidates.size();

  state.file(*path, language.str(), translation_unit.str()).calls.push_back({
      .source_compiler_id = compiler_id(source, sources),
      .form = form(call, target),
      .resolution = resolution(call, target, candidates),
      .expression = normalize_space(source_text(
          sources, context.getLangOpts(), call.getCallee()->getSourceRange())),
      .target = std::move(target_reference),
      .candidates = std::move(candidates),
      .receiver_type = std::move(receiver),
      .callee_value =
          target ? std::nullopt
                 : value_reference(call.getCallee(), sources, repository_root),
      .receiver_type_name = std::move(receiver_name),
      .virtual_dispatch =
          virtual_dispatch(call, target, context.getLangOpts()),
      .overload_selected = overload_selected(call),
      .macro_expanded = call.getExprLoc().isMacroID(),
      .compiler_candidate_count = compiler_count,
      .arguments = semantic_arguments(call, sources, context.getLangOpts(),
                                      repository_root),
      .span = source_span(sources, context.getLangOpts(),
                          call.getSourceRange(), *path),
  });
}

void observe_constructor(State &state, clang::ASTContext &context,
                         clang::CXXConstructExpr &call,
                         llvm::StringRef repository_root,
                         llvm::StringRef translation_unit,
                         llvm::StringRef language) {
  if (call.getExprLoc().isInvalid()) {
    return;
  }
  auto &sources = context.getSourceManager();
  auto path = source_path(sources, call.getExprLoc(), repository_root);
  const auto *source = enclosing_function(context, call);
  const auto *target = call.getConstructor();
  if (!path || !source || source->isImplicit() || !target) {
    return;
  }

  state.file(*path, language.str(), translation_unit.str()).calls.push_back({
      .source_compiler_id = compiler_id(source, sources),
      .form = "constructor",
      .resolution = call.isTypeDependent() || call.isValueDependent()
                        ? "dependent"
                        : "resolved",
      .expression = normalize_space(source_text(
          sources, context.getLangOpts(), call.getSourceRange())),
      .target = symbol_reference(target, sources, repository_root),
      .candidates = {},
      .receiver_type = std::nullopt,
      .callee_value = std::nullopt,
      .receiver_type_name = call.getType().getAsString(),
      .virtual_dispatch = false,
      .overload_selected = call.hadMultipleCandidates(),
      .macro_expanded = call.getExprLoc().isMacroID(),
      .compiler_candidate_count = 1,
      .arguments = semantic_arguments(call, sources, context.getLangOpts(),
                                      repository_root),
      .span = source_span(sources, context.getLangOpts(),
                          call.getSourceRange(), *path),
  });
}

} // namespace lexicon::clang_frontend
