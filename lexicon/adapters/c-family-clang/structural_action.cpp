#include "structural_frontend.h"

#include <memory>
#include <filesystem>
#include <functional>
#include <optional>
#include <string>
#include <utility>

#include "clang/Basic/Diagnostic.h"
#include "clang/Frontend/CompilerInstance.h"
#include "clang/Frontend/CompilerInvocation.h"
#include "clang/Frontend/FrontendAction.h"
#include "clang/Lex/Lexer.h"
#include "clang/Lex/PPCallbacks.h"
#include "llvm/Support/VirtualFileSystem.h"

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
    clang::DiagnosticConsumer::HandleDiagnostic(level, info);
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
        if (state_.owns(*path)) {
          state_.file(*path, language_, translation_unit_);
        }
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

  void FileChanged(clang::SourceLocation location, FileChangeReason reason,
                   clang::SrcMgr::CharacteristicKind,
                   clang::FileID) override {
    if (reason != EnterFile) {
      return;
    }
    auto path = source_path(sources_, location, root_);
    if (path && state_.owns(*path)) {
      state_.file(*path, language_, translation_unit_);
    }
  }

  void InclusionDirective(
      clang::SourceLocation hash_location, const clang::Token &,
      llvm::StringRef file_name, bool angled,
      clang::CharSourceRange filename_range, clang::OptionalFileEntryRef file,
      llvm::StringRef, llvm::StringRef, const clang::Module *,
      clang::SrcMgr::CharacteristicKind) override {
    auto path = source_path(sources_, hash_location, root_);
    if (!path || !state_.owns(*path)) {
      return;
    }
    auto spelling = sources_.getSpellingLoc(hash_location);
    Include value{
        .target = file_name.str(),
        .resolved_path =
            file ? repository_path(file->getName(), root_,
                                    sources_.getFileManager()).value_or("") : "",
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
    if (!path || !state_.owns(*path)) {
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
  StructuralAction(ParseUnit unit, State state, std::string root,
                   std::function<void(State, int)> submit)
      : unit_(std::move(unit)), state_(std::move(state)),
        root_(std::move(root)), submit_(std::move(submit)) {}

  ~StructuralAction() override {
    if (submit_) {
      if (!ended_) {
        const auto language = state_.translation_units.empty()
                                  ? std::string("cpp")
                                  : state_.translation_units.front().language;
        state_.add_diagnostic({
            .severity = "error",
            .message = "Clang frontend did not complete translation unit " +
                       unit_.translation_unit,
            .path = unit_.translation_unit,
        });
        if (state_.owns(unit_.translation_unit)) {
          state_.file(unit_.translation_unit, language,
                      unit_.translation_unit);
        }
      }
      submit_(std::move(state_), ended_ ? 0 : 1);
    }
  }

  bool BeginSourceFileAction(clang::CompilerInstance &compiler) override {
    return clang::ASTFrontendAction::BeginSourceFileAction(compiler);
  }

  void EndSourceFileAction() override {
    clang::ASTFrontendAction::EndSourceFileAction();
    ended_ = true;
  }

  std::unique_ptr<clang::ASTConsumer>
  CreateASTConsumer(clang::CompilerInstance &compiler,
                    llvm::StringRef input_file) override {
    auto translation_unit = unit_.translation_unit;
    auto language = compiler.getLangOpts().CPlusPlus ? "cpp" : "c";
    compiler.getDiagnostics().setClient(
        new DiagnosticObserver(state_, root_, language, translation_unit), true);
    compiler.getPreprocessor().addPPCallbacks(
        std::make_unique<PreprocessorObserver>(
            state_, compiler.getSourceManager(), compiler.getLangOpts(), root_,
            translation_unit, language));
    return make_ast_consumer(state_, compiler, root_, translation_unit, language);
  }

private:
  ParseUnit unit_;
  State state_;
  std::string root_;
  std::function<void(State, int)> submit_;
  bool ended_ = false;
};

class StructuralActionFactory final
    : public clang::tooling::FrontendActionFactory {
public:
  StructuralActionFactory(
      std::vector<ParseUnit> units, CompilationCommands &database,
      std::vector<std::string> owned_files, std::string root,
      std::function<void(std::size_t, State, int)> submit)
      : units_(std::move(units)), database_(database),
        owned_files_(std::move(owned_files)), root_(std::move(root)),
        submit_(std::move(submit)) {}

  std::unique_ptr<clang::FrontendAction> create() override {
    if (current_index_ >= units_.size() || current_submitted_) {
      return nullptr;
    }
    auto unit = units_[current_index_];
    auto state = make_translation_unit_state(root_, database_, unit,
                                             owned_files_);
    auto submit = [this](State result, int status) mutable {
      current_submitted_ = true;
      current_result_ = std::move(result);
      current_status_ = status;
    };
    return std::make_unique<StructuralAction>(
        std::move(unit), std::move(state), root_, std::move(submit));
  }

  bool runInvocation(
      std::shared_ptr<clang::CompilerInvocation> invocation,
      clang::FileManager *files,
      std::shared_ptr<clang::PCHContainerOperations> pch_container_ops,
      clang::DiagnosticConsumer *diagnostic_consumer) override {
    if (!invocation || invocation->getFrontendOpts().Inputs.empty()) {
      return false;
    }
    const auto input = invocation->getFrontendOpts().Inputs.front().getFile();
    auto input_file = std::filesystem::path(input.str());
    if (input_file.is_relative()) {
      if (auto directory = files->getVirtualFileSystem().getCurrentWorkingDirectory()) {
        input_file = std::filesystem::path(*directory) / input_file;
      }
    }
    const auto input_path = normalize_input(input_file.string());
    auto match = current_index_;
    while (match < units_.size() &&
           normalize_input(units_[match].translation_unit) != input_path) {
      ++match;
    }
    if (match == units_.size()) {
      return false;
    }
    while (current_index_ < match) {
      submit_failed(units_[current_index_],
                    "Clang skipped translation unit before " + input.str());
      ++current_index_;
    }
    current_submitted_ = false;
    current_result_.reset();
    current_status_ = 0;
    clang::tooling::FrontendActionFactory::runInvocation(
        std::move(invocation), files, std::move(pch_container_ops),
        diagnostic_consumer);
    auto completed = false;
    if (!current_submitted_) {
      submit_failed(units_[current_index_],
                    "Clang did not create a frontend action for " +
                        input.str());
    } else {
      const auto status = current_status_;
      submit_(units_[current_index_].rank, std::move(*current_result_), status);
      current_result_.reset();
      completed = status == 0;
    }
    current_submitted_ = true;
    ++current_index_;
    return completed;
  }

private:
  std::string normalize_input(llvm::StringRef input) const {
    auto path = std::filesystem::path(input.str());
    if (path.is_relative()) {
      path = std::filesystem::path(root_) / path;
    }
    return std::filesystem::absolute(path).lexically_normal().string();
  }

  void submit_failed(const ParseUnit &unit, std::string message) {
    auto state = make_translation_unit_state(root_, database_, unit,
                                             owned_files_);
    state.add_diagnostic({
        .severity = "error",
        .message = std::move(message),
    });
    if (state.owns(unit.translation_unit)) {
      const auto language = state.translation_units.empty()
                                ? std::string("cpp")
                                : state.translation_units.front().language;
      state.file(unit.translation_unit, language, unit.translation_unit);
    }
    submit_(unit.rank, std::move(state), 1);
  }

  std::vector<ParseUnit> units_;
  CompilationCommands &database_;
  std::vector<std::string> owned_files_;
  std::string root_;
  std::function<void(std::size_t, State, int)> submit_;
  std::optional<State> current_result_;
  std::size_t current_index_ = 0;
  int current_status_ = 0;
  bool current_submitted_ = true;
};

} // namespace

std::unique_ptr<clang::tooling::FrontendActionFactory>
make_frontend_factory(const std::vector<ParseUnit> &units,
                      CompilationCommands &database,
                      std::vector<std::string> owned_files,
                      std::string repository_root,
                      std::function<void(std::size_t, State, int)> submit) {
  return std::make_unique<StructuralActionFactory>(
      units, database, std::move(owned_files), std::move(repository_root),
      std::move(submit));
}

State make_translation_unit_state(
    const std::string &repository_root, CompilationCommands &database,
    const ParseUnit &unit, const std::vector<std::string> &owned_files) {
  State state(repository_root);
  state.set_owned_files(owned_files);
  const auto absolute =
      (std::filesystem::path(repository_root) /
       std::filesystem::path(unit.translation_unit))
          .lexically_normal()
          .string();
  const auto commands = database.getCompileCommands(absolute);
  if (commands.empty()) {
    state.add_diagnostic({
        .severity = "error",
        .message = "no compile command for translation unit " +
                   unit.translation_unit,
        .path = unit.translation_unit,
    });
    return state;
  }
  const auto &command = commands.front();
  state.translation_units.push_back({
      .path = unit.translation_unit,
      .language = language_for(unit.translation_unit, command.CommandLine),
      .directory = command.Directory,
      .arguments = command.CommandLine,
      .synthesized = unit.synthesized,
  });
  if (state.owns(unit.translation_unit)) {
    state.file(unit.translation_unit, state.translation_units.front().language,
               unit.translation_unit);
  }
  return state;
}

} // namespace lexicon::clang_frontend
