#include "structural_frontend.h"

#include <string>
#include <utility>

#include "clang/AST/ASTConsumer.h"
#include "clang/AST/RecursiveASTVisitor.h"
#include "clang/Frontend/CompilerInstance.h"

#include "structural_calls.h"
#include "structural_declaration_support.h"
#include "structural_relationships.h"
#include "structural_source.h"
#include "perf.h"
#include "structural_hot_path.h"
#include "structural_value_flow.h"

namespace lexicon::clang_frontend {
namespace {

class SemanticVisitor
    : public clang::RecursiveASTVisitor<SemanticVisitor> {
public:
  SemanticVisitor(State &state, clang::ASTContext &context, std::string root,
                  std::string translation_unit, std::string language)
      : state_(state), context_(context), sources_(context.getSourceManager()),
        root_(std::move(root)), translation_unit_(std::move(translation_unit)),
        language_(std::move(language)) {
    reset_hot_path_context(perf_enabled());
  }

  bool VisitDecl(clang::Decl *declaration) {
    if (!declaration || declaration->isImplicit() ||
        declaration->getLocation().isInvalid()) {
      return true;
    }
    auto path = source_path(sources_, declaration->getLocation(), root_);
    if (!path || !state_.owns(*path)) {
      return true;
    }
    auto *named = llvm::dyn_cast<clang::NamedDecl>(declaration);
    if (!named ||
        (llvm::isa<clang::ParmVarDecl>(named) && named->getName().empty())) {
      return true;
    }

    auto observation = classify_declaration(*named, *path, context_);
    if (observation) {
      auto &file = state_.file(*path, language_, translation_unit_);
      file.declaration_compiler_ids.insert(observation->compiler_id);
      file.declarations.push_back(std::move(*observation));
    }
    return true;
  }

  bool VisitCXXRecordDecl(clang::CXXRecordDecl *record) {
    if (record && owned(record->getLocation())) {
      observe_inheritance(state_, context_, *record, root_, translation_unit_,
                          language_);
    }
    return true;
  }

  bool VisitCXXMethodDecl(clang::CXXMethodDecl *method) {
    if (method && owned(method->getLocation())) {
      observe_overrides(state_, context_, *method, root_, translation_unit_,
                        language_);
    }
    return true;
  }

  bool VisitCallExpr(clang::CallExpr *call) {
    if (call && owned(call->getExprLoc())) {
      observe_call(state_, context_, *call, root_, translation_unit_, language_);
    }
    return true;
  }

  bool VisitCXXConstructExpr(clang::CXXConstructExpr *call) {
    if (call && owned(call->getExprLoc())) {
      observe_constructor(state_, context_, *call, root_, translation_unit_,
                          language_);
    }
    return true;
  }

  bool VisitDeclRefExpr(clang::DeclRefExpr *expression) {
    if (expression && owned(expression->getExprLoc())) {
      observe_value_access(state_, context_, *expression, root_,
                           translation_unit_, language_);
    }
    return true;
  }

  bool VisitMemberExpr(clang::MemberExpr *expression) {
    if (expression && owned(expression->getExprLoc())) {
      observe_value_access(state_, context_, *expression, root_,
                           translation_unit_, language_);
    }
    return true;
  }

  bool VisitVarDecl(clang::VarDecl *declaration) {
    if (declaration && owned(declaration->getLocation())) {
      observe_variable(state_, context_, *declaration, root_,
                       translation_unit_, language_);
    }
    return true;
  }

  bool VisitFieldDecl(clang::FieldDecl *declaration) {
    if (declaration && owned(declaration->getLocation())) {
      observe_pointer_field(state_, context_, *declaration, root_,
                            translation_unit_, language_);
    }
    return true;
  }

  bool VisitBinaryOperator(clang::BinaryOperator *assignment) {
    if (assignment && owned(assignment->getExprLoc())) {
      observe_pointer_assignment(state_, context_, *assignment, root_,
                                 translation_unit_, language_);
    }
    return true;
  }

  bool VisitDesignatedInitExpr(clang::DesignatedInitExpr *initializer) {
    if (initializer && owned(initializer->getExprLoc())) {
      observe_designated_pointer(state_, context_, *initializer, root_,
                                 translation_unit_, language_);
    }
    return true;
  }

private:
  bool owned(clang::SourceLocation location) const {
    if (location.isInvalid()) {
      return false;
    }
    auto path = source_path(sources_, location, root_);
    return path && state_.owns(*path);
  }

  State &state_;
  clang::ASTContext &context_;
  clang::SourceManager &sources_;
  std::string root_;
  std::string translation_unit_;
  std::string language_;
};

class VisitorConsumer final : public clang::ASTConsumer {
public:
  VisitorConsumer(State &state, clang::ASTContext &context, std::string root,
                  std::string translation_unit, std::string language)
      : state_(state), visitor_(state, context, std::move(root),
                               std::move(translation_unit),
                               std::move(language)) {}

  void HandleTranslationUnit(clang::ASTContext &context) override {
    const auto started = PerfClock::now();
    visitor_.TraverseDecl(context.getTranslationUnitDecl());
    state_.semantic_analysis_ns +=
        std::chrono::duration_cast<std::chrono::nanoseconds>(
            PerfClock::now() - started)
            .count();

    const auto metrics = hot_path_metrics();
    emit_perf(
        "c-family.clang.hot_path", std::chrono::nanoseconds(0),
        {{"repository_path_cache_hits", metrics.repository_path_hits},
         {"repository_path_cache_misses", metrics.repository_path_misses},
         {"compiler_id_cache_hits", metrics.compiler_id_hits},
         {"compiler_id_cache_misses", metrics.compiler_id_misses},
         {"compiler_id_ns", metrics.compiler_id_ns},
         {"qualified_name_cache_hits", metrics.qualified_name_hits},
         {"qualified_name_cache_misses", metrics.qualified_name_misses},
         {"source_text_cache_hits", metrics.source_text_hits},
         {"source_text_cache_misses", metrics.source_text_misses},
         {"parent_chain_queries", metrics.parent_chain_queries},
         {"parent_chain_steps", metrics.parent_chain_steps},
         {"parent_chain_ns", metrics.parent_chain_ns}});
  }

private:
  State &state_;
  SemanticVisitor visitor_;
};

} // namespace

std::unique_ptr<clang::ASTConsumer>
make_ast_consumer(State &state, clang::CompilerInstance &compiler,
                  std::string repository_root, std::string translation_unit,
                  std::string language) {
  return std::make_unique<VisitorConsumer>(
      state, compiler.getASTContext(), std::move(repository_root),
      std::move(translation_unit), std::move(language));
}

} // namespace lexicon::clang_frontend
