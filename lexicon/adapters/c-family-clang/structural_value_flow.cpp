#include "structural_value_flow.h"

#include "clang/AST/ExprCXX.h"

#include "structural_declaration_support.h"
#include "structural_semantic_support.h"
#include "structural_source.h"

namespace lexicon::clang_frontend {
namespace {

void add_pointer_binding(State &state, clang::ASTContext &context,
                         const clang::NamedDecl &pointer,
                         const clang::Expr *initializer,
                         clang::SourceRange range,
                         llvm::StringRef repository_root,
                         llvm::StringRef translation_unit,
                         llvm::StringRef language) {
  if (!initializer) {
    return;
  }
  auto &sources = context.getSourceManager();
  auto path = source_path(sources, pointer.getLocation(), repository_root);
  auto target = callable_reference(initializer, state, context, repository_root);
  if (!path || !state.owns(*path) || !target) {
    return;
  }
  state.file(*path, language.str(), translation_unit.str())
      .pointer_bindings.push_back({
          .pointer = symbol_reference(&pointer, state, context, repository_root),
          .target = *target,
          .expression = normalize_space(
              source_text(sources, context.getLangOpts(), range)),
          .span = source_span(sources, context.getLangOpts(), range, *path),
      });
}

} // namespace

void observe_variable(State &state, clang::ASTContext &context,
                      clang::VarDecl &declaration,
                      llvm::StringRef repository_root,
                      llvm::StringRef translation_unit,
                      llvm::StringRef language) {
  if (!declaration.hasInit()) {
    return;
  }

  auto &sources = context.getSourceManager();
  if (const auto *source = enclosing_function(context, declaration)) {
    auto path =
        source_path(sources, declaration.getLocation(), repository_root);
    if (path && state.owns(*path)) {
      const auto source_id =
          ensure_callable_declaration(state, context, *source, repository_root,
                                      translation_unit, language);
      if (source_id.empty()) {
        return;
      }
      state.file(*path, language.str(), translation_unit.str())
          .accesses.push_back({
              .source_compiler_id = source_id,
              .target =
                  symbol_reference(&declaration, state, context, repository_root),
              .relation = "writes",
              .expression = declaration.getNameAsString(),
              .span = source_span(sources, context.getLangOpts(),
                                  declaration.getSourceRange(), *path),
          });
    }
  }

  if (function_pointer(declaration.getType())) {
    add_pointer_binding(state, context, declaration, declaration.getInit(),
                        declaration.getSourceRange(), repository_root,
                        translation_unit, language);
  }
}

void observe_pointer_field(State &state, clang::ASTContext &context,
                           clang::FieldDecl &declaration,
                           llvm::StringRef repository_root,
                           llvm::StringRef translation_unit,
                           llvm::StringRef language) {
  if (!function_pointer(declaration.getType()) ||
      !declaration.hasInClassInitializer()) {
    return;
  }
  add_pointer_binding(state, context, declaration,
                      declaration.getInClassInitializer(),
                      declaration.getSourceRange(), repository_root,
                      translation_unit, language);
}

void observe_pointer_assignment(State &state, clang::ASTContext &context,
                                clang::BinaryOperator &assignment,
                                llvm::StringRef repository_root,
                                llvm::StringRef translation_unit,
                                llvm::StringRef language) {
  if (assignment.getOpcode() != clang::BO_Assign ||
      !function_pointer(assignment.getLHS()->getType())) {
    return;
  }
  auto &sources = context.getSourceManager();
  auto pointer =
      value_reference(assignment.getLHS(), state, context, repository_root);
  auto target = callable_reference(assignment.getRHS(), state, context,
                                   repository_root);
  auto path = source_path(sources, assignment.getExprLoc(), repository_root);
  if (!pointer || !target || !path || !state.owns(*path)) {
    return;
  }
  state.file(*path, language.str(), translation_unit.str())
      .pointer_bindings.push_back({
          .pointer = *pointer,
          .target = *target,
          .expression = normalize_space(source_text(
              sources, context.getLangOpts(), assignment.getSourceRange())),
          .span = source_span(sources, context.getLangOpts(),
                              assignment.getSourceRange(), *path),
      });
}

void observe_designated_pointer(State &state, clang::ASTContext &context,
                                clang::DesignatedInitExpr &initializer,
                                llvm::StringRef repository_root,
                                llvm::StringRef translation_unit,
                                llvm::StringRef language) {
  for (const auto &designator : initializer.designators()) {
    if (!designator.isFieldDesignator()) {
      continue;
    }
    const auto *field = designator.getFieldDecl();
    if (field && function_pointer(field->getType())) {
      add_pointer_binding(state, context, *field, initializer.getInit(),
                          initializer.getSourceRange(), repository_root,
                          translation_unit, language);
    }
  }
}

} // namespace lexicon::clang_frontend
