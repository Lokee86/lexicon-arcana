#pragma once

#include <memory>
#include <functional>
#include <string>
#include <vector>

#include "clang/AST/ASTConsumer.h"
#include "clang/Frontend/CompilerInstance.h"
#include "clang/Tooling/Tooling.h"

#include "structural_model.h"
#include "structural_compilation.h"

namespace lexicon::clang_frontend {

std::unique_ptr<clang::ASTConsumer>
make_ast_consumer(State &state, clang::CompilerInstance &compiler,
                  std::string repository_root, std::string translation_unit,
                  std::string language, std::size_t rank);

std::unique_ptr<clang::tooling::FrontendActionFactory>
make_frontend_factory(const std::vector<ParseUnit> &units,
                      CompilationCommands &database,
                      std::vector<std::string> owned_files,
                      std::string repository_root,
                      std::function<void(std::size_t, State, int)> submit,
                      std::vector<std::string> prior_claims = {});

State make_translation_unit_state(const std::string &repository_root,
                                  CompilationCommands &database,
                                  const ParseUnit &unit,
                                  const std::vector<std::string> &owned_files);

} // namespace lexicon::clang_frontend
