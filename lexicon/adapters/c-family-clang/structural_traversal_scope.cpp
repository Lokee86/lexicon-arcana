#include "structural_traversal_scope.h"

#include <algorithm>
#include "clang/AST/ASTContext.h"
#include "structural_source.h"

namespace lexicon::clang_frontend {
TraversalScope::TraversalScope(const State &state,
                               const clang::SourceManager &sources)
    : state_(state), sources_(sources) {}

void TraversalScope::index() {
  protected_locations_.clear();
  // Loaded/PCH locations need a separate index; retain traversal for them.
  for (unsigned i = 1; i < sources_.local_sloc_entry_size(); ++i) {
    const auto &entry = sources_.getLocalSLocEntry(i);
    clang::SourceLocation spelling, expansion;
    if (entry.isFile()) {
      spelling = clang::SourceLocation::getFromRawEncoding(entry.getOffset());
      expansion = entry.getFile().getIncludeLoc();
      if (expansion.isInvalid()) expansion = spelling;
    } else {
      spelling = entry.getExpansion().getSpellingLoc();
      expansion = entry.getExpansion().getExpansionLocStart();
    }
    auto path = source_path(sources_, spelling, state_.repository_root);
    if (path && state_.owns(*path) && expansion.isValid()) {
      protected_locations_.push_back(sources_.getExpansionLoc(expansion));
    }
  }
  auto before = [&](auto left, auto right) {
    return sources_.isBeforeInTranslationUnit(left, right);
  };
  std::sort(protected_locations_.begin(), protected_locations_.end(), before);
  protected_locations_.erase(
      std::unique(protected_locations_.begin(), protected_locations_.end()),
      protected_locations_.end());
}

std::size_t TraversalScope::restrict_parent_map(clang::ASTContext &context) const {
  // Preserve loaded/PCH traversal until nested ownership can be indexed safely.
  if (sources_.loaded_sloc_entry_size() != 0) return 0;
  std::vector<clang::Decl *> roots;
  std::size_t pruned = 0;
  for (auto *declaration : context.getTranslationUnitDecl()->decls()) {
    if (can_prune(*declaration)) ++pruned;
    else roots.push_back(declaration);
  }
  // This is Clang's analysis scope, not a modified AST. Target declarations and
  // type resolution remain accessible, while getParents() indexes these roots.
  if (pruned) context.setTraversalScope(roots);
  return pruned;
}

bool TraversalScope::can_prune(const clang::Decl &declaration) const {
  if (sources_.loaded_sloc_entry_size() != 0) return false;
  if (declaration.isImplicit() ||
      llvm::isa<clang::TranslationUnitDecl>(declaration)) return false;
  const auto range = declaration.getSourceRange();
  const auto begin = range.getBegin(), end = range.getEnd();
  if (begin.isInvalid() || end.isInvalid() || begin.isMacroID() ||
      end.isMacroID() || sources_.isLoadedSourceLocation(begin) ||
      sources_.isLoadedSourceLocation(end) ||
      sources_.getFileID(begin) != sources_.getFileID(end)) return false;
  auto path = source_path(sources_, declaration.getLocation(), state_.repository_root);
  if (!path || state_.owns(*path)) {
    // External system files have no repository path; they can still be pruned
    // once their source range and protected locations have been checked.
    if (path || declaration.getLocation().isMacroID()) return false;
  }
  auto before = [&](auto left, auto right) {
    return sources_.isBeforeInTranslationUnit(left, right);
  };
  if (before(end, begin)) return false;
  auto next = std::lower_bound(protected_locations_.begin(),
                               protected_locations_.end(), begin, before);
  return next == protected_locations_.end() || before(end, *next);
}
} // namespace lexicon::clang_frontend
