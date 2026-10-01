#pragma once

#include <vector>
#include "clang/AST/Decl.h"
#include "clang/Basic/SourceManager.h"
#include "structural_model.h"

namespace lexicon::clang_frontend {
// Protect owned includes/macro expansions nested inside otherwise external AST
// ranges. Pruning is deliberately disabled for ambiguous or loaded locations.
class TraversalScope {
public:
  TraversalScope(const State &state, const clang::SourceManager &sources);
  void index(); // After preprocessing and AST construction, not at consumer creation.
  bool can_prune(const clang::Decl &declaration) const;
private:
  const State &state_;
  const clang::SourceManager &sources_;
  std::vector<clang::SourceLocation> protected_locations_;
};
} // namespace lexicon::clang_frontend
