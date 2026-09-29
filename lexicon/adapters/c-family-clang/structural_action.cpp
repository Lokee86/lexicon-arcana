#include "structural_frontend.h"

#include <memory>
#include <string>
#include <utility>

#include "clang/Basic/Diagnostic.h"
#include "clang/Frontend/CompilerInstance.h"
#include "clang/Frontend/FrontendAction.h"
#include "clang/Lex/Lexer.h"
#include "clang/Lex/PPCallbacks.h"

#include "structural_source.h"

namespace lexicon::clang_frontend {
namespace {

std::string severity(clang::DiagnosticsEngine::Level level) {
  switch (level) {
  case clang::DiagnosticsEngine::Ignored:
  case clang::DiagnosticsEngine::Remark:
    return "remark";
  case clang::DiagnosticsEngine::Note:
    return "note";
  case clang::DiagnosticsEngine::Warning:
    return "warning";
  case clang::DiagnosticsEngine::Error:
    return "error";
  case clang::DiagnosticsEngine::Fatal:
    return "fatal";
  }
  return "error";
}

class DiagnosticObserver final : public clang::DiagnosticConsumer {
public:
  DiagnosticObserver(State &state, std::string root, std::string language,
                     std::string translation_unit)
      : state_(state), root_(std::move(root)), language_(std::move(language)),
        translation_unit_(std::move(translation_unit)) {}

  void HandleDiagnostic(clang::DiagnosticsEngine::Level level,
                        const clang::Diagnostic &info) override {
    llvm::SmallString<256> message;
    info.FormatDiagnostic(message);
    Diagnostic value{
        .severity = severity(level),
        .message = message.str().str(),
    };
    if (info.hasSourceManager() && info.getLocation().isValid()) {
      const auto &sources = info.getSourceManager();
      if (auto path = source_path(sources, info.getLocation(), root_)) {
        value.path = *path;
        value.span = source_span(sources, clang::LangOptions(),
                                 clang::SourceRange(info.getLocation()),
                                 *path);
        state_.file(*path, language_, translation_unit_);
      }
    }
    state_.add_diagnostic(std::move(value));
  }

private:
  State &state_;
  std::string root_;
  std::string language_;
  std::string translation_unit_;
};

class PreprocessorObserver final : public clang::PPCallbacks {
public:
  PreprocessorObserver(State &state, clang::SourceManager &sources,
                       const clang::LangOptions &language_options,
                       std::string root, std::string translation_unit,
                       std::string language)
      : state_(state), sources_(sources), language_options_(language_options),
        root_(std::move(root)), translation_unit_(std::move(translation_unit)),
        language_(std::move(language)) {}

  void InclusionDirective(
      clang::SourceLocation hash_location, const clang::Token &,
      llvm::StringRef file_name, bool angled,
      clang::CharSourceRange filename_range, clang::OptionalFileEntryRef file,
      llvm::StringRef, llvm::StringRef, const clang::Module *,
      clang::SrcMgr::CharacteristicKind) override {
    auto path = source_path(sources_, hash_location, root_);
    if (!path) {
      return;
    }
    auto spelling = sources_.getSpellingLoc(hash_location);
    Include value{
        .target = file_name.str(),
        .resolved_path =
            file ? repository_path(file->getName(), root_).value_or("") : "",
        .expression = angled ? "<" + file_name.str() + ">"
                             : "\"" + file_name.str() + "\"",
        .system = angled,
        .offset = sources_.getFileOffset(spelling),
        .span = source_span(sources_, language_options_,
                            clang::SourceRange(hash_location,
                                               filename_range.getEnd()),
                            *path),
    };
    state_.file(*path, language_, translation_unit_)
        .includes.push_back(std::move(value));
  }

  void MacroDefined(const clang::Token &name_token,
                    const clang::MacroDirective *directive) override {
    const auto *info = directive ? directive->getMacroInfo() : nullptr;
    if (!info) {
      return;
    }
    auto path = source_path(sources_, info->getDefinitionLoc(), root_);
    if (!path) {
      return;
    }
    auto spelling = sources_.getSpellingLoc(info->getDefinitionLoc());
    Macro value{
        .name = name_token.getIdentifierInfo()
                    ? name_token.getIdentifierInfo()->getName().str()
                    : std::string(),
        .function_like = info->isFunctionLike(),
        .conditional = conditional_depth_ != 0,
        .offset = sources_.getFileOffset(spelling),
        .span = source_span(
            sources_, language_options_,
            clang::SourceRange(info->getDefinitionLoc(),
                               info->getDefinitionEndLoc()),
            *path),
    };
    value.compiler_id = "macro:" + *path + ":" + std::to_string(value.offset) +
                        ":" + value.name;
    for (const auto *parameter : info->params()) {
      value.parameters.push_back(parameter->getName().str());
    }
    if (!info->tokens_empty()) {
      const auto &first = *info->tokens_begin();
      const auto &last = *(info->tokens_end() - 1);
      value.replacement = normalize_space(source_text(
          sources_, language_options_,
          clang::SourceRange(first.getLocation(), last.getEndLoc())));
    }
    state_.file(*path, language_, translation_unit_)
        .macros.push_back(std::move(value));
  }

  void If(clang::SourceLocation, clang::SourceRange,
          ConditionValueKind) override {
    ++conditional_depth_;
  }
  void Ifdef(clang::SourceLocation, const clang::Token &,
             const clang::MacroDefinition &) override {
    ++conditional_depth_;
  }
  void Ifndef(clang::SourceLocation, const clang::Token &,
              const clang::MacroDefinition &) override {
    ++conditional_depth_;
  }
  void Endif(clang::SourceLocation, clang::SourceLocation) override {
    if (conditional_depth_ != 0) {
      --conditional_depth_;
    }
  }

private:
  State &state_;
  clang::SourceManager &sources_;
  const clang::LangOptions &language_options_;
  std::string root_;
  std::string translation_unit_;
  std::string language_;
  std::size_t conditional_depth_ = 0;
};

class StructuralAction final : public clang::ASTFrontendAction {
public:
  StructuralAction(State &state, std::string root)
      : state_(state), root_(std::move(root)) {}

  std::unique_ptr<clang::ASTConsumer>
  CreateASTConsumer(clang::CompilerInstance &compiler,
                    llvm::StringRef input_file) override {
    auto translation_unit = repository_path(input_file, root_)
                                .value_or(input_file.str());
    auto language = compiler.getLangOpts().CPlusPlus ? "cpp" : "c";
    state_.file(translation_unit, language, translation_unit);
    compiler.getDiagnostics().setClient(
        new DiagnosticObserver(state_, root_, language, translation_unit), true);
    compiler.getPreprocessor().addPPCallbacks(
        std::make_unique<PreprocessorObserver>(
            state_, compiler.getSourceManager(), compiler.getLangOpts(), root_,
            translation_unit, language));
    return make_ast_consumer(state_, compiler, root_, translation_unit, language);
  }

private:
  State &state_;
  std::string root_;
};

class StructuralActionFactory final
    : public clang::tooling::FrontendActionFactory {
public:
  StructuralActionFactory(State &state, std::string root)
      : state_(state), root_(std::move(root)) {}

  std::unique_ptr<clang::FrontendAction> create() override {
    return std::make_unique<StructuralAction>(state_, root_);
  }

private:
  State &state_;
  std::string root_;
};

} // namespace

std::unique_ptr<clang::tooling::FrontendActionFactory>
make_frontend_factory(State &state, std::string repository_root) {
  return std::make_unique<StructuralActionFactory>(
      state, std::move(repository_root));
}

} // namespace lexicon::clang_frontend
