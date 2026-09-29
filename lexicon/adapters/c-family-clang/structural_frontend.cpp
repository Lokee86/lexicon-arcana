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
        language_(std::move(language)) {}

  bool VisitDecl(clang::Decl *declaration) {
    if (!declaration || declaration->isImplicit() ||
        declaration->getLocation().isInvalid()) {
      return true;
    }
    auto path = source_path(sources_, declaration->getLocation(), root_);
    if (!path) {
      return true;
    }
    auto *named = llvm::dyn_cast<clang::NamedDecl>(declaration);
    if (!named ||
        (llvm::isa<clang::ParmVarDecl>(named) && named->getName().empty())) {
      return true;
    }

    auto observation = classify_declaration(*named, *path, context_);
    if (observation) {
      state_.file(*path, language_, translation_unit_)
          .declarations.push_back(std::move(*observation));
    }
    return true;
  }

  bool VisitCXXRecordDecl(clang::CXXRecordDecl *record) {
    if (record) {
      observe_inheritance(state_, context_, *record, root_, translation_unit_,
                          language_);
    }
    return true;
  }

  bool VisitCXXMethodDecl(clang::CXXMethodDecl *method) {
    if (method) {
      observe_overrides(state_, context_, *method, root_, translation_unit_,
                        language_);
    }
    return true;
  }

  bool VisitCallExpr(clang::CallExpr *call) {
    if (call) {
      observe_call(state_, context_, *call, root_, translation_unit_, language_);
    }
    return true;
  }

  bool VisitCXXConstructExpr(clang::CXXConstructExpr *call) {
    if (call) {
      observe_constructor(state_, context_, *call, root_, translation_unit_,
                          language_);
    }
    return true;
  }

  bool VisitDeclRefExpr(clang::DeclRefExpr *expression) {
    if (expression) {
      observe_value_access(state_, context_, *expression, root_,
                           translation_unit_, language_);
    }
    return true;
  }

  bool VisitMemberExpr(clang::MemberExpr *expression) {
    if (expression) {
      observe_value_access(state_, context_, *expression, root_,
                           translation_unit_, language_);
    }
    return true;
  }

  bool VisitVarDecl(clang::VarDecl *declaration) {
    if (declaration) {
      observe_variable(state_, context_, *declaration, root_,
                       translation_unit_, language_);
    }
    return true;
  }

  bool VisitFieldDecl(clang::FieldDecl *declaration) {
    if (declaration) {
      observe_pointer_field(state_, context_, *declaration, root_,
                            translation_unit_, language_);
    }
    return true;
  }

  bool VisitBinaryOperator(clang::BinaryOperator *assignment) {
    if (assignment) {
      observe_pointer_assignment(state_, context_, *assignment, root_,
                                 translation_unit_, language_);
    }
    return true;
  }

  bool VisitDesignatedInitExpr(clang::DesignatedInitExpr *initializer) {
    if (initializer) {
      observe_designated_pointer(state_, context_, *initializer, root_,
                                 translation_unit_, language_);
    }
    return true;
  }

private:
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
      : visitor_(state, context, std::move(root),
                 std::move(translation_unit), std::move(language)) {}

  void HandleTranslationUnit(clang::ASTContext &context) override {
    visitor_.TraverseDecl(context.getTranslationUnitDecl());
  }

private:
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
