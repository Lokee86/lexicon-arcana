#include "structural_relationships.h"

#include "structural_declaration_support.h"
#include "structural_semantic_support.h"
#include "structural_source.h"

namespace lexicon::clang_frontend {

void observe_inheritance(State &state, clang::ASTContext &context,
                         clang::CXXRecordDecl &record,
                         llvm::StringRef repository_root,
                         llvm::StringRef translation_unit,
                         llvm::StringRef language) {
  if (!record.isThisDeclarationADefinition() || record.isImplicit()) {
    return;
  }
  auto &sources = context.getSourceManager();
  auto path = source_path(sources, record.getLocation(), repository_root);
  if (!path) {
    return;
  }
  auto source_id = compiler_id(&record, sources);
  for (const auto &base : record.bases()) {
    state.file(*path, language.str(), translation_unit.str())
        .relationships.push_back({
            .kind = "extends",
            .source_compiler_id = source_id,
            .target = symbol_reference(base.getType()->getAsCXXRecordDecl(),
                                       sources, repository_root),
            .expression = normalize_space(source_text(
                sources, context.getLangOpts(), base.getSourceRange())),
            .span = source_span(sources, context.getLangOpts(),
                                base.getSourceRange(), *path),
        });
  }
}

void observe_overrides(State &state, clang::ASTContext &context,
                       clang::CXXMethodDecl &method,
                       llvm::StringRef repository_root,
                       llvm::StringRef translation_unit,
                       llvm::StringRef language) {
  if (method.isImplicit()) {
    return;
  }
  auto &sources = context.getSourceManager();
  auto path = source_path(sources, method.getLocation(), repository_root);
  if (!path) {
    return;
  }
  auto source_id = compiler_id(&method, sources);
  for (const auto *target : method.overridden_methods()) {
    state.file(*path, language.str(), translation_unit.str())
        .relationships.push_back({
            .kind = "overrides",
            .source_compiler_id = source_id,
            .target = symbol_reference(target, sources, repository_root),
            .expression = method.getQualifiedNameAsString(),
            .span = source_span(
                sources, context.getLangOpts(),
                clang::SourceRange(method.getLocation()), *path),
        });
  }
}

} // namespace lexicon::clang_frontend
